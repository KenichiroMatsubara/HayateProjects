//! ぼかし角丸矩形プリミティブ（issue #657）。drop shadow の lowering が、シェル多数 emit ではなく
//! 第一級プリミティブ [`NodeKind::BlurredRoundedRect`] を **影1個につき1ノード** emit すること、
//! `RecordingPainter` がそれを **1 op** として記録すること、ハードシャドウ（blur なし）は従来どおり
//! `Rect` のままであることを検証する。default painter のシェル近似フォールバックのピクセル不変は
//! tiny-skia の box-shadow ゴールデン（`parity_box_shadow_drop`）が別途担保する。

use hayate_core::{
    Color, Dimension, DrawOp, ElementKind, ElementTree, NodeKind, RecordingPainter, Shadow,
    ShadowOccluder, StyleProp, render_scene_graph,
};

fn tree_with_shadow(shadow: Shadow) -> ElementTree {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(50.0)),
            StyleProp::Height(Dimension::px(50.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 1.0, 1.0, 1.0)),
            StyleProp::BorderRadius(8.0),
            StyleProp::BoxShadow(vec![shadow]),
        ],
    );
    tree
}

fn blurred_shadow() -> Shadow {
    Shadow {
        offset_x: 8.0,
        offset_y: 8.0,
        blur: 6.0,
        spread: 0.0,
        color: Color::new(0.0, 0.0, 0.0, 0.5),
        inset: false,
    }
}

type BlurredNode = (f32, f32, f32, f32, f32, f32, [f32; 4], Option<ShadowOccluder>);

fn blurred_rect_nodes(tree: &ElementTree) -> Vec<BlurredNode> {
    tree.scene_graph()
        .iter()
        .filter_map(|(_, n)| match n.kind {
            NodeKind::BlurredRoundedRect {
                x,
                y,
                width,
                height,
                corner_radius,
                std_dev,
                color,
                occluder,
            } => Some((x, y, width, height, corner_radius, std_dev, color, occluder)),
            _ => None,
        })
        .collect()
}

#[test]
fn blurred_drop_shadow_lowers_to_a_single_primitive_node() {
    let mut tree = tree_with_shadow(blurred_shadow());
    tree.render(0.0);

    let blurred = blurred_rect_nodes(&tree);
    assert_eq!(
        blurred.len(),
        1,
        "a blurred drop shadow must lower to exactly one BlurredRoundedRect node, got {}",
        blurred.len()
    );

    // 外形は spread=0 なのでオフセット済みのボーダーボックス、σ = blur/2、色は不透明度適用済み。
    let (x, y, w, h, radius, std_dev, color, occluder) = blurred[0];
    assert_eq!((x, y, w, h), (8.0, 8.0, 50.0, 50.0), "shadow outline geometry");
    assert_eq!(radius, 8.0, "corner radius follows the box");
    assert_eq!(std_dev, 3.0, "std_dev is blur/2");
    assert_eq!(color, [0.0, 0.0, 0.0, 0.5], "shadow colour with opacity applied");
    // 背景が不透明（白）なので occluder = ボーダーボックス内側（border 無し = 全ボックス）を
    // AA 帯ぶん（1px）内側へ縮めた矩形。
    assert_eq!(
        occluder,
        Some(ShadowOccluder { x: 1.0, y: 1.0, width: 48.0, height: 48.0, corner_radius: 7.0 }),
        "opaque owner sets a border-box occluder inset by the AA margin"
    );
}

#[test]
fn recording_painter_records_one_op_per_blurred_shadow() {
    let mut tree = tree_with_shadow(blurred_shadow());
    tree.render(0.0);

    let mut painter = RecordingPainter::new();
    render_scene_graph(tree.scene_graph(), &mut painter);

    let blurred_ops = painter
        .ops()
        .iter()
        .filter(|op| matches!(op, DrawOp::FillBlurredRoundedRect { .. }))
        .count();
    assert_eq!(
        blurred_ops, 1,
        "one shadow must record exactly one FillBlurredRoundedRect op (not {} shells)",
        blurred_ops
    );
}

fn tree_with_bg_and_shadow(bg: Option<Color>) -> ElementTree {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    let mut props = vec![
        StyleProp::Width(Dimension::px(50.0)),
        StyleProp::Height(Dimension::px(50.0)),
        StyleProp::BoxShadow(vec![blurred_shadow()]),
    ];
    if let Some(c) = bg {
        props.push(StyleProp::BackgroundColor(c));
    }
    tree.element_set_style(root, &props);
    tree
}

#[test]
fn semi_transparent_owner_gets_no_occluder() {
    // 半透明背景では影が透けるので occluder を付けない（全面塗り・回帰なし、issue #659）。
    let mut tree = tree_with_bg_and_shadow(Some(Color::new(1.0, 1.0, 1.0, 0.5)));
    tree.render(0.0);
    let (.., occluder) = blurred_rect_nodes(&tree)[0];
    assert_eq!(occluder, None, "a translucent owner must not occlude its shadow");
}

#[test]
fn transparent_owner_gets_no_occluder() {
    // 背景色なし（透明）でも occluder は付かない。
    let mut tree = tree_with_bg_and_shadow(None);
    tree.render(0.0);
    let (.., occluder) = blurred_rect_nodes(&tree)[0];
    assert_eq!(occluder, None, "a background-less owner must not occlude its shadow");
}

fn inset_tree(blur: f32) -> ElementTree {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(60.0)),
            StyleProp::Height(Dimension::px(60.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 1.0, 1.0, 1.0)),
            StyleProp::BorderRadius(12.0),
            StyleProp::BoxShadow(vec![Shadow {
                offset_x: 0.0,
                offset_y: 0.0,
                blur,
                spread: 2.0,
                color: Color::new(0.0, 0.0, 0.0, 0.6),
                inset: true,
            }]),
        ],
    );
    tree
}

fn count_kind(tree: &ElementTree, pred: impl Fn(&NodeKind) -> bool) -> usize {
    tree.scene_graph().iter().filter(|(_, n)| pred(&n.kind)).count()
}

#[test]
fn blurred_inset_shadow_lowers_to_one_primitive_clipped_to_the_border_box() {
    let mut tree = inset_tree(8.0);
    tree.render(0.0);

    let inset_nodes = count_kind(&tree, |k| matches!(k, NodeKind::InsetBlurredRoundedRect { .. }));
    assert_eq!(inset_nodes, 1, "a blurred inset shadow lowers to exactly one primitive node");
    // シェルの `RoundedRing` は出さない（10 段から 1 ノードへ削減、issue #660）。
    let rings = count_kind(&tree, |k| matches!(k, NodeKind::RoundedRing { .. }));
    assert_eq!(rings, 0, "the blurred inset must not fall back to shell rings");

    // painter からは border-box クリップの中で 1 op として見える。
    let mut painter = RecordingPainter::new();
    render_scene_graph(tree.scene_graph(), &mut painter);
    let ops = painter.ops();
    let inset_pos = ops
        .iter()
        .position(|op| matches!(op, DrawOp::FillInsetBlurredRoundedRect { .. }))
        .expect("one inset blurred op");
    assert_eq!(
        ops.iter().filter(|op| matches!(op, DrawOp::FillInsetBlurredRoundedRect { .. })).count(),
        1,
        "exactly one inset op per shadow"
    );
    // 直前に border-box クリップが積まれ、直後に外される。
    assert!(
        ops[..inset_pos].iter().rev().any(|op| matches!(op, DrawOp::PushClipRect { .. })),
        "the inset shadow is clipped to the border-box"
    );
    assert!(
        ops[inset_pos + 1..].iter().any(|op| matches!(op, DrawOp::PopClip)),
        "the border-box clip is popped after the inset shadow"
    );
}

#[test]
fn hard_inset_shadow_stays_a_rounded_ring() {
    // ぼかしなし（blur=0）のハード inset は従来どおり `RoundedRing`（ピクセル不変）。
    let mut tree = inset_tree(0.0);
    tree.render(0.0);
    assert_eq!(
        count_kind(&tree, |k| matches!(k, NodeKind::InsetBlurredRoundedRect { .. })),
        0,
        "a hard inset shadow must not use the blurred primitive"
    );
    assert!(
        count_kind(&tree, |k| matches!(k, NodeKind::RoundedRing { .. })) >= 1,
        "a hard inset shadow is drawn as a rounded ring"
    );
}

#[test]
fn hard_drop_shadow_stays_a_plain_rect() {
    // ぼかしなし（blur=0）のハードシャドウはプリミティブ化せず、従来どおり 1 枚の Rect。
    let mut tree = tree_with_shadow(Shadow {
        blur: 0.0,
        ..blurred_shadow()
    });
    tree.render(0.0);

    assert!(
        blurred_rect_nodes(&tree).is_empty(),
        "a hard (blur=0) shadow must not emit a BlurredRoundedRect"
    );
}
