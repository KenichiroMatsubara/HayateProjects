//! Issue #228: a change to an ancestor's *inheritable* text style propagates
//! through inheritance (ADR-0065 two-channel: text-local + ambient default) and
//! the inheriting descendant interpolates its own continuous property over its
//! resolved `transition-duration`. This exercises the #227 design intent — the
//! `resolve_effective` per-property diff (ADR-0093) captures non-local changes
//! (mutating a parent changes a child's computed value) that never appear in the
//! child's own mutation. Verified end-to-end through `render(timestamp_ms)` and
//! the painted `DrawTextRun` colour.

use hayate_core::{
    Color, Dimension, DrawOp, ElementId, ElementKind, ElementTree, RecordingPainter, StyleProp,
    render_scene_graph,
};

fn draw_ops(tree: &ElementTree) -> Vec<DrawOp> {
    let sg = tree.scene_graph();
    let mut painter = RecordingPainter::new();
    render_scene_graph(sg, &mut painter);
    painter.into_ops()
}

/// Colour of the first painted text run in the current (retained) scene.
fn text_color(tree: &ElementTree) -> [f32; 4] {
    for op in draw_ops(tree) {
        if let DrawOp::DrawTextRun { color, .. } = op {
            return color;
        }
    }
    panic!("no DrawTextRun in scene");
}

/// Colour of the first filled rect (the parent box's background) in the scene.
fn background(tree: &ElementTree) -> [f32; 4] {
    for op in draw_ops(tree) {
        if let DrawOp::FillRect { color, .. } = op {
            return color;
        }
    }
    panic!("no FillRect in scene");
}

/// Parent `View` carrying an ambient `default-color` (ch2, block-penetrating) and
/// a child `Text` that inherits it. The child owns the `transition-duration`, so
/// it interpolates its inherited colour when the parent's default-color changes.
fn view_over_text(
    default_color: Color,
    child_duration_ms: f32,
) -> (ElementTree, ElementId, ElementId) {
    let mut tree = ElementTree::new();
    let view = tree.element_create(1, ElementKind::View);
    let text = tree.element_create(2, ElementKind::Text);
    tree.set_root(view);
    tree.set_viewport(200.0, 200.0);
    tree.element_append_child(view, text);
    tree.element_set_style(
        view,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(100.0)),
            StyleProp::DefaultColor(default_color),
        ],
    );
    tree.element_set_style(text, &[StyleProp::TransitionDuration(child_duration_ms)]);
    tree.element_set_text(text, "Hello");
    (tree, view, text)
}

#[test]
fn inherited_default_color_change_interpolates_descendant_text() {
    // AC1: changing the parent's inheritable property interpolates the child's
    // corresponding property toward the target.
    let (mut tree, view, _text) = view_over_text(Color::new(1.0, 0.0, 0.0, 1.0), 200.0);
    tree.render(0.0);
    let start = text_color(&tree);
    assert!(
        (start[0] - 1.0).abs() < 1e-3 && start[2].abs() < 1e-3,
        "child text starts red (inherited): {start:?}"
    );

    // Mutate the *parent* — the child never mutates, its computed value does.
    tree.element_set_style(
        view,
        &[StyleProp::DefaultColor(Color::new(0.0, 0.0, 1.0, 1.0))],
    );

    // First post-change frame anchors the child's clock; still red.
    tree.render(100.0);
    let anchored = text_color(&tree);
    assert!(
        (anchored[0] - 1.0).abs() < 1e-3 && anchored[2].abs() < 1e-3,
        "frame 0 of the inherited transition is still red: {anchored:?}"
    );

    // Halfway: between red and blue.
    tree.render(200.0);
    let mid = text_color(&tree);
    assert!(mid[0] < 1.0 && mid[0] > 0.0, "red channel mid: {}", mid[0]);
    assert!(mid[2] > 0.0 && mid[2] < 1.0, "blue channel mid: {}", mid[2]);

    // Past the window: fully blue.
    tree.render(300.0);
    let end = text_color(&tree);
    assert!(
        end[2] > 0.999 && end[0].abs() < 1e-3,
        "child settles on the inherited blue: {end:?}"
    );
}

#[test]
fn descendant_without_duration_takes_inherited_target_immediately() {
    // AC2: when the descendant's own resolved `transition-duration` is 0 /
    // unset, an inherited change snaps straight to the target — no interpolation,
    // no in-flight transition.
    let (mut tree, view, text) = view_over_text(Color::new(1.0, 0.0, 0.0, 1.0), 0.0);
    tree.render(0.0);
    assert!((text_color(&tree)[0] - 1.0).abs() < 1e-3, "starts red");

    tree.element_set_style(
        view,
        &[StyleProp::DefaultColor(Color::new(0.0, 0.0, 1.0, 1.0))],
    );
    tree.render(100.0);

    let after = text_color(&tree);
    assert!(
        after[2] > 0.999 && after[0].abs() < 1e-3,
        "zero-duration descendant jumps straight to inherited blue: {after:?}"
    );
    assert!(
        !tree.test_transition_active(text),
        "no transition is started for a zero-duration descendant"
    );
}

#[test]
fn ancestor_change_re_evaluates_descendant_so_its_diff_runs() {
    // AC3: the inherited change is non-local — only the parent mutates. The
    // descendant must be pulled back into the diff (re-emitted) for its
    // per-property transition to start at all; a started transition is proof the
    // child's `resolve_effective` diff ran. It then stays scheduled for
    // re-evaluation until it settles.
    let (mut tree, view, text) = view_over_text(Color::new(1.0, 0.0, 0.0, 1.0), 200.0);
    tree.render(0.0);
    assert!(
        !tree.test_transition_active(text),
        "no transition before the ancestor changes"
    );

    tree.element_set_style(
        view,
        &[StyleProp::DefaultColor(Color::new(0.0, 0.0, 1.0, 1.0))],
    );
    tree.render(100.0);

    assert!(
        tree.test_transition_active(text),
        "the child's diff ran off the ancestor change and started a transition"
    );
    assert!(
        tree.test_visual_dirty_contains(text),
        "an interpolating child stays visual-dirty for continued re-evaluation"
    );
}

#[test]
fn mutated_parent_interpolates_its_own_property_alongside_inherited_child() {
    // AC4: the parent is both the mutation site and an inheritance source. A
    // single `setStyle` that changes the parent's own `background-color` *and*
    // the ambient `default-color` it passes down must interpolate the parent's
    // box (per #227) and the child's text (the #228 inherited slice) together —
    // the inherited path must not regress the parent's direct-mutation path.
    let (mut tree, view, text) = view_over_text(Color::new(1.0, 0.0, 0.0, 1.0), 200.0);
    tree.element_set_style(
        view,
        &[
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
            StyleProp::TransitionDuration(200.0),
        ],
    );
    tree.render(0.0);
    assert!(
        (background(&tree)[0] - 1.0).abs() < 1e-3,
        "parent box starts red"
    );
    assert!(
        (text_color(&tree)[0] - 1.0).abs() < 1e-3,
        "child text starts red"
    );

    // One mutation drives both the parent's own box and the inherited child.
    tree.element_set_style(
        view,
        &[
            StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
            StyleProp::DefaultColor(Color::new(0.0, 0.0, 1.0, 1.0)),
        ],
    );
    tree.render(100.0); // anchor both clocks
    tree.render(200.0); // halfway

    let parent_mid = background(&tree);
    assert!(
        parent_mid[0] > 0.0 && parent_mid[0] < 1.0 && parent_mid[2] > 0.0 && parent_mid[2] < 1.0,
        "parent box interpolates its own background (#227 intact): {parent_mid:?}"
    );
    assert!(
        tree.test_transition_active(view),
        "parent's own transition is live"
    );

    let child_mid = text_color(&tree);
    assert!(
        child_mid[0] > 0.0 && child_mid[0] < 1.0 && child_mid[2] > 0.0 && child_mid[2] < 1.0,
        "child text interpolates the inherited colour at the same time: {child_mid:?}"
    );
    assert!(
        tree.test_transition_active(text),
        "child's inherited transition is live"
    );
}
