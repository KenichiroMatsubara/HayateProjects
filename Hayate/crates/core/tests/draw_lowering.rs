//! draw display list の scene lowering（#724 / ADR-0141）。
//!
//! wire で decode された `DrawCommand` 列が retained scene に lowering され、
//! background → border → **draw** → children の順で、ボーダーボックス左上原点
//! （論理 px）へ平行移動されて painter に届くことを外形（RecordingPainter の
//! 観測列）だけで検証する。

use std::sync::Arc;

use hayate_core::{
    Color, Dimension, DrawCommand, DrawOp, DrawPaint, ElementKind, ElementTree, PathVerb,
    RecordingPainter, StyleProp, render_scene_graph,
};

fn triangle(color: [f32; 4]) -> Vec<DrawCommand> {
    vec![DrawCommand::FillPath {
        verbs: vec![
            PathVerb::MoveTo { x: 10.0, y: 10.0 },
            PathVerb::LineTo { x: 90.0, y: 10.0 },
            PathVerb::LineTo { x: 50.0, y: 70.0 },
            PathVerb::Close,
        ],
        paint: DrawPaint {
            color,
            ..Default::default()
        },
    }]
}

fn recorded_ops(tree: &ElementTree) -> Vec<DrawOp> {
    let mut painter = RecordingPainter::new();
    render_scene_graph(tree.scene_graph(), &mut painter);
    painter.into_ops()
}

// 背景付き view の draw は background の後・children の前に、要素のボーダーボックス
// 原点（親 padding 分だけずれた絶対座標）で FillPath として描かれる。
#[test]
fn draw_paints_after_background_at_border_box_origin() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(root, &[StyleProp::Padding(Dimension::px(20.0))]);

    let child = tree.element_create(2, ElementKind::View);
    tree.element_append_child(root, child);
    tree.element_set_style(
        child,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(80.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
        ],
    );

    let grandchild = tree.element_create(3, ElementKind::View);
    tree.element_append_child(child, grandchild);
    tree.element_set_style(
        grandchild,
        &[
            StyleProp::Width(Dimension::px(10.0)),
            StyleProp::Height(Dimension::px(10.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0)),
        ],
    );

    tree.element_set_draw(child, triangle([0.0, 0.0, 1.0, 1.0]));
    tree.render(0.0);

    let ops = recorded_ops(&tree);
    let bg_index = ops
        .iter()
        .position(|op| matches!(op, DrawOp::FillRect { color: [1.0, 0.0, 0.0, 1.0], .. }))
        .expect("child background fill");
    let draw_index = ops
        .iter()
        .position(|op| matches!(op, DrawOp::FillPath { .. }))
        .expect("draw display list fill");
    let child_bg_index = ops
        .iter()
        .position(|op| matches!(op, DrawOp::FillRect { color: [0.0, 1.0, 0.0, 1.0], .. }))
        .expect("grandchild background fill");
    assert!(
        bg_index < draw_index && draw_index < child_bg_index,
        "paint order must be background → draw → children, got {ops:?}"
    );

    match &ops[draw_index] {
        DrawOp::FillPath { x, y, verbs, fill_rule, color } => {
            assert_eq!((*x, *y), (20.0, 20.0), "border-box origin (parent padding)");
            assert_eq!(*fill_rule, hayate_core::DrawFillRule::NonZero, "default fill rule");
            assert_eq!(color, &[0.0, 0.0, 1.0, 1.0]);
            assert_eq!(
                verbs,
                &vec![
                    PathVerb::MoveTo { x: 10.0, y: 10.0 },
                    PathVerb::LineTo { x: 90.0, y: 10.0 },
                    PathVerb::LineTo { x: 50.0, y: 70.0 },
                    PathVerb::Close,
                ],
                "verbs stay border-box relative; the painter applies the origin"
            );
        }
        other => panic!("expected FillPath, got {other:?}"),
    }
}

// Draw Carrier（carriesDraw）: view 以外への draw は no-op（carrier 文化の踏襲）。
#[test]
fn draw_on_non_carrier_kind_is_a_no_op() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    let text = tree.element_create(2, ElementKind::Text);
    tree.element_append_child(root, text);
    tree.element_set_text(text, "hello");

    tree.element_set_draw(text, triangle([0.0, 0.0, 1.0, 1.0]));
    tree.render(0.0);

    assert!(
        !recorded_ops(&tree)
            .iter()
            .any(|op| matches!(op, DrawOp::FillPath { .. })),
        "draw on a text element must not paint"
    );
}

// draw の差し替えは retained scene に反映される（visual dirty 経由の再 lowering）。
#[test]
fn replacing_the_draw_list_updates_the_retained_scene() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    tree.element_set_draw(root, triangle([0.0, 0.0, 1.0, 1.0]));
    tree.render(0.0);

    tree.element_set_draw(root, triangle([1.0, 1.0, 0.0, 1.0]));
    tree.render(16.0);

    let colors: Vec<[f32; 4]> = recorded_ops(&tree)
        .iter()
        .filter_map(|op| match op {
            DrawOp::FillPath { color, .. } => Some(*color),
            _ => None,
        })
        .collect();
    assert_eq!(colors, vec![[1.0, 1.0, 0.0, 1.0]], "old list replaced, not appended");
}

// 空リストへの差し替えで描画が消える（空 = 描画なしの縮退）。
#[test]
fn clearing_the_draw_list_removes_the_paint() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_draw(root, triangle([0.0, 0.0, 1.0, 1.0]));
    tree.render(0.0);

    tree.element_set_draw(root, Vec::new());
    tree.render(16.0);

    assert!(
        !recorded_ops(&tree)
            .iter()
            .any(|op| matches!(op, DrawOp::FillPath { .. })),
        "cleared draw list must stop painting"
    );
}

// Arc 共有の確認: 同じ display list を StyleProp::Draw として複数要素へ渡せる。
#[test]
fn shared_display_list_paints_on_both_views() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(300.0, 300.0);

    let shared = Arc::new(triangle([0.0, 0.0, 1.0, 1.0]));
    for id in [2u64, 3u64] {
        let v = tree.element_create(id, ElementKind::View);
        tree.element_append_child(root, v);
        tree.element_set_style(
            v,
            &[
                StyleProp::Width(Dimension::px(100.0)),
                StyleProp::Height(Dimension::px(100.0)),
                StyleProp::Draw(shared.clone()),
            ],
        );
    }
    tree.render(0.0);

    let count = recorded_ops(&tree)
        .iter()
        .filter(|op| matches!(op, DrawOp::FillPath { .. }))
        .count();
    assert_eq!(count, 2);
}
