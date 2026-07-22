//! Skia レイヤ rasterizer / compositor の出力パリティ + work-count 契約（ADR-0125・ADR-0146 §6）。
//! tiny-skia `tests/layer_compositor.rs` と同型: 「各レイヤを raster → placement quad で合成」した
//! 結果が「全面 `render_scene`」とピクセル一致し、composite-only フレームでは raster が 0 回になる。

mod support;

use hayate_core::element::style::{Dimension, Shadow, StyleProp};
use hayate_core::{
    Color, CommittedFrame, ElementId, ElementKind, ElementTree, LayerRasterBounds, LayerScene,
    LayerSceneKind, SceneGraph,
};
use hayate_layer_compositor::{
    scroll_layer_geometry_from_inputs, GpuBudget, LayerRasterizer, PresentPlanner, RasterBand,
};
use hayate_scene_renderer_skia::{
    new_raster_surface, read_rgba, SkiaLayerPresenter, SkiaLayerRasterizer, SkiaLayerSurfaceFactory,
};

const W: u32 = 200;
const H: u32 = 200;
const CLEAR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

fn px(v: f32) -> Dimension {
    Dimension::px(v)
}

fn render_full(tree: &ElementTree) -> Vec<u8> {
    support::render_scene_to_pixels_scaled(tree.committed_frame().snapshot(), W, H, 1.0)
}

fn render_full_at(tree: &ElementTree, scale: f32) -> Vec<u8> {
    support::render_scene_to_pixels_scaled(
        tree.committed_frame().snapshot(),
        (W as f32 * scale) as u32,
        (H as f32 * scale) as u32,
        scale,
    )
}

fn render_presented_at(tree: &ElementTree, scale: f32) -> Vec<u8> {
    let frame = tree.committed_frame();
    let geometry = scroll_layer_geometry_from_inputs(frame.scroll_inputs());
    let width = (W as f32 * scale) as u32;
    let height = (H as f32 * scale) as u32;
    let mut presenter = SkiaLayerPresenter::new(width, height, scale);
    let target = new_raster_surface(width as i32, height as i32).unwrap();
    let mut target = presenter
        .present(
            frame.snapshot(),
            frame.layer_topology(),
            &geometry,
            CLEAR,
            (0.0, 0.0),
            GpuBudget::from_viewports(width, height, 8.0),
            target,
        )
        .unwrap();
    read_rgba(&mut target)
}

/// 本 crate の `SkiaLayerRasterizer` / `SkiaLayerCompositor`（trait 実装）で合成する。
fn render_layered(tree: &ElementTree, root: ElementId) -> Vec<u8> {
    let _ = root;
    render_presented_at(tree, 1.0)
}

fn assert_pixels_equal(full: &[u8], layered: &[u8], label: &str) {
    assert_pixels_with_tolerance(full, layered, label, 2);
}

fn assert_pixels_with_tolerance(full: &[u8], layered: &[u8], label: &str, tolerance: u8) {
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
        worst <= tolerance,
        "{label}: 全面 raster と Skia レイヤ合成の出力が一致しない（byte {worst_at} で {worst} 差）"
    );
}

struct FailingLayerSurfaceFactory;

impl SkiaLayerSurfaceFactory for FailingLayerSurfaceFactory {
    fn create_layer_surface(
        &mut self,
        _width: i32,
        _height: i32,
    ) -> Result<skia_safe::Surface, String> {
        Err("layer surface unavailable".to_string())
    }
}

#[derive(Default)]
struct RecordingLayerSurfaceFactory {
    allocations: Vec<(i32, i32)>,
}

impl SkiaLayerSurfaceFactory for RecordingLayerSurfaceFactory {
    fn create_layer_surface(
        &mut self,
        width: i32,
        height: i32,
    ) -> Result<skia_safe::Surface, String> {
        self.allocations.push((width, height));
        new_raster_surface(width, height).ok_or_else(|| "recording layer surface".to_string())
    }
}

#[test]
fn layer_surface_failure_is_returned_to_the_render_host() {
    let mut rasterizer = SkiaLayerRasterizer::new(W, H, 1.0);
    let graph = SceneGraph::new();
    let error = rasterizer
        .rasterize_with_layer_surface_factory(
            &mut FailingLayerSurfaceFactory,
            ElementId::from_u64(1),
            &graph,
            None,
        )
        .expect_err("surface allocation failure must escape the renderer");

    assert!(
        error.contains("layer surface unavailable"),
        "the shared transaction must preserve the adapter failure: {error}"
    );
}

#[test]
fn presenter_returns_layer_surface_failure_without_fallback() {
    let (mut tree, _boxed, _inner) = transform_tree();
    let frame = tree.commit_rendered_frame(0.0);
    let mut presenter = SkiaLayerPresenter::new(W, H, 1.0);
    let mut factory = FailingLayerSurfaceFactory;
    let error = presenter
        .present_with_layer_surface_factory(
            frame.snapshot(),
            frame.layer_topology(),
            &Default::default(),
            CLEAR,
            (0.0, 0.0),
            GpuBudget::from_viewports(W, H, 8.0),
            &mut factory,
            new_raster_surface(W as i32, H as i32).unwrap(),
        )
        .expect_err("selected Skia layer allocation failure must escape presentation");

    assert!(
        error.contains("layer surface unavailable"),
        "the shared transaction must preserve the adapter failure: {error}"
    );
}

#[test]
fn non_root_cache_uses_core_raster_bounds_at_device_scale() {
    let layer = ElementId::from_u64(42);
    let bounds = LayerRasterBounds {
        layer,
        origin_x: 10.25,
        origin_y: 20.5,
        width: 30.5,
        height: 20.25,
    };
    let mut rasterizer = SkiaLayerRasterizer::new(W * 2, H * 2, 2.0);

    rasterizer
        .rasterize_with_bounds(layer, &SceneGraph::new(), bounds, None)
        .unwrap();

    let texture = rasterizer.texture(layer).expect("bounded layer texture");
    assert_eq!((texture.width(), texture.height()), (62, 41));
    assert_eq!(texture.device_origin(), (20, 41));
    assert_eq!(rasterizer.texture_bytes(layer), 62 * 41 * 4);
}

#[test]
fn scroll_content_factory_uses_actual_layer_width_and_band_height() {
    let layer = ElementId::from_u64(43);
    let bounds = LayerRasterBounds {
        layer,
        origin_x: 10.25,
        origin_y: 0.0,
        width: 30.5,
        height: 200.0,
    };
    let mut factory = RecordingLayerSurfaceFactory::default();
    let mut rasterizer = SkiaLayerRasterizer::new(W * 2, H * 2, 2.0);

    rasterizer
        .rasterize_with_bounds_and_layer_surface_factory(
            &mut factory,
            layer,
            &SceneGraph::new(),
            bounds,
            Some(RasterBand {
                origin_y: 5.25,
                height: 50.5,
            }),
        )
        .unwrap();

    assert_eq!(factory.allocations, vec![(62, 102)]);
    let texture = rasterizer.texture(layer).expect("scroll content texture");
    assert_eq!(texture.device_origin(), (20, 10));
    assert_eq!(rasterizer.texture_bytes(layer), 62 * 102 * 4);
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

fn transform_tree_with_translucent_shadow() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let boxed = tree.element_create(1, ElementKind::View);
    tree.element_append_child(root, boxed);
    tree.set_root(root);
    tree.set_viewport(W as f32, H as f32);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(px(W as f32)),
            StyleProp::Height(px(H as f32)),
            StyleProp::Display(hayate_core::DisplayValue::Flex),
            StyleProp::AlignItems(hayate_core::AlignValue::Center),
            StyleProp::JustifyContent(hayate_core::JustifyValue::Center),
            StyleProp::BackgroundColor(Color::new(0.9, 0.85, 0.2, 1.0)),
        ],
    );
    tree.element_set_style(
        boxed,
        &[
            StyleProp::Width(px(60.0)),
            StyleProp::Height(px(60.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 1.0, 1.0, 1.0)),
            StyleProp::BoxShadow(vec![Shadow {
                offset_x: 0.0,
                offset_y: 0.0,
                blur: 20.0,
                spread: 0.0,
                color: Color::new(1.0, 0.0, 0.0, 0.3),
                inset: false,
            }]),
        ],
    );
    tree.element_set_transform(boxed, Some([1.0, 0.0, 0.0, 1.0, 5.0, 5.0]));
    tree
}

#[test]
fn bounded_shadow_restores_transform_clip_and_origin_at_dpr_1_2_3() {
    let mut tree = transform_tree_with_translucent_shadow();
    let _ = tree.render(0.0);

    for scale in [1.0, 2.0, 3.0] {
        assert_pixels_with_tolerance(
            &render_full_at(&tree, scale),
            &render_presented_at(&tree, scale),
            &format!("bounded Skia shadow at DPR {scale}"),
            3,
        );
    }
}

#[test]
fn nested_bounded_layers_match_full_raster_at_dpr_1_2_3() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let outer = tree.element_create(1, ElementKind::View);
    let inner = tree.element_create(2, ElementKind::View);
    tree.element_append_child(root, outer);
    tree.element_append_child(outer, inner);
    tree.set_root(root);
    tree.set_viewport(W as f32, H as f32);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(px(W as f32)),
            StyleProp::Height(px(H as f32)),
            StyleProp::BackgroundColor(Color::new(0.8, 0.8, 0.8, 1.0)),
        ],
    );
    tree.element_set_style(
        outer,
        &[
            StyleProp::Width(px(100.0)),
            StyleProp::Height(px(100.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 0.2, 0.8, 1.0)),
        ],
    );
    tree.element_set_transform(outer, Some([1.0, 0.0, 0.0, 1.0, 20.0, 15.0]));
    tree.element_set_style(
        inner,
        &[
            StyleProp::Width(px(40.0)),
            StyleProp::Height(px(40.0)),
            StyleProp::BackgroundColor(Color::new(0.1, 0.9, 0.2, 1.0)),
        ],
    );
    tree.element_set_transform(inner, Some([1.0, 0.0, 0.0, 1.0, 10.0, 5.0]));
    let _ = tree.render(0.0);

    for scale in [1.0, 2.0, 3.0] {
        assert_pixels_equal(
            &render_full_at(&tree, scale),
            &render_presented_at(&tree, scale),
            &format!("nested bounded Skia layers at DPR {scale}"),
        );
    }
}

#[test]
fn transform_layer_skia_composite_matches_full_raster() {
    let (mut tree, _boxed, _inner) = transform_tree();
    let root = ElementId::from_u64(0);
    let _ = tree.render(0.0);
    assert_pixels_equal(
        &render_full(&tree),
        &render_layered(&tree, root),
        "skia transform layer",
    );
}

#[test]
fn native_shared_presenter_matches_full_raster() {
    let (mut tree, _boxed, _inner) = transform_tree();
    let _ = tree.render(0.0);
    let frame = tree.committed_frame();
    let mut presenter = SkiaLayerPresenter::new(W, H, 1.0);
    let target = new_raster_surface(W as i32, H as i32).unwrap();
    let mut target = presenter
        .present(
            frame.snapshot(),
            frame.layer_topology(),
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

#[test]
fn bounded_transition_frame_matches_full_raster_while_reusing_content() {
    let (mut tree, boxed, _inner) = transform_tree();
    let mut presenter = SkiaLayerPresenter::new(W, H, 1.0);
    let budget = GpuBudget::from_viewports(W, H, 8.0);
    let frame = tree.commit_rendered_frame(0.0);
    let _ = present_committed_frame(&frame, &mut presenter, budget);

    tree.element_set_transform(boxed, Some([1.0, 0.0, 0.0, 1.0, 70.0, 45.0]));
    let frame = tree.commit_rendered_frame(16.0);
    let pixels = present_committed_frame(&frame, &mut presenter, budget);

    assert_eq!(
        presenter.last_raster_count(),
        0,
        "a transform-only transition frame must reuse bounded content"
    );
    assert_pixels_equal(
        &render_full(&tree),
        &pixels,
        "bounded transform transition frame",
    );
}

#[test]
fn presenter_accounts_actual_root_and_bounded_texture_bytes() {
    let (mut tree, _boxed, _inner) = transform_tree();
    let frame = tree.commit_rendered_frame(0.0);
    let scale = 2.0;
    let width = W * 2;
    let height = H * 2;
    let mut presenter = SkiaLayerPresenter::new(width, height, scale);
    let target = new_raster_surface(width as i32, height as i32).unwrap();
    let _target = presenter
        .present(
            frame.snapshot(),
            frame.layer_topology(),
            &Default::default(),
            CLEAR,
            (0.0, 0.0),
            GpuBudget::from_viewports(width, height, 8.0),
            target,
        )
        .unwrap();

    let bounded_bytes: u64 = frame
        .layer_topology()
        .raster_bounds()
        .iter()
        .filter(|bounds| {
            Some(bounds.layer) != frame.layer_topology().paint_order().first().copied()
        })
        .map(|bounds| {
            let left = (bounds.origin_x * scale).floor() as i32;
            let top = (bounds.origin_y * scale).floor() as i32;
            let right = ((bounds.origin_x + bounds.width) * scale).ceil() as i32;
            let bottom = ((bounds.origin_y + bounds.height) * scale).ceil() as i32;
            (right - left).max(1) as u64 * (bottom - top).max(1) as u64 * 4
        })
        .sum();
    let expected = u64::from(width) * u64::from(height) * 4 + bounded_bytes;

    assert_eq!(presenter.cached_texture_bytes(), expected);
    assert_eq!(presenter.last_raster_pixels(), expected / 4);
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
        &[
            StyleProp::Width(px(W as f32)),
            StyleProp::Height(px(H as f32)),
        ],
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
    frame: &CommittedFrame,
    presenter: &mut SkiaLayerPresenter,
    budget: GpuBudget,
) -> Vec<u8> {
    let geometry = scroll_layer_geometry_from_inputs(frame.scroll_inputs());
    let target = new_raster_surface(W as i32, H as i32).unwrap();
    let mut target = presenter
        .present(
            frame.snapshot(),
            frame.layer_topology(),
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
        frame.layer_topology().content_changed().is_empty(),
        "pure scroll must keep both the root and scroll content caches clean"
    );
    assert!(
        frame.layer_topology().chrome_changed().contains(&scroll),
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
fn scroll_cache_budget_counts_actual_content_and_chrome_surfaces() {
    let (mut tree, _scroll) = scroll_tree();
    let frame = tree.commit_rendered_frame(0.0);
    let geometry = scroll_layer_geometry_from_inputs(frame.scroll_inputs());
    let mut presenter = SkiaLayerPresenter::new(W, H, 1.0);
    let mut factory = RecordingLayerSurfaceFactory::default();
    let target = new_raster_surface(W as i32, H as i32).unwrap();
    let _target = presenter
        .present_with_layer_surface_factory(
            frame.snapshot(),
            frame.layer_topology(),
            &geometry,
            CLEAR,
            (0.0, 0.0),
            GpuBudget::from_viewports(W, H, 8.0),
            &mut factory,
            target,
        )
        .unwrap();

    let allocated_bytes: u64 = factory
        .allocations
        .iter()
        .map(|(width, height)| *width as u64 * *height as u64 * 4)
        .sum();
    assert_eq!(presenter.cached_texture_bytes(), allocated_bytes);
}

#[test]
fn native_presenter_moves_the_cached_texture_during_ios_rubber_overscroll() {
    let (mut tree, scroll) = scroll_tree();
    let mut presenter = SkiaLayerPresenter::new(W, H, 1.0);
    let budget = GpuBudget::from_viewports(W, H, 8.0);

    let frame = tree.commit_rendered_frame(0.0);
    let _ = present_committed_frame(&frame, &mut presenter, budget);
    let (_, max_y) = tree.element_scroll_max_offset(scroll);

    // Cache the bottom-edge band first. The next overscroll frame must reuse this texture.
    tree.element_set_scroll_offset(scroll, 0.0, max_y);
    let frame = tree.commit_rendered_frame(16.0);
    let edge_pixels = present_committed_frame(&frame, &mut presenter, budget);

    tree.element_set_scroll_offset(scroll, 0.0, max_y + 120.0);
    let frame = tree.commit_rendered_frame(32.0);
    let overscroll_pixels = present_committed_frame(&frame, &mut presenter, budget);

    assert_eq!(
        presenter.last_raster_count(),
        0,
        "rubber overscroll must remain composite-only"
    );
    assert_ne!(
        edge_pixels, overscroll_pixels,
        "the cached edge texture must visibly move during rubber overscroll"
    );
    assert_pixels_equal(
        &render_full(&tree),
        &overscroll_pixels,
        "composite-only iOS rubber overscroll",
    );
}

#[test]
fn android_overscroll_projects_stretch_into_the_compositor_affine() {
    let (mut tree, scroll) = scroll_tree();
    tree.set_scroll_profile(hayate_core::scroll::ScrollPhysicsProfile::Android);
    let mut presenter = SkiaLayerPresenter::new(W, H, 1.0);
    let budget = GpuBudget::from_viewports(W, H, 8.0);

    let frame = tree.commit_rendered_frame(0.0);
    let _ = present_committed_frame(&frame, &mut presenter, budget);
    let (_, max_y) = tree.element_scroll_max_offset(scroll);
    tree.element_set_scroll_offset(scroll, 0.0, max_y);
    let frame = tree.commit_rendered_frame(16.0);
    let _ = present_committed_frame(&frame, &mut presenter, budget);

    tree.element_set_scroll_offset(scroll, 0.0, max_y + 120.0);
    let frame = tree.commit_rendered_frame(32.0);
    let input = frame
        .scroll_inputs()
        .iter()
        .find(|input| input.layer == scroll)
        .expect("scroll compositor input");
    assert!(
        input.scroll_affine[3] > 1.0,
        "Android edge motion must reach the compositor as a Y stretch: {:?}",
        input.scroll_affine
    );
    let _ = present_committed_frame(&frame, &mut presenter, budget);
    assert_eq!(
        presenter.last_raster_count(),
        0,
        "Android edge stretch must remain composite-only"
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
        frame.layer_topology().content_changed().is_empty(),
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
        frame.layer_topology().content_changed().contains(&scroll),
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
        frame.layer_topology().content_changed().contains(&scroll),
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
        frame.layer_topology().content_changed().is_empty(),
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
fn pump(
    tree: &mut ElementTree,
    planner: &mut PresentPlanner,
    rz: &mut SkiaLayerRasterizer,
    ts: f64,
) -> usize {
    let _ = tree.render(ts);
    let frame = tree.committed_frame();
    let topology = frame.layer_topology();
    let plan = planner.plan_layers(topology.paint_order(), topology.content_changed());
    for &layer in &plan.raster {
        let Some(scene) = LayerScene::new(
            frame.snapshot().clone(),
            topology.clone(),
            layer,
            LayerSceneKind::Content,
        ) else {
            continue;
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
    assert!(
        pump(&mut tree, &mut planner, &mut rz, 0.0) > 0,
        "cold フレームは raster"
    );

    for frame in 1..=4 {
        tree.element_set_transform(boxed, Some([1.0, 0.0, 0.0, 1.0, frame as f64 * 10.0, 0.0]));
        let rasters = pump(&mut tree, &mut planner, &mut rz, frame as f64 * 16.0);
        assert_eq!(
            rasters, 0,
            "skia: transform のみのフレーム {frame} で全面ラスタが走った"
        );
    }
}

#[test]
fn content_change_rerasters_only_the_dirty_layer_on_skia() {
    let (mut tree, boxed, inner) = transform_tree();
    let mut planner = PresentPlanner::new();
    let mut rz = SkiaLayerRasterizer::new(W, H, 1.0);
    let _ = pump(&mut tree, &mut planner, &mut rz, 0.0);

    tree.element_set_style(
        inner,
        &[StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0))],
    );
    let _ = tree.render(16.0);
    let plan = planner.plan_layers(tree.frame_layers(), tree.frame_layer_dirty());
    assert_eq!(
        plan.raster,
        vec![boxed],
        "skia: dirty レイヤ（boxed）だけ raster"
    );
    assert!(
        plan.reuse.contains(&tree.frame_layers()[0]),
        "root レイヤは reuse（キャッシュ面合成）"
    );
}
