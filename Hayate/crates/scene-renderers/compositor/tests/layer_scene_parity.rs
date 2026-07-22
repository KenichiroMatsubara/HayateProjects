//! Zero-copy Layer Scene + retained Layer Presentation output parity.
//!
//! 「レイヤを個別 texture に raster し、quad（transform/clip 付き）で合成した結果」が「従来の
//! 全面 raster」とピクセル一致することを、CPU（tiny-skia）でホスト固定する。wgpu compositor は
//! 同じ `LayerPlacement` / 抽出シーンを消費するだけなので、この分解パリティが合成の正しさの正本。
//! 整数 translate のみを使う（回転はリサンプリング差が出る既知制限・ADR-0125 の焼き込み系）。

use hayate_core::element::style::{Dimension, StyleProp};
use hayate_core::{Color, ElementId, ElementKind, ElementTree, LayerScene, LayerSceneKind};
use hayate_layer_compositor::{
    scroll_layer_geometry_from_inputs, LayerPresentation, LayerPresentationFrame,
};
use hayate_scene_renderer_tiny_skia::{
    TinySkiaLayerCompositor, TinySkiaLayerPresentationAdapter, TinySkiaLayerRasterizer,
    TinySkiaSceneRenderer,
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
    let frame = tree.committed_frame();
    TinySkiaSceneRenderer::new().render_scene(frame.snapshot(), &mut pixmap, CLEAR, 1.0);
    pixmap
}

/// Run the production retained presentation with zero-copy Layer Scene projections.
fn render_layered(tree: &ElementTree, _root: ElementId) -> Pixmap {
    let frame = tree.committed_frame();
    let scroll_geometry = scroll_layer_geometry_from_inputs(frame.scroll_inputs());
    let mut presentation = LayerPresentation::new();
    let mut rasterizer = TinySkiaLayerRasterizer::new(W, H, 1.0);
    let mut chrome_rasterizer = TinySkiaLayerRasterizer::new(W, H, 1.0);
    let mut compositor = TinySkiaLayerCompositor::new(1.0);
    let mut out = Pixmap::new(W, H).unwrap();
    presentation
        .present(
            LayerPresentationFrame {
                snapshot: frame.snapshot(),
                topology: frame.layer_topology(),
                scroll_geometry: &scroll_geometry,
            },
            &mut TinySkiaLayerPresentationAdapter {
                rasterizer: &mut rasterizer,
                chrome_rasterizer: &mut chrome_rasterizer,
                compositor: &mut compositor,
                target: &mut out,
                clear: CLEAR,
            },
        )
        .unwrap();
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
fn scroll_layer_placement_keeps_its_own_viewport_clip() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let header = tree.element_create(1, ElementKind::View);
    let scroll = tree.element_create(2, ElementKind::ScrollView);
    let content = tree.element_create(3, ElementKind::View);
    tree.element_append_child(root, header);
    tree.element_append_child(root, scroll);
    tree.element_append_child(scroll, content);
    tree.set_root(root);
    tree.set_viewport(W as f32, H as f32);
    tree.element_set_style(
        root,
        &[
            StyleProp::Display(hayate_core::DisplayValue::Flex),
            StyleProp::FlexDirection(hayate_core::FlexDirectionValue::Column),
        ],
    );
    tree.element_set_style(
        header,
        &[StyleProp::Width(px(W as f32)), StyleProp::Height(px(64.0))],
    );
    tree.element_set_style(
        scroll,
        &[StyleProp::Width(px(W as f32)), StyleProp::Height(px(136.0))],
    );
    tree.element_set_style(
        content,
        &[StyleProp::Width(px(W as f32)), StyleProp::Height(px(400.0))],
    );
    let _ = tree.render(0.0);

    let frame = tree.committed_frame();
    let placements = frame.layer_topology().placements();
    let placement = placements
        .iter()
        .find(|placement| placement.layer == scroll)
        .unwrap();
    assert_eq!(
        placement.clip,
        Some([0.0, 64.0, 200.0, 136.0]),
        "a translated scroll cache must remain clipped to its viewport at composite time",
    );

    let extracted = LayerScene::new(
        frame.snapshot().clone(),
        frame.layer_topology().clone(),
        scroll,
        LayerSceneKind::ScrollContent {
            scroll_affine: tree.element_scroll_group_affine(scroll),
        },
    )
    .unwrap();
    let mut painter = hayate_core::RecordingPainter::new();
    hayate_core::render_scene_graph(&extracted, &mut painter);
    assert!(
        !painter
            .ops()
            .iter()
            .any(|op| matches!(op, hayate_core::DrawOp::PushClipRect { .. })),
        "the scroll viewport clip must not be baked into an overscan cache texture",
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

    assert_pixmaps_equal(
        &render_full(&tree),
        &render_layered(&tree, root),
        "transform layer",
    );
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
    let frame = tree.committed_frame();
    let boxed_scene = LayerScene::new(
        frame.snapshot().clone(),
        frame.layer_topology().clone(),
        boxed,
        LayerSceneKind::Content,
    )
    .unwrap();
    let mut painter = hayate_core::RecordingPainter::new();
    hayate_core::render_scene_graph(&boxed_scene, &mut painter);
    let has_red = painter.ops().iter().any(|op| {
        matches!(op, hayate_core::DrawOp::FillRect { color, .. } if color[0] > 0.9 && color[2] < 0.1)
    });
    assert!(
        !has_red,
        "ネストレイヤ（赤）の内容は親レイヤ texture から除外される"
    );

    assert_pixmaps_equal(
        &render_full(&tree),
        &render_layered(&tree, root),
        "nested layers",
    );
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

    let frame = tree.committed_frame();
    let scene = LayerScene::new(
        frame.snapshot().clone(),
        frame.layer_topology().clone(),
        boxed,
        LayerSceneKind::Content,
    )
    .unwrap();
    let mut painter = hayate_core::RecordingPainter::new();
    hayate_core::render_scene_graph(&scene, &mut painter);
    let has_transform = painter
        .ops()
        .iter()
        .any(|op| matches!(op, hayate_core::DrawOp::PushTransform { .. }));
    assert!(
        !has_transform,
        "外側 transform Group は texture に焼き込まない"
    );

    // placement 側が transform を持つ。
    let placements = frame.layer_topology().placements();
    let boxed_placement = placements.iter().find(|p| p.layer == boxed).unwrap();
    assert_eq!(boxed_placement.transform, [1.0, 0.0, 0.0, 1.0, 30.0, 20.0]);
}

/// scroll(150x100) 直下に単色コンテンツ 1 枚を持つツリー。overscroll のバウンス位置で render する。
/// `profile` が iOS（素の translate）/ Android（stretch）を切り替える。
fn overscroll_tree(
    profile: hayate_core::scroll::ScrollPhysicsProfile,
    release_offset: f32,
) -> (ElementTree, ElementId, ElementId) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let scroll = tree.element_create(1, ElementKind::ScrollView);
    let content = tree.element_create(2, ElementKind::View);
    tree.element_append_child(root, scroll);
    tree.element_append_child(scroll, content);
    tree.set_root(root);
    tree.set_viewport(W as f32, H as f32);
    tree.set_scroll_profile(profile);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(px(W as f32)),
            StyleProp::Height(px(H as f32)),
            StyleProp::BackgroundColor(Color::new(0.9, 0.85, 0.2, 1.0)), // 露出する背景（黄）
        ],
    );
    tree.element_set_style(
        scroll,
        &[StyleProp::Width(px(150.0)), StyleProp::Height(px(100.0))],
    );
    tree.element_set_style(
        content,
        &[
            StyleProp::Width(px(150.0)),
            StyleProp::Height(px(400.0)), // max_y = 300
            StyleProp::BackgroundColor(Color::new(0.0, 0.3, 0.8, 1.0)),
        ],
    );
    let _ = tree.render(0.0);
    tree.element_set_scroll_offset(scroll, 0.0, release_offset);
    let _ = tree.render(16.0);
    (tree, root, scroll)
}

#[test]
fn ios_bottom_overscroll_composite_matches_full_raster() {
    // iOS プロファイル：下端を 80px 越えたバウンス位置。content は整数 translate で丸ごと上へ動き、
    // 下端に背景（黄）が露出する。合成（テクスチャ quad）と全面 raster がピクセル一致することで、
    // 「overscroll 域のカバレッジ＋背景露出が従来（全面描画）と一致」を固定する（#639 AC）。
    let (tree, root, _) = overscroll_tree(
        hayate_core::scroll::ScrollPhysicsProfile::Auto,
        300.0 + 80.0,
    );
    assert_pixmaps_equal(
        &render_full(&tree),
        &render_layered(&tree, root),
        "iOS bottom overscroll",
    );
}

#[test]
fn ios_top_overscroll_composite_matches_full_raster() {
    // iOS プロファイル：上端を 60px 越えたバウンス位置（offset < 0）。content は下へ動き、上端に
    // 背景が露出する。整数 translate なので合成と全面 raster がピクセル一致。
    let (tree, root, _) = overscroll_tree(hayate_core::scroll::ScrollPhysicsProfile::Auto, -60.0);
    assert_pixmaps_equal(
        &render_full(&tree),
        &render_layered(&tree, root),
        "iOS top overscroll",
    );
}

#[test]
fn android_stretch_bottom_overscroll_composite_matches_full_raster() {
    // Android stretch プロファイル（ADR-0131）：下端を 80px 越えたバウンス位置。越境は scroll Group の
    // 一様スケール（scale_y > 1・端ピン）に現れる。レイヤ合成はその Group を含む sub-scene をベクタ
    // 再 raster するので、全面 raster と同じ crisp なスケール結果になりピクセル一致する（テクスチャ
    // 拡大のリサンプリングではない）。stretch profile の合成出力が従来の全面描画と一致することを固定。
    let (tree, root, scroll) = overscroll_tree(
        hayate_core::scroll::ScrollPhysicsProfile::Android,
        300.0 + 80.0,
    );
    // 越境がスケールとして affine に載っていることを確認（profile が効いている前提の担保）。
    let affine = tree.element_scroll_group_affine(scroll);
    assert!(
        affine[3] > 1.0,
        "Android 越境は一様 stretch scale（scale_y > 1）: {affine:?}"
    );
    assert_pixmaps_equal(
        &render_full(&tree),
        &render_layered(&tree, root),
        "Android stretch bottom overscroll",
    );
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
