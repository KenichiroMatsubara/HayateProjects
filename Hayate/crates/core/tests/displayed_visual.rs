//! Issue #301 (§1b): a read-only `element_displayed_visual(id, now_ms)` query
//! lets a transition's in-flight value be observed straight from the retained
//! state — no `render()` → SceneGraph walk, no frame loop. It interpolates the
//! same way the render path does (ADR-0093) but is side-effect free (`&self`):
//! it never advances render's memoized transition state.

use hayate_core::{
    Color, Dimension, DrawOp, ElementKind, ElementTree, PseudoState, RecordingPainter, StyleProp,
    render_scene_graph,
};

/// Background colour of the first filled rect painted by the retained scene.
fn painted_background(tree: &ElementTree) -> [f32; 4] {
    let mut painter = RecordingPainter::new();
    render_scene_graph(tree.scene_graph(), &mut painter);
    for op in painter.into_ops() {
        if let DrawOp::FillRect { color, .. } = op {
            return color;
        }
    }
    panic!("no FillRect in scene");
}

/// A 100×50 box that is red at rest and transitions to green on `:hover` over
/// `duration_ms`. Mirrors the fixture in `transition_interpolation.rs`.
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
fn displayed_visual_observes_mid_transition_without_a_frame_at_that_time() {
    let (mut tree, root) = hover_box(200.0);
    tree.render(0.0);
    tree.update_pointer_hover(Some(root));
    tree.render(100.0); // anchors the transition clock at t=100; still red

    // Query halfway through the 200ms window WITHOUT rendering at t=200: the
    // in-flight track is sampled straight from the retained state.
    let mid = tree
        .element_displayed_visual(root, 200.0)
        .unwrap()
        .background_color
        .unwrap();
    assert!(
        mid.r < 1.0 && mid.r > 0.0,
        "red channel is mid-transition: {}",
        mid.r
    );
    assert!(
        mid.g > 0.0 && mid.g < 1.0,
        "green channel is mid-transition: {}",
        mid.g
    );
}

#[test]
fn displayed_visual_returns_effective_target_when_settled() {
    let (mut tree, root) = hover_box(200.0);
    tree.render(0.0);

    // Nothing is in flight, so the displayed visual is just the resolved target.
    let displayed = tree.element_displayed_visual(root, 0.0).unwrap();
    assert_eq!(
        displayed.background_color,
        Some(Color::new(1.0, 0.0, 0.0, 1.0)),
        "a settled element's displayed visual is its effective target"
    );
}

#[test]
fn displayed_visual_does_not_advance_render_state() {
    let (mut tree, root) = hover_box(200.0);
    tree.render(0.0);
    tree.update_pointer_hover(Some(root));
    tree.render(100.0); // anchor at t=100

    // A query at a time well past the window's end would — if it mutated the
    // retained state the way the render path does — settle the track on green
    // and drop it. The query must leave the in-flight transition untouched.
    let _ = tree.element_displayed_visual(root, 100_000.0);
    assert!(
        tree.test_transition_active(root),
        "query must not complete or drop the in-flight transition"
    );

    // The next real frame therefore advances from where render left off (t=100),
    // landing mid-window at t=200 rather than snapped to green.
    tree.render(200.0);
    let painted = painted_background(&tree);
    assert!(
        painted[0] > 0.0 && painted[1] > 0.0 && painted[1] < 1.0,
        "render continues mid-transition, unperturbed by the query: {painted:?}"
    );
}

#[test]
fn displayed_visual_matches_what_render_paints_at_the_same_time() {
    // The query and the render path must share one interpolation (no second
    // implementation): sampling at `now_ms` equals what a frame at `now_ms`
    // paints, because both run the same blend over the same retained state.
    let (mut tree, root) = hover_box(200.0);
    tree.render(0.0);
    tree.update_pointer_hover(Some(root));
    tree.render(100.0); // anchor at t=100

    // Observe via query first (side-effect free), then paint a frame at the same
    // instant. Both advance the same track from t=100.
    let queried = tree
        .element_displayed_visual(root, 175.0)
        .unwrap()
        .background_color
        .unwrap();
    tree.render(175.0);
    let painted = painted_background(&tree);

    assert!(
        (queried.r as f32 - painted[0]).abs() < 1e-4
            && (queried.g as f32 - painted[1]).abs() < 1e-4
            && (queried.b as f32 - painted[2]).abs() < 1e-4,
        "query must agree with the painted frame at the same time: \
         query={queried:?} painted={painted:?}"
    );
}
