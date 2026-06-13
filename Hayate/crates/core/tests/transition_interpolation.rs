//! Issue #209: pseudo-state transitions interpolate continuous visual props
//! over `transition-duration` ms, driven by `render(timestamp_ms)`.

use hayate_core::{
    Color, Dimension, DrawOp, ElementKind, ElementTree, PseudoState, RecordingPainter, StyleProp,
    render_scene_graph,
};

fn draw_ops(tree: &ElementTree) -> Vec<DrawOp> {
    let sg = tree.scene_graph();
    let mut painter = RecordingPainter::new();
    render_scene_graph(sg, &mut painter);
    painter.into_ops()
}

/// Background colour of the first filled rect in the current scene.
fn background(tree: &ElementTree) -> [f32; 4] {
    for op in draw_ops(tree) {
        if let DrawOp::FillRect { color, .. } = op {
            return color;
        }
    }
    panic!("no FillRect in scene");
}

fn hover_box(duration_ms: f32) -> (ElementTree, hayate_core::ElementId) {
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
            StyleProp::TransitionDuration(duration_ms),
        ],
    );
    tree.element_set_pseudo_style(
        root,
        PseudoState::Hover,
        &[StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0))],
    );
    (tree, root)
}

#[test]
fn hover_transition_interpolates_background_color_over_duration() {
    let (mut tree, root) = hover_box(200.0);
    tree.render(0.0);
    assert_eq!(background(&tree)[0], 1.0, "starts fully red");

    tree.update_pointer_hover(Some(root));

    // First post-hover frame anchors the transition clock; still red.
    tree.render(100.0);
    let start = background(&tree);
    assert!((start[0] - 1.0).abs() < 1e-3 && start[1].abs() < 1e-3, "frame 0 still red");

    // Halfway through the 200ms window: between red and green.
    tree.render(200.0);
    let mid = background(&tree);
    assert!(mid[0] < 1.0 && mid[0] > 0.0, "red channel mid-transition: {}", mid[0]);
    assert!(mid[1] > 0.0 && mid[1] < 1.0, "green channel mid-transition: {}", mid[1]);

    // Past the window: fully green.
    tree.render(300.0);
    let end = background(&tree);
    assert!((end[0]).abs() < 1e-3, "red channel done: {}", end[0]);
    assert!((end[1] - 1.0).abs() < 1e-3, "green channel done: {}", end[1]);
}

#[test]
fn zero_duration_switches_immediately() {
    let (mut tree, root) = hover_box(0.0);
    tree.render(0.0);
    tree.update_pointer_hover(Some(root));
    tree.render(0.0);
    let after = background(&tree);
    assert!(
        (after[0]).abs() < 1e-3 && (after[1] - 1.0).abs() < 1e-3,
        "zero-duration hover must switch straight to green: {after:?}"
    );
    assert!(!tree.test_transition_active(root), "no transition is started");
}

#[test]
fn set_style_call_applies_instantly_regardless_of_duration() {
    // A direct `setStyle` is not a pseudo-state switch: it must take effect
    // immediately even when transition-duration is positive (issue #209).
    let (mut tree, root) = hover_box(200.0);
    tree.render(0.0);
    tree.element_set_style(root, &[StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0))]);
    tree.render(0.0);
    let after = background(&tree);
    assert!(
        (after[2] - 1.0).abs() < 1e-3 && after[0].abs() < 1e-3,
        "direct setStyle must apply instantly: {after:?}"
    );
    assert!(!tree.test_transition_active(root), "setStyle starts no transition");
}
