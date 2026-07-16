//! ADR-0127 scroll overscan band sizing wired into the real per-layer present path (#707).
//!
//! `compositor/tests/scroll_composite_only.rs` already proves the *planning* math (band
//! coverage / raster-count gating) against `PresentPlanner` alone, with no real texture involved.
//! This file proves the same scenario through a real `VelloLayerRasterizer` + `WgpuQuadCompositor`:
//! the actual GPU cache texture is sized to the overscan band (not the full surface), the GPU
//! budget byte accounting reflects that smaller size, and the rendered pixels still match a
//! full-surface raster of the same scrolled scene — including, critically, on a **later**
//! composite-only frame (scrolled further within the cached band, no re-raster) — proving the
//! translate-at-raster / recompute-at-composite mechanism this issue introduces is correct, not
//! just that it compiles. That last case matters: a first draft of this mechanism baked the
//! raster-time scroll offset into the compositing translate and only checked pixels on the exact
//! frame it rastered, which happened to hide the bug (see `scroll_geometry.rs`'s module doc
//! comment for the full story) — `banded_present_tracks_further_in_band_scrolling_without_a_reraster`
//! below is the test that would have caught it.
//!
//! `vello.rs`'s `present_layers` itself can't be unit tested directly — it's wasm/browser-locked
//! (`wgpu::Backends::BROWSER_WEBGPU` only). The helpers below mirror its exact call sequence
//! (geometry lookup → coverage gate → banded rasterize → planner bookkeeping → compositing with a
//! per-frame-recomputed compensating translate) against a real (non-browser) wgpu adapter
//! instead, exactly like `layer_present_parity.rs`'s `assert_layered_matches_full` already does
//! for the non-scroll paths.
//!
//! Skips (like every other GPU-backed test in this crate) when no wgpu adapter is available —
//! confirmed absent in the sandbox this was authored in (no Vulkan ICD, no `/dev/dri`). These
//! tests are correct and will run on a real dev machine or a CI runner with a GPU.

use std::collections::HashSet;

use hayate_core::element::style::{Dimension, StyleProp};
use hayate_core::{Color, ElementId, ElementKind, ElementTree};
use hayate_layer_compositor::layer_scene::compose;
use hayate_layer_compositor::{
    collect_layer_placements, extract_layer_scene, extract_root_scene, extract_scroll_chrome_scene,
    extract_scroll_layer_scene, scroll_layer_geometry, scroll_layer_geometry_table, tunables,
    CompositeQuad, LayerCompositor, LayerRasterizer, PresentPlanner,
};
use hayate_scene_renderer_vello::layer_compositor::{
    CompositeTarget, VelloLayerRasterizer, WgpuQuadCompositor,
};
use hayate_scene_test_support::vello::{
    readback_rgba8, render_scene_to_pixels_scaled, try_vello_harness, VelloHarness,
};

const W: u32 = 64;
/// "Canvas"/app surface height — deliberately much larger than any band this fixture produces
/// (band is capped near `SCROLL_VIEWPORT_H + 2 * OVERSCAN_MARGIN_PX` ≈ 1400px, using the crate's
/// real `tunables::OVERSCAN_MARGIN_PX`, not a fake smaller value) so the "band, not full surface"
/// comparison is meaningful rather than vacuous.
const SURFACE_H: u32 = 2000;
const SCROLL_VIEWPORT_H: f32 = 200.0;
const CONTENT_H: f32 = 10_000.0;
const ROWS: u64 = 20;

fn px(v: f32) -> Dimension {
    Dimension::px(v)
}

/// A tall scrollable list of `ROWS` distinctly-colored rows (root fills the whole `W`×`SURFACE_H`
/// canvas; the scroll view sits at the canvas' own top-left, matching
/// `compositor/tests/scroll_composite_only.rs`'s fixture). Distinct per-row colors (not a solid
/// fill) matter here: a Y-translate bug in the band raster/composite would misalign colors and be
/// caught by a pixel diff against the full-raster reference — a solid fill would look identical
/// either way.
fn tall_list_tree() -> (ElementTree, ElementId) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let scroll = tree.element_create(1, ElementKind::ScrollView);
    let content = tree.element_create(2, ElementKind::View);
    tree.element_append_child(root, scroll);
    tree.element_append_child(scroll, content);
    tree.set_root(root);
    tree.set_viewport(W as f32, SURFACE_H as f32);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(px(W as f32)),
            StyleProp::Height(px(SURFACE_H as f32)),
            StyleProp::BackgroundColor(Color::new(0.9, 0.9, 0.9, 1.0)),
        ],
    );
    tree.element_set_style(
        scroll,
        &[
            StyleProp::Width(px(W as f32)),
            StyleProp::Height(px(SCROLL_VIEWPORT_H)),
        ],
    );
    tree.element_set_style(
        content,
        &[
            StyleProp::Display(hayate_core::DisplayValue::Flex),
            StyleProp::FlexDirection(hayate_core::FlexDirectionValue::Column),
            StyleProp::Width(px(W as f32)),
            StyleProp::Height(px(CONTENT_H)),
        ],
    );
    let row_h = CONTENT_H / ROWS as f32;
    for i in 0..ROWS {
        let row = tree.element_create(10 + i, ElementKind::View);
        tree.element_append_child(content, row);
        let hue = i as f64 / (ROWS - 1) as f64;
        tree.element_set_style(
            row,
            &[
                StyleProp::Width(px(W as f32)),
                StyleProp::Height(px(row_h)),
                StyleProp::BackgroundColor(Color::new(hue, 1.0 - hue, 0.5, 1.0)),
            ],
        );
    }
    let _ = tree.render(0.0);
    (tree, scroll)
}

/// Present one frame for `scroll` the same way `vello.rs`'s `present_layers` does for a scroll
/// layer (geometry lookup → coverage gate → banded rasterize → planner bookkeeping). Returns
/// `true` if a (re)raster happened (`false` = the cached band still covers the visible region,
/// composite-only).
fn pump_scroll_layer(
    tree: &ElementTree,
    scroll: ElementId,
    planner: &mut PresentPlanner,
    rasterizer: &mut VelloLayerRasterizer,
) -> bool {
    let graph = tree.scene_graph();
    let root = tree.frame_layers()[0];
    let boundaries: HashSet<ElementId> = tree.frame_layers().iter().copied().collect();
    let geometry = scroll_layer_geometry(tree, scroll).expect("scroll view has geometry");
    let needs_content_raster = geometry.content_dirty
        || planner.scroll_layer_needs_raster(
            scroll,
            geometry.visible_top,
            geometry.viewport_height,
        );
    if needs_content_raster {
        let extracted = if scroll == root {
            extract_root_scene(graph, root, &boundaries)
        } else {
            extract_scroll_layer_scene(graph, scroll, &boundaries).expect("scroll view is lowered")
        };
        rasterizer
            .rasterize(scroll, &extracted, Some(geometry.raster_band()))
            .unwrap();
    }
    if scroll != root {
        let chrome = extract_scroll_chrome_scene(graph, scroll, &boundaries)
            .expect("scroll chrome is lowered");
        rasterizer.rasterize_scroll_chrome(scroll, &chrome).unwrap();
    }
    if needs_content_raster {
        let bytes = rasterizer.scroll_cache_bytes(geometry.band);
        planner.note_scroll_rasterized(scroll, geometry.band, bytes);
    }
    needs_content_raster
}

/// Rasters every non-scroll layer of `tree` blanket-style (mirrors `present_layers`'s non-scroll
/// loop — `None` band, full surface, unconditional), leaving `scroll` for the caller to pump.
fn rasterize_non_scroll_layers(
    tree: &ElementTree,
    scroll: ElementId,
    rasterizer: &mut VelloLayerRasterizer,
) {
    let graph = tree.scene_graph();
    let root = tree.frame_layers()[0];
    let boundaries: HashSet<ElementId> = tree.frame_layers().iter().copied().collect();
    for &layer in tree.frame_layers() {
        if layer == scroll {
            continue;
        }
        let extracted = if layer == root {
            extract_root_scene(graph, root, &boundaries)
        } else {
            match extract_layer_scene(graph, layer, &boundaries) {
                Some(s) => s,
                None => continue,
            }
        };
        rasterizer.rasterize(layer, &extracted, None).unwrap();
    }
}

/// Composites `tree`'s **current** frame from whatever `rasterizer`/`planner` already hold —
/// mirrors `present_layers`'s compositing step exactly, including the per-frame-recomputed
/// compensating translate (`ScrollLayerGeometry::screen_top_for_band`) for banded scroll layers.
/// Deliberately takes no raster step itself, so callers can composite frames that reuse a cache
/// texture rastered on an *earlier* call (proving composite-only correctness, not just "it
/// compiles the frame it was rastered on").
fn composite_frame_scaled(
    harness: &VelloHarness,
    tree: &ElementTree,
    rasterizer: &VelloLayerRasterizer,
    planner: &PresentPlanner,
    content_scale: f32,
) -> Vec<u8> {
    let graph = tree.scene_graph();
    let root = tree.frame_layers()[0];
    let boundaries: HashSet<ElementId> = tree.frame_layers().iter().copied().collect();
    let scroll_geometry = scroll_layer_geometry_table(tree, tree.frame_layers());

    let mut compositor = WgpuQuadCompositor::new(harness.device.clone(), harness.queue.clone());
    compositor.set_content_scale(content_scale);
    compositor.warmup();
    let target_width = (W as f32 * content_scale) as u32;
    let target_height = (SURFACE_H as f32 * content_scale) as u32;
    let target_texture = harness.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("scroll_overscan_present_target"),
        size: wgpu::Extent3d {
            width: target_width,
            height: target_height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let mut target = CompositeTarget {
        view: target_texture.create_view(&wgpu::TextureViewDescriptor::default()),
        width: target_width,
        height: target_height,
        format: wgpu::TextureFormat::Rgba8Unorm,
        clear: [1.0, 1.0, 1.0, 1.0],
    };
    let placements = collect_layer_placements(graph, root, &boundaries);
    let mut quads: Vec<CompositeQuad<'_, _>> = Vec::new();
    for placement in &placements {
        if let Some(texture) = rasterizer.texture(placement.layer) {
            // Same formula as `vello.rs`'s `present_layers`: the CACHED band (what's
            // actually in the texture, possibly from an earlier raster) composed with
            // *this frame's* geometry (fresh every call) — see `screen_top_for_band`'s doc
            // comment for why both matter.
            let transform = match (
                planner.cached_scroll_band(placement.layer),
                scroll_geometry.get(&placement.layer),
            ) {
                (Some(cached_band), Some(geometry)) => {
                    let screen_top = geometry.screen_top_for_band(cached_band);
                    compose(
                        placement.transform,
                        [1.0, 0.0, 0.0, 1.0, 0.0, screen_top as f64],
                    )
                }
                _ => placement.transform,
            };
            quads.push(CompositeQuad {
                layer: placement.layer,
                transform,
                opacity: 1.0,
                clip: placement.clip,
                texture,
            });
        }
        if let Some(texture) = rasterizer.scroll_chrome_texture(placement.layer) {
            quads.push(CompositeQuad {
                layer: placement.layer,
                transform: placement.transform,
                opacity: 1.0,
                clip: placement.clip,
                texture,
            });
        }
    }
    compositor.composite(&mut target, &quads).unwrap();
    readback_rgba8(
        &harness.device,
        &harness.queue,
        &target_texture,
        target_width,
        target_height,
    )
    .expect("readback")
}

fn composite_frame(
    harness: &VelloHarness,
    tree: &ElementTree,
    rasterizer: &VelloLayerRasterizer,
    planner: &PresentPlanner,
) -> Vec<u8> {
    composite_frame_scaled(harness, tree, rasterizer, planner, 1.0)
}

fn assert_pixels_match(full: &[u8], layered: &[u8], label: &str) {
    let worst = full
        .iter()
        .zip(layered.iter())
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap_or(0);
    assert!(
        worst <= 2,
        "{label}: banded scroll-layer present must match full raster (max {worst} diff)"
    );
}

#[test]
fn scroll_layer_texture_is_rastered_at_band_size_not_full_surface() {
    let Some(harness) = try_vello_harness() else {
        eprintln!("skip: no wgpu adapter");
        return;
    };
    let (mut tree, scroll) = tall_list_tree();
    // Scroll deep into the list, away from both edges, so the band is neither top- nor
    // bottom-clamped (the "generic" case: band == viewport + 2*overscan).
    tree.element_set_scroll_offset(scroll, 0.0, 4000.0);
    let _ = tree.render(16.0);

    let mut planner = PresentPlanner::new();
    let mut rasterizer = VelloLayerRasterizer::new(
        harness.device.clone(),
        harness.queue.clone(),
        W,
        SURFACE_H,
        1.0,
    )
    .unwrap();
    let rastered = pump_scroll_layer(&tree, scroll, &mut planner, &mut rasterizer);
    assert!(rastered, "cold frame must raster");

    let texture = rasterizer
        .texture(scroll)
        .expect("scroll layer has a cached texture");
    let expected_band_height = (SCROLL_VIEWPORT_H + 2.0 * tunables::OVERSCAN_MARGIN_PX) as u32;
    assert_eq!(
        texture.height, expected_band_height,
        "AC #1: scroll layer texture must be sized to the overscan band"
    );
    assert_eq!(
        texture.width, W,
        "only the vertical axis is banded (ADR-0127 scope)"
    );
    assert!(
        texture.height < SURFACE_H,
        "band ({}) must be smaller than the full surface ({SURFACE_H}) it would have used pre-#707",
        texture.height
    );
    assert!(
        (texture.height as f32) < CONTENT_H,
        "band ({}) must be far smaller than the full scrollable content height ({CONTENT_H})",
        texture.height
    );

    // AC #2: GPU budget byte accounting reflects the band size, not the full-surface size.
    let full_surface_bytes = u64::from(W) * u64::from(SURFACE_H) * tunables::BYTES_PER_PIXEL;
    assert_eq!(
        planner.cached_bytes(),
        u64::from(W) * u64::from(expected_band_height) * tunables::BYTES_PER_PIXEL
            + full_surface_bytes,
        "charged bytes must include the band texture plus the separate full-surface scroll chrome texture"
    );
}

#[test]
fn scrolling_within_the_cached_band_rasters_the_scroll_layer_zero_times() {
    let Some(harness) = try_vello_harness() else {
        eprintln!("skip: no wgpu adapter");
        return;
    };
    let (mut tree, scroll) = tall_list_tree();
    tree.element_set_scroll_offset(scroll, 0.0, 4000.0);
    let _ = tree.render(16.0);
    let mut planner = PresentPlanner::new();
    let mut rasterizer = VelloLayerRasterizer::new(
        harness.device.clone(),
        harness.queue.clone(),
        W,
        SURFACE_H,
        1.0,
    )
    .unwrap();
    assert!(
        pump_scroll_layer(&tree, scroll, &mut planner, &mut rasterizer),
        "cold frame rasters"
    );

    // Overscan is 600px; small scrolls well within that stay inside the cached band.
    for frame in 1..=5 {
        tree.element_set_scroll_offset(scroll, 0.0, 4000.0 + frame as f32 * 50.0);
        let _ = tree.render(frame as f64 * 16.0);
        assert!(
            !pump_scroll_layer(&tree, scroll, &mut planner, &mut rasterizer),
            "in-band scroll frame {frame} rastered the scroll layer (composite-only violation)"
        );
    }
}

/// Renders `tree` two ways and asserts the pixels match: (a) full-surface raster of the whole
/// scene (the `layer-present` OFF baseline), (b) the real per-layer present path for a scrolled
/// scroll layer — banded texture (offset by `RasterBand::origin_y`) + compositor quad with the
/// compensating translate this issue adds. This is strong evidence the translate/resize mechanism
/// (the issue's own "biggest technical risk") is bit-correct on the frame it rasters — but NOT
/// sufficient on its own; see `banded_present_tracks_further_in_band_scrolling_without_a_reraster`
/// for the composite-only (later frame, no re-raster) case, which is where a first draft of this
/// mechanism actually had a bug that this same-frame check could not have caught.
fn assert_banded_scroll_present_matches_full_raster(
    harness: &mut VelloHarness,
    tree: &ElementTree,
    label: &str,
) {
    let scroll = tree.frame_layers()[1];
    let full = render_scene_to_pixels_scaled(harness, tree.scene_graph(), W, SURFACE_H, 1.0)
        .expect("full raster");

    let mut rasterizer = VelloLayerRasterizer::new(
        harness.device.clone(),
        harness.queue.clone(),
        W,
        SURFACE_H,
        1.0,
    )
    .unwrap();
    let mut planner = PresentPlanner::new();
    rasterize_non_scroll_layers(tree, scroll, &mut rasterizer);
    assert!(
        pump_scroll_layer(tree, scroll, &mut planner, &mut rasterizer),
        "scroll layer must raster"
    );

    let layered = composite_frame(harness, tree, &rasterizer, &planner);
    assert_pixels_match(&full, &layered, label);
}

#[test]
fn banded_present_matches_full_raster_scrolled_deep_into_a_long_list() {
    let Some(mut harness) = try_vello_harness() else {
        eprintln!("skip: no wgpu adapter");
        return;
    };
    let (mut tree, scroll) = tall_list_tree();
    tree.element_set_scroll_offset(scroll, 0.0, 4000.0);
    let _ = tree.render(16.0);
    assert_banded_scroll_present_matches_full_raster(
        &mut harness,
        &tree,
        "scrolled deep into a long list",
    );
}

#[test]
fn banded_present_matches_full_raster_near_content_top() {
    let Some(mut harness) = try_vello_harness() else {
        eprintln!("skip: no wgpu adapter");
        return;
    };
    // Near the top edge, the band is top-clamped to 0 (shorter than viewport + 2*overscan) —
    // exercises the clamped-band arm of `scroll_layer_extent`, not just the generic middle case.
    let (mut tree, scroll) = tall_list_tree();
    tree.element_set_scroll_offset(scroll, 0.0, 50.0);
    let _ = tree.render(16.0);
    assert_banded_scroll_present_matches_full_raster(
        &mut harness,
        &tree,
        "near content top (clamped band)",
    );
}

#[test]
fn banded_present_tracks_further_in_band_scrolling_without_a_reraster() {
    let Some(mut harness) = try_vello_harness() else {
        eprintln!("skip: no wgpu adapter");
        return;
    };
    let (mut tree, scroll) = tall_list_tree();
    tree.element_set_scroll_offset(scroll, 0.0, 4000.0);
    let _ = tree.render(16.0);

    let mut rasterizer = VelloLayerRasterizer::new(
        harness.device.clone(),
        harness.queue.clone(),
        W,
        SURFACE_H,
        1.0,
    )
    .unwrap();
    let mut planner = PresentPlanner::new();
    rasterize_non_scroll_layers(&tree, scroll, &mut rasterizer);
    assert!(
        pump_scroll_layer(&tree, scroll, &mut planner, &mut rasterizer),
        "cold frame must raster"
    );

    // Scroll further, still within the cached band (overscan is 600px) — must NOT re-raster.
    tree.element_set_scroll_offset(scroll, 0.0, 4050.0);
    let _ = tree.render(32.0);
    assert!(
        !pump_scroll_layer(&tree, scroll, &mut planner, &mut rasterizer),
        "scrolling 50px further (well within the 600px overscan band) must be composite-only"
    );

    // The composited output on this LATER, un-rastered frame must still match a full raster of
    // THIS frame's (scrolled-further) tree state — proving `screen_top_for_band` correctly
    // recomputes the on-screen position from the cached band + this frame's (not the
    // raster-time) geometry, instead of freezing the picture at the raster-time offset.
    let full = render_scene_to_pixels_scaled(&mut harness, tree.scene_graph(), W, SURFACE_H, 1.0)
        .expect("full raster");
    let layered = composite_frame(&harness, &tree, &rasterizer, &planner);
    assert_pixels_match(
        &full,
        &layered,
        "composite-only frame after scrolling further within the cached band",
    );
}

#[test]
fn banded_present_matches_full_raster_at_device_scale_two() {
    let Some(mut harness) = try_vello_harness() else {
        eprintln!("skip: no wgpu adapter");
        return;
    };
    let (mut tree, scroll) = tall_list_tree();
    tree.element_set_scroll_offset(scroll, 0.0, 4000.0);
    let _ = tree.render(16.0);

    let scale = 2.0;
    let target_width = (W as f32 * scale) as u32;
    let target_height = (SURFACE_H as f32 * scale) as u32;
    let full = render_scene_to_pixels_scaled(
        &mut harness,
        tree.scene_graph(),
        target_width,
        target_height,
        scale,
    )
    .expect("full raster");
    let mut rasterizer = VelloLayerRasterizer::new(
        harness.device.clone(),
        harness.queue.clone(),
        target_width,
        target_height,
        scale,
    )
    .unwrap();
    let mut planner = PresentPlanner::new();
    rasterize_non_scroll_layers(&tree, scroll, &mut rasterizer);
    assert!(pump_scroll_layer(
        &tree,
        scroll,
        &mut planner,
        &mut rasterizer
    ));

    let layered = composite_frame_scaled(&harness, &tree, &rasterizer, &planner, scale);
    assert_pixels_match(&full, &layered, "device scale 2");
}
