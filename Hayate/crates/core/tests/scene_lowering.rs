//! Retained incremental scene lowering + Element Anchor (issue #182).

use hayate_core::{
    Color, Dimension, DrawOp, ElementKind, ElementTree, NodeKind, RecordingPainter,
    StyleProp, render_scene_graph,
};

fn draw_ops(tree: &ElementTree) -> Vec<DrawOp> {
    let sg = tree.scene_graph();
    let mut painter = RecordingPainter::new();
    render_scene_graph(sg, &mut painter);
    painter.into_ops()
}

fn simple_view_tree() -> (ElementTree, hayate_core::ElementId) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(50.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
        ],
    );
    (tree, root)
}

#[test]
fn clean_frame_performs_no_lowering_walks() {
    let (mut tree, _) = simple_view_tree();
    tree.render(0.0);
    assert!(tree.test_scene_lowering_built());
    tree.render(0.0);
    assert_eq!(tree.test_scene_lowering_walk_count(), 0);
}

#[test]
fn incremental_draw_ops_match_full_rebuild() {
    let (mut tree, root) = simple_view_tree();
    tree.render(0.0);

    tree.element_set_style(
        root,
        &[StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0))],
    );
    tree.render(0.0);
    let incremental_ops = draw_ops(&tree);

    let reference_ops = tree.test_scene_full_rebuild_draw_ops();
    assert_eq!(incremental_ops.len(), reference_ops.len());
    for (got, expected) in incremental_ops.iter().zip(reference_ops.iter()) {
        match (got, expected) {
            (
                DrawOp::FillRect {
                    x: gx,
                    y: gy,
                    width: gw,
                    height: gh,
                    color: gc,
                    corner_radius: gr,
                },
                DrawOp::FillRect {
                    x: ex,
                    y: ey,
                    width: ew,
                    height: eh,
                    color: ec,
                    corner_radius: er,
                },
            ) => {
                assert!((gx - ex).abs() < 0.01);
                assert!((gy - ey).abs() < 0.01);
                assert!((gw - ew).abs() < 0.01);
                assert!((gh - eh).abs() < 0.01);
                assert!((gr - er).abs() < 0.01);
                for (a, b) in gc.iter().zip(ec.iter()) {
                    assert!((a - b).abs() < 1e-3);
                }
            }
            _ => panic!("draw op mismatch: {got:?} vs {expected:?}"),
        }
    }
}

#[test]
fn element_anchor_stable_across_clean_frames() {
    let (mut tree, root) = simple_view_tree();
    tree.render(0.0);
    let anchor_after_first = tree.test_element_anchor_id(root);
    tree.render(0.0);
    assert_eq!(tree.test_element_anchor_id(root), anchor_after_first);
}

#[test]
fn child_visual_change_preserves_parent_anchor() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    let child = tree.element_create(2, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    tree.element_append_child(root, child);
    tree.element_set_style(
        child,
        &[
            StyleProp::Width(Dimension::px(40.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
        ],
    );
    tree.render(0.0);
    let parent_anchor = tree.test_element_anchor_id(root);

    tree.element_set_style(
        child,
        &[StyleProp::BackgroundColor(Color::new(1.0, 1.0, 0.0, 1.0))],
    );
    tree.render(0.0);

    assert_eq!(tree.test_element_anchor_id(root), parent_anchor);
    assert!(tree.test_scene_lowering_walk_count() > 0);
}

#[test]
fn scene_contains_element_anchor_nodes() {
    let (mut tree, root) = simple_view_tree();
    tree.render(0.0);
    let anchor = tree.test_element_anchor_id(root);
    let sg = tree.scene_graph();
    let node = sg.get(anchor).expect("anchor node");
    assert!(matches!(
        node.kind,
        NodeKind::ElementAnchor { element_id } if element_id == root
    ));
}
