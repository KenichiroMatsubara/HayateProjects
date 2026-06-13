//! Issue #227: transitions trigger off the `resolve_effective` per-property diff
//! (ADR-0093), so pseudo switches, `setStyle`, and inherited changes all
//! interpolate continuous visual props over `transition-duration` ms, driven by
//! `render(timestamp_ms)`. `from` is the on-screen (post-blend) value so reverse
//! interrupts reverse continuously, and duration/timing come from the
//! after-change resolved visual.

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

/// The first filled rect in the current (retained) scene.
fn first_fill(tree: &ElementTree) -> ([f32; 4], f32) {
    for op in draw_ops(tree) {
        if let DrawOp::FillRect {
            color,
            corner_radius,
            ..
        } = op
        {
            return (color, corner_radius);
        }
    }
    panic!("no FillRect in scene");
}

/// Background colour of the first filled rect in the current scene.
fn background(tree: &ElementTree) -> [f32; 4] {
    first_fill(tree).0
}

/// Background colour painted by a full ephemeral rebuild (parity reference path).
fn ephemeral_background(tree: &ElementTree) -> [f32; 4] {
    for op in tree.test_scene_full_rebuild_draw_ops() {
        if let DrawOp::FillRect { color, .. } = op {
            return color;
        }
    }
    panic!("no FillRect in ephemeral scene");
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
    assert!(
        (start[0] - 1.0).abs() < 1e-3 && start[1].abs() < 1e-3,
        "frame 0 still red"
    );

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
fn set_style_interpolates_continuous_property_over_duration() {
    // AC1: a direct `setStyle` (no pseudo-state switch) is just another
    // effective-visual change, so it interpolates when duration > 0 (ADR-0093,
    // restoring Canvas/DOM semantics parity).
    let (mut tree, root) = hover_box(200.0);
    tree.render(0.0);
    assert_eq!(background(&tree)[0], 1.0, "starts fully red");

    tree.element_set_style(root, &[StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0))]);

    // First post-change frame anchors the clock; still red.
    tree.render(100.0);
    let start = background(&tree);
    assert!(
        (start[0] - 1.0).abs() < 1e-3 && start[2].abs() < 1e-3,
        "setStyle frame 0 still red: {start:?}"
    );
    assert!(tree.test_transition_active(root), "setStyle starts a transition");

    // Mid-window: between red and blue.
    tree.render(200.0);
    let mid = background(&tree);
    assert!(mid[0] < 1.0 && mid[0] > 0.0, "red mid: {}", mid[0]);
    assert!(mid[2] > 0.0 && mid[2] < 1.0, "blue mid: {}", mid[2]);

    // Past the window: fully blue.
    tree.render(300.0);
    let end = background(&tree);
    assert!(
        end[2] > 0.999 && end[0].abs() < 1e-3,
        "setStyle settles on blue: {end:?}"
    );
}

#[test]
fn reverse_interrupt_continues_from_displayed_value() {
    // AC3: reversing mid-transition restarts from the current on-screen value,
    // never jumping to the resolved value.
    let (mut tree, root) = hover_box(200.0);
    tree.render(0.0);
    tree.update_pointer_hover(Some(root));
    tree.render(100.0); // anchor
    tree.render(200.0); // halfway red -> green
    let mid = background(&tree);
    assert!(mid[0] > 0.0 && mid[1] > 0.0, "captured a mid value: {mid:?}");

    // Reverse at the same instant: the displayed value must not jump.
    tree.update_pointer_hover(None);
    tree.render(200.0);
    let reversed = background(&tree);
    assert!(
        (reversed[0] - mid[0]).abs() < 1e-2 && (reversed[1] - mid[1]).abs() < 1e-2,
        "reversal is continuous, not a jump: {mid:?} -> {reversed:?}"
    );

    // Continuing the reverse heads back toward red (red channel climbs).
    tree.render(300.0);
    let back = background(&tree);
    assert!(back[0] > reversed[0], "red channel climbs back: {} -> {}", reversed[0], back[0]);
}

#[test]
fn duration_is_read_from_after_change_resolved_visual() {
    // AC4: `:hover { transition-duration: 0 }` over a base 500ms duration makes
    // hover-in instant and hover-out animated — duration is the after-change
    // resolved value, not a base-direct read.
    let mut tree = ElementTree::new();
    let root = tree.element_create(2, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(50.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
            StyleProp::TransitionDuration(500.0),
        ],
    );
    tree.element_set_pseudo_style(
        root,
        PseudoState::Hover,
        &[
            StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0)),
            StyleProp::TransitionDuration(0.0),
        ],
    );
    tree.render(0.0);

    // Hover-in: after-change duration is 0 → instant green, no transition.
    tree.update_pointer_hover(Some(root));
    tree.render(100.0);
    let hovered = background(&tree);
    assert!(
        (hovered[1] - 1.0).abs() < 1e-3 && hovered[0].abs() < 1e-3,
        "hover-in is instant: {hovered:?}"
    );
    assert!(!tree.test_transition_active(root), "instant hover-in starts no transition");

    // Hover-out: after-change duration is base 500ms → animated back to red.
    tree.update_pointer_hover(None);
    tree.render(200.0); // anchor
    tree.render(450.0); // 250ms into the 500ms window
    let mid = background(&tree);
    assert!(
        mid[0] > 0.0 && mid[0] < 1.0 && mid[1] > 0.0 && mid[1] < 1.0,
        "hover-out animates over 500ms: {mid:?}"
    );
    assert!(tree.test_transition_active(root), "hover-out starts a transition");
}

#[test]
fn first_emit_shows_target_without_transition() {
    // AC5: an element's very first render takes the target immediately — there
    // is no before-change value to interpolate from.
    let (mut tree, root) = hover_box(200.0);
    // Hover is already active before the first ever render.
    tree.update_pointer_hover(Some(root));
    tree.render(0.0);
    let first = background(&tree);
    assert!(
        (first[1] - 1.0).abs() < 1e-3 && first[0].abs() < 1e-3,
        "first emit paints the target (green) with no interpolation: {first:?}"
    );
    assert!(!tree.test_transition_active(root), "first emit starts no transition");
}

#[test]
fn properties_interpolate_from_independent_from_values() {
    // AC8: per element × property state — a later-changing property starts from
    // its own value and start time while an earlier one keeps running.
    let mut tree = ElementTree::new();
    let root = tree.element_create(3, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(50.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
            StyleProp::BorderRadius(0.0),
            StyleProp::TransitionDuration(200.0),
        ],
    );
    tree.render(0.0);

    // Background starts changing at t=100.
    tree.element_set_style(root, &[StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0))]);
    tree.render(100.0);

    // Border-radius starts changing one frame later, at t=200.
    tree.element_set_style(root, &[StyleProp::BorderRadius(20.0)]);
    tree.render(200.0);

    // By t=300 the background (started at 100) has completed its 200ms window
    // while the radius (started at 200) is only halfway — independent clocks.
    tree.render(300.0);
    let (color, radius) = first_fill(&tree);
    assert!(
        (color[1] - 1.0).abs() < 1e-3 && color[0].abs() < 1e-3,
        "background finished its own window: {color:?}"
    );
    assert!(
        radius > 0.0 && radius < 20.0,
        "border-radius is mid-flight on its own clock: {radius}"
    );
}

#[test]
fn full_ephemeral_rebuild_paints_target_without_interpolation() {
    // AC7: the ephemeral (parity reference) path has no retained `last_displayed`
    // so it never interpolates — it paints the resolved target.
    let (mut tree, root) = hover_box(200.0);
    tree.render(0.0);
    tree.update_pointer_hover(Some(root));
    tree.render(100.0); // anchor
    tree.render(200.0); // retained path is mid-transition

    let retained = background(&tree);
    assert!(
        retained[0] > 0.0 && retained[1] > 0.0 && retained[1] < 1.0,
        "retained path is mid-transition: {retained:?}"
    );

    let ephemeral = ephemeral_background(&tree);
    assert!(
        (ephemeral[1] - 1.0).abs() < 1e-3 && ephemeral[0].abs() < 1e-3,
        "ephemeral rebuild paints the resolved target (green): {ephemeral:?}"
    );
}
