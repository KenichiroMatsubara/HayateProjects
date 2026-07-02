//! レイヤ分解（extract + quad 合成）の出力パリティ（#633・ADR-0125 backend 半分）。
//!
//! 「レイヤを個別 texture に raster し、quad（transform/clip 付き）で合成した結果」が「従来の
//! 全面 raster」とピクセル一致することを、CPU（tiny-skia）でホスト固定する。wgpu compositor は
//! 同じ `LayerPlacement` / 抽出シーンを消費するだけなので、この分解パリティが合成の正しさの正本。
//! 整数 translate のみを使う（回転はリサンプリング差が出る既知制限・ADR-0125 の焼き込み系）。

use std::collections::HashSet;

use hayate_core::element::style::{Dimension, StyleProp};
use hayate_core::{Color, ElementId, ElementKind, ElementTree};
use hayate_layer_compositor::layer_scene::{
    collect_layer_placements, extract_layer_scene, extract_root_scene,
};
use hayate_scene_renderer_tiny_skia::TinySkiaSceneRenderer;
use tiny_skia::{Pixmap, PixmapPaint, Transform};

const W: u32 = 200;
const H: u32 = 200;
const CLEAR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const TRANSPARENT: [f32; 4] = [0.0, 0.0, 0.0, 0.0];

fn px(v: f32) -> Dimension {
    Dimension::px(v)
}

fn render_full(tree: &ElementTree) -> Pixmap {
    let mut pixmap = Pixmap::new(W, H).unwrap();
    TinySkiaSceneRenderer::new().render_scene(tree.scene_graph(), &mut pixmap, CLEAR, 1.0);
    pixmap
}

/// レイヤ分解して CPU 合成する（wgpu compositor の CompositeQuad 意味論の CPU ミラー）。
fn render_layered(tree: &ElementTree, root: ElementId) -> Pixmap {
    let graph = tree.scene_graph();
    let boundaries: HashSet<ElementId> = tree.frame_layers().iter().copied().collect();
    let placements = collect_layer_placements(graph, root, &boundaries);

    // root レイヤ（他レイヤの内容を除外した残り）を不透明背景で raster。
    let mut out = Pixmap::new(W, H).unwrap();
    let root_scene = extract_root_scene(graph, root, &boundaries);
    TinySkiaSceneRenderer::new().render_scene(&root_scene, &mut out, CLEAR, 1.0);

    // 各レイヤを透明キャッシュ面へ raster し、placement（transform/clip）で合成。
    for placement in &placements {
        if placement.layer == root {
            continue;
        }
        let Some(scene) = extract_layer_scene(graph, placement.layer, &boundaries) else {
            continue;
        };
        let mut layer_pixmap = Pixmap::new(W, H).unwrap();
        TinySkiaSceneRenderer::new().render_scene(&scene, &mut layer_pixmap, TRANSPARENT, 1.0);

        let t = placement.transform;
        let transform = Transform::from_row(
            t[0] as f32,
            t[1] as f32,
            t[2] as f32,
            t[3] as f32,
            t[4] as f32,
            t[5] as f32,
        );
        let mask = placement.clip.map(|[x, y, w, h]| {
            let mut mask = tiny_skia::Mask::new(W, H).unwrap();
            let rect = tiny_skia::Rect::from_xywh(x, y, w, h).unwrap();
            let path = tiny_skia::PathBuilder::from_rect(rect);
            mask.fill_path(&path, tiny_skia::FillRule::Winding, true, Transform::identity());
            mask
        });
        out.draw_pixmap(
            0,
            0,
            layer_pixmap.as_ref(),
            &PixmapPaint::default(),
            transform,
            mask.as_ref(),
        );
    }
    out
}

fn assert_pixmaps_equal(full: &Pixmap, layered: &Pixmap, label: &str) {
    // クリップ境界の AA 合成順（mask×edge の乗算丸め）だけは分解で ±数値ずれる。内容の
    // 取り違え（配置・除外漏れ）は数十〜255 の差になるため、しきい値 2 が分解正しさの oracle。
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
        "{label}: 全面 raster とレイヤ合成の出力が一致しない（byte {worst_at} で {worst} 差）"
    );
}

#[test]
fn transform_layer_composite_matches_full_raster() {
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
    let _ = tree.render(0.0);

    assert_pixmaps_equal(&render_full(&tree), &render_layered(&tree, root), "transform layer");
}

#[test]
fn nested_transform_layers_composite_matches_full_raster() {
    // boxed(translate(30,20)) の中に inner(translate(10,10)) — 双方が独立レイヤ。
    // inner の quad transform は親レイヤの transform と合成される。
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
            StyleProp::BackgroundColor(Color::new(0.9, 0.9, 0.9, 1.0)),
        ],
    );
    tree.element_set_style(
        boxed,
        &[
            StyleProp::Width(px(80.0)),
            StyleProp::Height(px(80.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
        ],
    );
    tree.element_set_transform(boxed, Some([1.0, 0.0, 0.0, 1.0, 30.0, 20.0]));
    tree.element_set_style(
        inner,
        &[
            StyleProp::Width(px(30.0)),
            StyleProp::Height(px(30.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
        ],
    );
    tree.element_set_transform(inner, Some([1.0, 0.0, 0.0, 1.0, 10.0, 10.0]));
    let _ = tree.render(0.0);

    // 親レイヤの texture にネストレイヤの内容が焼き込まれていないこと（二重描画防止）。
    let graph = tree.scene_graph();
    let boundaries: HashSet<ElementId> = tree.frame_layers().iter().copied().collect();
    let boxed_scene = extract_layer_scene(graph, boxed, &boundaries).unwrap();
    let mut painter = hayate_core::RecordingPainter::new();
    hayate_core::render_scene_graph(&boxed_scene, &mut painter);
    let has_red = painter.ops().iter().any(|op| {
        matches!(op, hayate_core::DrawOp::FillRect { color, .. } if color[0] > 0.9 && color[2] < 0.1)
    });
    assert!(!has_red, "ネストレイヤ（赤）の内容は親レイヤ texture から除外される");

    assert_pixmaps_equal(&render_full(&tree), &render_layered(&tree, root), "nested layers");
}

#[test]
fn layer_extraction_strips_the_outer_transform_group() {
    // レイヤ texture は transform 前の座標で raster される（transform は quad が適用）。
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let boxed = tree.element_create(1, ElementKind::View);
    tree.element_append_child(root, boxed);
    tree.set_root(root);
    tree.set_viewport(W as f32, H as f32);
    tree.element_set_style(
        boxed,
        &[
            StyleProp::Width(px(50.0)),
            StyleProp::Height(px(50.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
        ],
    );
    tree.element_set_transform(boxed, Some([1.0, 0.0, 0.0, 1.0, 30.0, 20.0]));
    let _ = tree.render(0.0);

    let boundaries: HashSet<ElementId> = tree.frame_layers().iter().copied().collect();
    let scene = extract_layer_scene(tree.scene_graph(), boxed, &boundaries).unwrap();
    let mut painter = hayate_core::RecordingPainter::new();
    hayate_core::render_scene_graph(&scene, &mut painter);
    let has_transform = painter
        .ops()
        .iter()
        .any(|op| matches!(op, hayate_core::DrawOp::PushTransform { .. }));
    assert!(!has_transform, "外側 transform Group は texture に焼き込まない");

    // placement 側が transform を持つ。
    let placements = collect_layer_placements(tree.scene_graph(), root, &boundaries);
    let boxed_placement = placements.iter().find(|p| p.layer == boxed).unwrap();
    assert_eq!(boxed_placement.transform, [1.0, 0.0, 0.0, 1.0, 30.0, 20.0]);
}

#[test]
fn layer_inside_scroll_container_gets_scroll_offset_and_clip() {
    // scroll(高さ 100, 内容 300) の中の transform レイヤ。スクロール済み状態で合成しても、
    // クリップとオフセット込みで全面 raster と一致する。
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
    tree.element_set_style(
        scroll,
        &[StyleProp::Width(px(150.0)), StyleProp::Height(px(100.0))],
    );
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
        "layer inside scrolled container",
    );
}
