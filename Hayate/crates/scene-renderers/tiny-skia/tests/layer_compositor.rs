//! CPU レイヤ rasterizer / compositor の出力パリティ + work-count 契約（#636・ADR-0125 web CPU 経路）。
//!
//! - パリティ: 「各レイヤを Pixmap へ raster → placement quad で `draw_pixmap` 合成」した結果が
//!   「従来の全面 `render_scene`」とピクセル一致する（transform レイヤ / scroll 内レイヤ）。
//! - work-count: 同一 planning（`PresentPlanner`）で composite-only フレーム（transform 係数だけの
//!   変化・clean フレーム）は raster 0 回、内容変化は dirty レイヤだけ raster する。
//!
//! wgpu 経路（vello/tests/layer_compositor.rs）と同じ契約を CPU 実装（trait 差し替え）で受ける。

use std::collections::HashSet;

use hayate_core::element::style::{Dimension, Shadow, StyleProp};
use hayate_core::{Color, ElementId, ElementKind, ElementTree};
use hayate_layer_compositor::layer_scene::{
    collect_layer_placements, extract_layer_scene, extract_root_scene,
};
use hayate_layer_compositor::{CompositeQuad, LayerCompositor, LayerRasterizer, PresentPlanner};
use hayate_scene_renderer_tiny_skia::{
    TinySkiaCompositeTarget, TinySkiaLayerCompositor, TinySkiaLayerRasterizer, TinySkiaSceneRenderer,
};
use tiny_skia::Pixmap;

const W: u32 = 200;
const H: u32 = 200;
const CLEAR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

fn px(v: f32) -> Dimension {
    Dimension::px(v)
}

fn render_full(tree: &ElementTree) -> Pixmap {
    let mut pixmap = Pixmap::new(W, H).unwrap();
    TinySkiaSceneRenderer::new().render_scene(tree.scene_graph(), &mut pixmap, CLEAR, 1.0);
    pixmap
}

/// 本 crate の `TinySkiaLayerRasterizer` / `TinySkiaLayerCompositor`（trait 実装）で合成する。
fn render_layered(tree: &ElementTree, root: ElementId) -> Pixmap {
    let graph = tree.scene_graph();
    let boundaries: HashSet<ElementId> = tree.frame_layers().iter().copied().collect();
    let placements = collect_layer_placements(graph, root, &boundaries);

    let mut rasterizer = TinySkiaLayerRasterizer::new(W, H, 1.0);
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

    let quads: Vec<CompositeQuad<'_, Pixmap>> = placements
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

    let mut compositor = TinySkiaLayerCompositor::new(1.0);
    let mut target = TinySkiaCompositeTarget {
        pixmap: Pixmap::new(W, H).unwrap(),
        clear: CLEAR,
    };
    compositor.composite(&mut target, &quads).unwrap();
    target.pixmap
}

fn assert_pixmaps_equal(full: &Pixmap, layered: &Pixmap, label: &str) {
    // クリップ境界の AA 合成順だけは分解で ±数値ずれる（layer_scene_parity と同じ oracle）。
    let mut worst = 0u8;
    let mut worst_at = 0usize;
    for (i, (a, b)) in full.data().iter().zip(layered.data().iter()).enumerate() {
        let d = a.abs_diff(*b);
        if d > worst {
            worst = d;
            worst_at = i;
        }
    }
    assert!(
        worst <= 2,
        "{label}: 全面 raster と CPU レイヤ合成の出力が一致しない（byte {worst_at} で {worst} 差）"
    );
}

fn transform_tree() -> (ElementTree, ElementId, ElementId) {
    // root(灰) > boxed(青 50x50, translate(30,20)) > inner(緑 20x20)。
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
fn transform_layer_cpu_composite_matches_full_raster() {
    let (mut tree, _boxed, _inner) = transform_tree();
    let root = ElementId::from_u64(0);
    let _ = tree.render(0.0);
    assert_pixmaps_equal(&render_full(&tree), &render_layered(&tree, root), "cpu transform layer");
}

/// #699 追加確認用（vello/wgpu 経路の premultiplied/straight alpha 取り違えバグが CPU 経路にも
/// あるか）: transform でレイヤ化した要素に非黒・半透明（alpha 0.3）の box-shadow を持たせる。
/// 黒（0,0,0）は straight/premultiplied どちらで解釈しても src 項が 0 のままで差が出ないため、
/// 非黒の彩度ある色が必須（vello 側の回帰テストで実際に検出漏れを確認済み）。root 中央に配置し
/// （flex center）、shadow の blur 到達域（reach ≈ 54px）が要素の pre-transform 位置（transform
/// 前の layout 位置）から見て画面内に収まるようにする——`boxed` を原点付近に置くと
/// `layer_scene.rs` が明記する既知の v1 制限（「texture 前の座標がビューポート外にある内容は
/// texture に載らない」）を誤って踏み、premultiplied/straight とは別問題を計測してしまう。
fn transform_tree_with_translucent_shadow() -> (ElementTree, ElementId) {
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
    (tree, boxed)
}

#[test]
fn translucent_box_shadow_layer_cpu_composite_matches_full_raster() {
    let (mut tree, _boxed) = transform_tree_with_translucent_shadow();
    let root = ElementId::from_u64(0);
    let _ = tree.render(0.0);
    assert_pixmaps_equal(
        &render_full(&tree),
        &render_layered(&tree, root),
        "cpu translucent box-shadow layer (issue #699)",
    );
}

#[test]
fn layer_inside_scroll_container_cpu_composite_matches_full_raster() {
    // scroll(150x100, 内容 300) の中の transform レイヤ。スクロール済み状態でも一致する。
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let scroll = tree.element_create(1, ElementKind::ScrollView);
    let filler = tree.element_create(2, ElementKind::View);
    let moving = tree.element_create(3, ElementKind::View);
    tree.element_append_child(root, scroll);
    tree.element_append_child(scroll, filler);
    tree.element_append_child(scroll, moving);
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
    tree.element_set_style(scroll, &[StyleProp::Width(px(150.0)), StyleProp::Height(px(100.0))]);
    tree.element_set_style(
        filler,
        &[
            StyleProp::Width(px(150.0)),
            StyleProp::Height(px(120.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 0.5, 0.0, 1.0)),
        ],
    );
    tree.element_set_style(
        moving,
        &[
            StyleProp::Width(px(40.0)),
            StyleProp::Height(px(40.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
        ],
    );
    tree.element_set_transform(moving, Some([1.0, 0.0, 0.0, 1.0, 20.0, 0.0]));
    tree.element_set_scroll_offset(scroll, 0.0, 50.0);
    let _ = tree.render(0.0);

    assert_pixmaps_equal(
        &render_full(&tree),
        &render_layered(&tree, root),
        "cpu layer inside scrolled container",
    );
}

/// per-layer present を 1 フレーム回し、raster したレイヤ数を返す（work-count 用）。
fn pump(tree: &mut ElementTree, planner: &mut PresentPlanner, rz: &mut TinySkiaLayerRasterizer, ts: f64) -> usize {
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
fn transform_only_frames_do_not_raster_on_cpu() {
    // AC: composite-only フレーム（transform 係数だけの変化）で全面 render_scene（レイヤ raster）が
    // 走らない。scroll フレームも同型（frame_layer_dirty が空）。
    let (mut tree, boxed, _inner) = transform_tree();
    let mut planner = PresentPlanner::new();
    let mut rz = TinySkiaLayerRasterizer::new(W, H, 1.0);
    assert!(pump(&mut tree, &mut planner, &mut rz, 0.0) > 0, "cold フレームは raster");

    for frame in 1..=4 {
        tree.element_set_transform(boxed, Some([1.0, 0.0, 0.0, 1.0, frame as f64 * 10.0, 0.0]));
        let rasters = pump(&mut tree, &mut planner, &mut rz, frame as f64 * 16.0);
        assert_eq!(rasters, 0, "CPU: transform のみのフレーム {frame} で全面ラスタが走った");
    }
}

#[test]
fn content_change_rerasters_only_the_dirty_layer_on_cpu() {
    let (mut tree, boxed, inner) = transform_tree();
    let mut planner = PresentPlanner::new();
    let mut rz = TinySkiaLayerRasterizer::new(W, H, 1.0);
    let _ = pump(&mut tree, &mut planner, &mut rz, 0.0);

    tree.element_set_style(inner, &[StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0))]);
    let _ = tree.render(16.0);
    let plan = planner.plan_layers(tree.frame_layers(), tree.frame_layer_dirty());
    assert_eq!(plan.raster, vec![boxed], "CPU: dirty レイヤ（boxed）だけ raster");
    assert!(plan.reuse.contains(&tree.frame_layers()[0]), "root レイヤは reuse（キャッシュ Pixmap 合成）");
}
