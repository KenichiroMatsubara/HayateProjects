//! Retained incremental lowering must not paint stale box geometry after a
//! layout reflow. Adding/removing/selecting an element ripples a flex reflow up
//! to ancestors (panels that grow) and sideways to siblings (pushed down). Those
//! reflowed-but-otherwise-clean boxes are never structure/visual-dirty on their
//! own, so before the layout->lowering geometry bridge the retained scene kept
//! their old positions/sizes while a full ephemeral rebuild used the new ones.
//!
//! The differential harness here is the ground truth: after every mutation the
//! retained `scene_graph()` fill rects MUST equal the ephemeral full-rebuild fill
//! rects. Any divergence is stale retained geometry. (The test scenes are solid
//! `View` boxes, so fill rects capture every painted element.)

use hayate_core::{
    Color, Dimension, DrawOp, ElementId, ElementKind, ElementTree, FlexDirectionValue,
    RecordingPainter, StyleProp, render_scene_graph,
};

/// (color, x, y, w, h) for every FillRect, in paint order.
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

/// Assert the retained scene matches a full ephemeral rebuild, fill-for-fill.
fn assert_parity(tree: &ElementTree, label: &str) {
    assert_eq!(
        retained_rects(tree),
        ephemeral_rects(tree),
        "retained scene diverged from ephemeral rebuild after {label}"
    );
}

fn footer_y(tree: &ElementTree) -> f32 {
    for (color, _x, y, _w, _h) in retained_rects(tree) {
        if color == [1.0, 0.0, 0.0, 1.0] {
            return y;
        }
    }
    panic!("footer rect not found");
}

fn card(tree: &mut ElementTree, next_id: &mut u64, color: Color) -> ElementId {
    let id = tree.element_create(*next_id, ElementKind::View);
    *next_id += 1;
    tree.element_set_style(
        id,
        &[
            StyleProp::Height(Dimension::px(20.0)),
            StyleProp::Width(Dimension::px(60.0)),
            StyleProp::BackgroundColor(color),
        ],
    );
    id
}

/// root(col) > [ board(col){ card } , footer ]
/// Adding a card grows `board` and pushes `footer` down. `footer` is never
/// marked dirty by the insert, so its retained box is the staleness canary.
fn studio() -> (ElementTree, u64, ElementId, ElementId, ElementId) {
    let mut next_id = 1u64;
    let mut tree = ElementTree::new();
    let root = tree.element_create(next_id, ElementKind::View);
    next_id += 1;
    tree.set_root(root);
    tree.set_viewport(200.0, 400.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(400.0)),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
            StyleProp::BackgroundColor(Color::new(0.9, 0.9, 0.9, 1.0)),
        ],
    );

    let board = tree.element_create(next_id, ElementKind::View);
    next_id += 1;
    tree.element_set_style(
        board,
        &[StyleProp::FlexDirection(FlexDirectionValue::Column)],
    );
    tree.element_append_child(root, board);

    let footer = tree.element_create(next_id, ElementKind::View);
    next_id += 1;
    tree.element_set_style(
        footer,
        &[
            StyleProp::Height(Dimension::px(30.0)),
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
        ],
    );
    tree.element_append_child(root, footer);

    let first = card(&mut tree, &mut next_id, Color::new(0.0, 0.0, 1.0, 1.0));
    tree.element_append_child(board, first);

    (tree, next_id, root, board, footer)
}

#[test]
fn footer_follows_board_growth_on_incremental_insert() {
    let (mut tree, mut next_id, _root, board, _footer) = studio();
    tree.render(0.0);
    assert_parity(&tree, "initial render");
    let y0 = footer_y(&tree);
    assert!((y0 - 20.0).abs() < 0.01, "footer starts below one card, got {y0}");

    // Incremental insert: board grows by one card height, footer must shift down.
    let c2 = card(&mut tree, &mut next_id, Color::new(0.0, 0.0, 1.0, 1.0));
    tree.element_append_child(board, c2);
    tree.render(16.0);

    assert_parity(&tree, "add second card");
    let y1 = footer_y(&tree);
    assert!(
        (y1 - 40.0).abs() < 0.01,
        "footer should be pushed to y=40 after a second card, got {y1}"
    );
}

#[test]
fn parity_holds_across_repeated_inserts_and_removes() {
    let (mut tree, mut next_id, _root, board, _footer) = studio();
    tree.render(0.0);
    assert_parity(&tree, "initial render");

    let mut cards = Vec::new();
    for _ in 0..4 {
        let c = card(&mut tree, &mut next_id, Color::new(0.0, 0.0, 1.0, 1.0));
        tree.element_append_child(board, c);
        cards.push(c);
        tree.render(16.0);
        assert_parity(&tree, "after insert");
    }

    for c in cards {
        tree.element_remove(c);
        tree.render(16.0);
        assert_parity(&tree, "after remove");
    }
}

#[test]
fn insert_before_shifts_following_sibling() {
    let (mut tree, mut next_id, _root, board, _footer) = studio();
    tree.render(0.0);
    let first_child = tree.ordered_children(board)[0];

    // Insert a card BEFORE the existing first card: the existing card slides down
    // and the footer follows. Neither is touched by the insert's own dirty mark.
    let inserted = card(&mut tree, &mut next_id, Color::new(0.0, 1.0, 0.0, 1.0));
    tree.element_insert_before(board, inserted, first_child);
    tree.render(16.0);

    assert_parity(&tree, "insert before first card");
    let y = footer_y(&tree);
    assert!((y - 40.0).abs() < 0.01, "footer pushed to y=40, got {y}");
}
