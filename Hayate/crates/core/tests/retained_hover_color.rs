//! ユーザー報告の症状に対する差分ハーネス: hover の色が戻らず、ある要素を操作すると
//! 別の要素が壊れる。正は full な ephemeral rebuild。各ポインタ変更後、retained シーンの
//! fill rect（色＋位置、描画順）が一致しなければならない。

use hayate_core::{
    Color, Dimension, DrawOp, ElementId, ElementKind, ElementTree, FlexDirectionValue, PseudoState,
    RecordingPainter, StyleProp, render_scene_graph,
};

type Rects = Vec<([f32; 4], f32, f32, f32, f32)>;

fn project(ops: Vec<DrawOp>) -> Rects {
    ops.into_iter()
        .filter_map(|op| match op {
            DrawOp::FillRect {
                x,
                y,
                width,
                height,
                color,
                ..
            } => Some((color, x, y, width, height)),
            _ => None,
        })
        .collect()
}

fn retained_rects(tree: &ElementTree) -> Rects {
    let mut painter = RecordingPainter::new();
    render_scene_graph(tree.scene_graph(), &mut painter);
    project(painter.into_ops())
}

fn ephemeral_rects(tree: &ElementTree) -> Rects {
    project(tree.test_scene_full_rebuild_draw_ops())
}

fn assert_parity(tree: &ElementTree, label: &str) {
    assert_eq!(
        retained_rects(tree),
        ephemeral_rects(tree),
        "retained scene diverged from ephemeral rebuild after {label}"
    );
}

const BLUE: Color = Color::new(0.0, 0.0, 1.0, 1.0);
const GREEN: Color = Color::new(0.0, 1.0, 0.0, 1.0);

/// root(col) > [cardA, cardB, cardC]、各 card は blue、:hover で green。
fn three_cards() -> (ElementTree, Vec<ElementId>) {
    let mut next = 1u64;
    let mut tree = ElementTree::new();
    let root = tree.element_create(next, ElementKind::View);
    next += 1;
    tree.set_root(root);
    tree.set_viewport(200.0, 400.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(400.0)),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
        ],
    );
    let mut cards = Vec::new();
    for _ in 0..3 {
        let c = tree.element_create(next, ElementKind::View);
        next += 1;
        tree.element_set_style(
            c,
            &[
                StyleProp::Width(Dimension::px(80.0)),
                StyleProp::Height(Dimension::px(20.0)),
                StyleProp::BackgroundColor(BLUE),
            ],
        );
        tree.element_set_pseudo_style(
            c,
            PseudoState::Hover,
            &[StyleProp::BackgroundColor(GREEN)],
        );
        tree.element_append_child(root, c);
        cards.push(c);
    }
    (tree, cards)
}

#[test]
fn hover_then_leave_reverts_color() {
    let (mut tree, cards) = three_cards();
    tree.render(0.0);
    assert_parity(&tree, "initial");

    tree.update_pointer_hover(Some(cards[1]));
    tree.render(16.0);
    assert_parity(&tree, "hover middle card");

    tree.update_pointer_hover(None);
    tree.render(32.0);
    assert_parity(&tree, "leave");
}

#[test]
fn hovering_one_card_does_not_reorder_or_recolor_siblings() {
    let (mut tree, cards) = three_cards();
    tree.render(0.0);

    // 各 card を順に hover する。シーンは描画順を含め常に正と完全一致しなければならない
    // （anchor の並び替えは「誤った要素が変わった」バグ）。
    for (i, &c) in cards.iter().enumerate() {
        tree.update_pointer_hover(Some(c));
        tree.render((i as f64 + 1.0) * 16.0);
        assert_parity(&tree, "hover card");
        tree.update_pointer_hover(None);
        tree.render((i as f64 + 1.0) * 16.0 + 8.0);
        assert_parity(&tree, "unhover card");
    }
}
