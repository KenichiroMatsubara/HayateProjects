//! Skia レイヤ rasterizer / compositor の出力パリティ + work-count 契約（ADR-0125・ADR-0146 §6）。
//! tiny-skia `tests/layer_compositor.rs` と同型: 「各レイヤを raster → placement quad で合成」した
//! 結果が「全面 `render_scene`」とピクセル一致し、composite-only フレームでは raster が 0 回になる。

mod support;

use std::collections::HashSet;

use hayate_core::element::style::{Dimension, StyleProp};
use hayate_core::{Color, CommittedFrame, ElementId, ElementKind, ElementTree};
use hayate_layer_compositor::layer_scene::{
    collect_layer_placements, extract_layer_scene, extract_root_scene,
};
use hayate_layer_compositor::{
    scroll_layer_geometry_from_inputs, CompositeQuad, GpuBudget, LayerCompositor, LayerRasterizer,
    PresentPlanner,
};
use hayate_scene_renderer_skia::{
    SkiaCompositeTarget, SkiaLayerCompositor, SkiaLayerPresenter, SkiaLayerRasterizer,
    new_raster_surface, read_rgba,
};

const W: u32 = 200;
const H: u32 = 200;
const CLEAR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

fn px(v: f32) -> Dimension {
    Dimension::px(v)
}

fn render_full(tree: &ElementTree) -> Vec<u8> {
    support::render_scene_to_pixels_scaled(tree.scene_graph(), W, H, 1.0)
}

/// 本 crate の `SkiaLayerRasterizer` / `SkiaLayerCompositor`（trait 実装）で合成する。
fn render_layered(tree: &ElementTree, root: ElementId) -> Vec<u8> {
    let graph = tree.scene_graph();
    let boundaries: HashSet<ElementId> = tree.frame_layers().iter().copied().collect();
    let placements = collect_layer_placements(graph, root, &boundaries);

    let mut rasterizer = SkiaLayerRasterizer::new(W, H, 1.0);
    for &layer in tree.frame_layers() {
        let scene = if layer == root {
            extract_root_scene(graph, root, &boundaries)
        } else {
            match extract_layer_scene(graph, layer, &boundaries) {
                Some(s) => s,
                None => continue,
            }
        };
        rasterizer.rasterize(layer, &scene, None).unwrap();
    }

    let quads: Vec<CompositeQuad<'_, skia_safe::Image>> = placements
        .iter()
        .filter_map(|p| {
            rasterizer.texture(p.layer).map(|texture| CompositeQuad {
                layer: p.layer,
                transform: p.transform,
                opacity: 1.0,
                clip: p.clip,
                texture,
            })
        })
        .collect();

    let mut compositor = SkiaLayerCompositor::new(1.0);
    let mut target = SkiaCompositeTarget {
        surface: new_raster_surface(W as i32, H as i32).unwrap(),
        clear: CLEAR,
    };
    compositor.composite(&mut target, &quads).unwrap();
    read_rgba(&mut target.surface)
}

fn assert_pixels_equal(full: &[u8], layered: &[u8], label: &str) {
    // クリップ境界の AA 合成順だけは分解で ±数値ずれる（tiny-skia の oracle と同じ許容）。
    let mut worst = 0u8;
    let mut worst_at = 0usize;
    for (i, (a, b)) in full.iter().zip(layered.iter()).enumerate() {
        let d = a.abs_diff(*b);
        if d > worst {
            worst = d;
            worst_at = i;
        }
    }
    assert!(
        worst <= 2,
        "{label}: 全面 raster と Skia レイヤ合成の出力が一致しない（byte {worst_at} で {worst} 差）"
    );
}

fn transform_tree() -> (ElementTree, ElementId, ElementId) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let boxed = tree.element_create(1, ElementKind::View);
    let inner = tree.element_create(2, ElementKind::View);
    tree.element_append_child(root, boxed);
    tree.element_append_child(boxed, inner);
    tree.set_root(root);
    tree.set_viewport(W as f32, H as f32);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(px(W as f32)),
            StyleProp::Height(px(H as f32)),
            StyleProp::BackgroundColor(Color::new(0.5, 0.5, 0.5, 1.0)),
        ],
    );
    tree.element_set_style(
        boxed,
        &[
            StyleProp::Width(px(50.0)),
            StyleProp::Height(px(50.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
        ],
    );
    tree.element_set_transform(boxed, Some([1.0, 0.0, 0.0, 1.0, 30.0, 20.0]));
    tree.element_set_style(
        inner,
        &[
            StyleProp::Width(px(20.0)),
            StyleProp::Height(px(20.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0)),
        ],
    );
    (tree, boxed, inner)
}

#[test]
fn transform_layer_skia_composite_matches_full_raster() {
    let (mut tree, _boxed, _inner) = transform_tree();
    let root = ElementId::from_u64(0);
    let _ = tree.render(0.0);
    assert_pixels_equal(&render_full(&tree), &render_layered(&tree, root), "skia transform layer");
}

#[test]
fn native_shared_presenter_matches_full_raster() {
    let (mut tree, _boxed, _inner) = transform_tree();
    let _ = tree.render(0.0);
    let mut presenter = SkiaLayerPresenter::new(W, H, 1.0);
    let target = new_raster_surface(W as i32, H as i32).unwrap();
    let mut target = presenter
        .present(
            tree.scene_graph(),
            tree.frame_layers(),
            tree.frame_layer_dirty(),
            &Default::default(),
            CLEAR,
            (0.0, 0.0),
            hayate_layer_compositor::GpuBudget::from_viewports(W, H, 8.0),
            target,
        )
        .unwrap();
    assert_pixels_equal(
        &render_full(&tree),
        &read_rgba(&mut target),
        "shared native skia presenter",
    );
}

fn scroll_tree() -> (ElementTree, ElementId) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let scroll = tree.element_create(1, ElementKind::ScrollView);
    let content = tree.element_create(2, ElementKind::View);
    let hidden_text = tree.element_create(3, ElementKind::Text);
    let empty_text = tree.element_create(4, ElementKind::Text);
    tree.element_append_child(root, scroll);
    tree.element_append_child(root, hidden_text);
    tree.element_append_child(scroll, content);
    tree.element_append_child(content, empty_text);
    tree.set_root(root);
    tree.set_viewport(W as f32, H as f32);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(px(W as f32)),
            StyleProp::Height(px(H as f32)),
            StyleProp::BackgroundColor(Color::new(0.9, 0.9, 0.9, 1.0)),
        ],
    );
    tree.element_set_style(
        scroll,
        &[StyleProp::Width(px(W as f32)), StyleProp::Height(px(H as f32))],
    );
    tree.element_set_style(
        content,
        &[
            StyleProp::Display(hayate_core::DisplayValue::Flex),
            StyleProp::FlexDirection(hayate_core::FlexDirectionValue::Column),
            StyleProp::Width(px(W as f32)),
            StyleProp::Height(px(2000.0)),
        ],
    );
    // 実アプリでは conditional label が非表示・未計測になったり、空文字の text が
    // scroll subtree に残る。どちらも安定後は shape request を持ち越さず、純粋な
    // offset 更新を root / scroll content の dirty へ昇格させてはならない（#843/#844）。
    tree.element_set_style(
        hidden_text,
        &[StyleProp::Display(hayate_core::DisplayValue::None)],
    );
    tree.element_set_text(hidden_text, "conditionally hidden");
    tree.element_set_text(empty_text, "");
    for i in 0..20 {
        let row = tree.element_create(10 + i, ElementKind::View);
        tree.element_append_child(content, row);
        let tone = i as f64 / 19.0;
        tree.element_set_style(
            row,
            &[
                StyleProp::Width(px(W as f32)),
                StyleProp::Height(px(100.0)),
                StyleProp::BackgroundColor(Color::new(tone, 1.0 - tone, 0.5, 1.0)),
            ],
        );
    }
    (tree, scroll)
}

/// Platform wiring と同じ順序で、Core の committed dirty capture を scroll geometry へ
/// projection し、Native Skia の共有 presenter へ渡す。
fn present_committed_frame(
    frame: &CommittedFrame<'_>,
    presenter: &mut SkiaLayerPresenter,
    budget: GpuBudget,
) -> Vec<u8> {
    let geometry = scroll_layer_geometry_from_inputs(frame.scroll_inputs());
    let mut dirty = frame.content_dirty_layers().clone();
    dirty.extend(frame.chrome_dirty_layers().iter().copied());
    let target = new_raster_surface(W as i32, H as i32).unwrap();
    let mut target = presenter
        .present(
            frame.scene(),
            frame.layers(),
            &dirty,
            &geometry,
            CLEAR,
            (0.0, 0.0),
            budget,
            target,
        )
        .unwrap();
    read_rgba(&mut target)
}

#[test]
fn native_presenter_keeps_app_like_in_band_scroll_composite_only() {
    let (mut tree, scroll) = scroll_tree();
    let mut presenter = SkiaLayerPresenter::new(W, H, 1.0);
    let budget = GpuBudget::from_viewports(W, H, 8.0);
    let frame = tree.commit_rendered_frame(0.0);
    let _ = present_committed_frame(&frame, &mut presenter, budget);
    assert!(presenter.last_raster_count() > 0);

    tree.element_set_scroll_offset(scroll, 0.0, 150.0);
    let frame = tree.commit_rendered_frame(16.0);
    assert!(
        frame.content_dirty_layers().is_empty(),
        "pure scroll must keep both the root and scroll content caches clean"
    );
    assert!(
        frame.chrome_dirty_layers().contains(&scroll),
        "the scroll offset must still schedule the fixed chrome update"
    );
    let pixels = present_committed_frame(&frame, &mut presenter, budget);
    assert_eq!(
        presenter.last_raster_count(),
        0,
        "in-band scroll must be composite-only"
    );
    assert_pixels_equal(
        &render_full(&tree),
        &pixels,
        "composite-only native skia scroll",
    );
}

#[test]
fn native_presenter_rerasters_scroll_content_after_crossing_the_cached_band() {
    let (mut tree, scroll) = scroll_tree();
    let mut presenter = SkiaLayerPresenter::new(W, H, 1.0);
    let budget = GpuBudget::from_viewports(W, H, 8.0);
    let frame = tree.commit_rendered_frame(0.0);
    let _ = present_committed_frame(&frame, &mut presenter, budget);

    tree.element_set_scroll_offset(scroll, 0.0, 900.0);
    let frame = tree.commit_rendered_frame(16.0);
    assert!(
        frame.content_dirty_layers().is_empty(),
        "crossing an overscan band is a presenter cache miss, not a Core content mutation"
    );
    let pixels = present_committed_frame(&frame, &mut presenter, budget);

    assert_eq!(
        presenter.last_raster_count(),
        1,
        "leaving the cached overscan band must raster the scroll content once"
    );
    assert_pixels_equal(
        &render_full(&tree),
        &pixels,
        "native skia scroll after overscan band refresh",
    );
}

#[test]
fn native_presenter_rerasters_scroll_content_after_a_real_content_change() {
    let (mut tree, scroll) = scroll_tree();
    let mut presenter = SkiaLayerPresenter::new(W, H, 1.0);
    let budget = GpuBudget::from_viewports(W, H, 8.0);
    let frame = tree.commit_rendered_frame(0.0);
    let _ = present_committed_frame(&frame, &mut presenter, budget);

    let first_row = ElementId::from_u64(10);
    tree.element_set_style(
        first_row,
        &[StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0))],
    );
    let frame = tree.commit_rendered_frame(16.0);
    assert!(
        frame.content_dirty_layers().contains(&scroll),
        "a descendant pixel change must dirty its enclosing scroll content layer"
    );
    let pixels = present_committed_frame(&frame, &mut presenter, budget);

    assert_eq!(
        presenter.last_raster_count(),
        1,
        "a real content change must refresh exactly the scroll content cache"
    );
    assert_pixels_equal(
        &render_full(&tree),
        &pixels,
        "native skia scroll after content refresh",
    );
}

#[test]
fn native_presenter_rerasters_scroll_content_after_geometry_changes() {
    let (mut tree, scroll) = scroll_tree();
    let mut presenter = SkiaLayerPresenter::new(W, H, 1.0);
    let budget = GpuBudget::from_viewports(W, H, 8.0);
    let frame = tree.commit_rendered_frame(0.0);
    let _ = present_committed_frame(&frame, &mut presenter, budget);

    let first_row = ElementId::from_u64(10);
    tree.element_set_style(first_row, &[StyleProp::Height(px(140.0))]);
    let frame = tree.commit_rendered_frame(16.0);
    assert!(
        frame.content_dirty_layers().contains(&scroll),
        "a descendant reflow must dirty its enclosing scroll content layer"
    );
    let pixels = present_committed_frame(&frame, &mut presenter, budget);

    assert_eq!(
        presenter.last_raster_count(),
        1,
        "geometry changes must refresh exactly the scroll content cache"
    );
    assert_pixels_equal(
        &render_full(&tree),
        &pixels,
        "native skia scroll after geometry refresh",
    );
}

#[test]
fn native_presenter_rerasters_clean_layers_after_cache_eviction() {
    let (mut tree, _scroll) = scroll_tree();
    let mut presenter = SkiaLayerPresenter::new(W, H, 1.0);
    let frame = tree.commit_rendered_frame(0.0);
    let _ = present_committed_frame(&frame, &mut presenter, GpuBudget::from_bytes(0));

    let frame = tree.commit_rendered_frame(16.0);
    assert!(
        frame.content_dirty_layers().is_empty(),
        "the second frame is clean at the Core seam; only eviction should force work"
    );
    let pixels =
        present_committed_frame(&frame, &mut presenter, GpuBudget::from_viewports(W, H, 8.0));

    assert_eq!(
        presenter.last_raster_count(),
        tree.frame_layers().len(),
        "evicted root and scroll caches must be rebuilt even on a clean frame"
    );
    assert_pixels_equal(
        &render_full(&tree),
        &pixels,
        "native skia after cache eviction",
    );
}

/// per-layer present を 1 フレーム回し、raster したレイヤ数を返す（work-count 用）。
fn pump(tree: &mut ElementTree, planner: &mut PresentPlanner, rz: &mut SkiaLayerRasterizer, ts: f64) -> usize {
    let _ = tree.render(ts);
    let graph = tree.scene_graph().clone();
    let boundaries: HashSet<ElementId> = tree.frame_layers().iter().copied().collect();
    let root = tree.frame_layers()[0];
    let plan = planner.plan_layers(tree.frame_layers(), tree.frame_layer_dirty());
    for &layer in &plan.raster {
        let scene = if layer == root {
            extract_root_scene(&graph, root, &boundaries)
        } else {
            match extract_layer_scene(&graph, layer, &boundaries) {
                Some(s) => s,
                None => continue,
            }
        };
        rz.rasterize(layer, &scene, None).unwrap();
        planner.note_layer_rasterized(layer, rz.texture_bytes_per_layer());
    }
    plan.raster.len()
}

#[test]
fn transform_only_frames_do_not_raster_on_skia() {
    let (mut tree, boxed, _inner) = transform_tree();
    let mut planner = PresentPlanner::new();
    let mut rz = SkiaLayerRasterizer::new(W, H, 1.0);
    assert!(pump(&mut tree, &mut planner, &mut rz, 0.0) > 0, "cold フレームは raster");

    for frame in 1..=4 {
        tree.element_set_transform(boxed, Some([1.0, 0.0, 0.0, 1.0, frame as f64 * 10.0, 0.0]));
        let rasters = pump(&mut tree, &mut planner, &mut rz, frame as f64 * 16.0);
        assert_eq!(rasters, 0, "skia: transform のみのフレーム {frame} で全面ラスタが走った");
    }
}

#[test]
fn content_change_rerasters_only_the_dirty_layer_on_skia() {
    let (mut tree, boxed, inner) = transform_tree();
    let mut planner = PresentPlanner::new();
    let mut rz = SkiaLayerRasterizer::new(W, H, 1.0);
    let _ = pump(&mut tree, &mut planner, &mut rz, 0.0);

    tree.element_set_style(inner, &[StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0))]);
    let _ = tree.render(16.0);
    let plan = planner.plan_layers(tree.frame_layers(), tree.frame_layer_dirty());
    assert_eq!(plan.raster, vec![boxed], "skia: dirty レイヤ（boxed）だけ raster");
    assert!(plan.reuse.contains(&tree.frame_layers()[0]), "root レイヤは reuse（キャッシュ面合成）");
}
