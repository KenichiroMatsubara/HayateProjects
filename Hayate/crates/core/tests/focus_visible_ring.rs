//! `:focus-visible` parity (#335, ADR-0102): Canvas Mode reproduces Chromium's
//! native focus ring faithfully. A keyboard-driven focus rings any element; a
//! pointer-driven focus rings text inputs but not buttons. The ring is drawn by
//! core into the scene so the Canvas backends paint it without per-renderer work.

use hayate_core::{
    Color, Dimension, DrawOp, ElementId, ElementKind, ElementTree, NodeKind, PseudoState,
    RecordingPainter, StyleProp, render_scene_graph,
};

/// All fill colours in the scene, in paint order.
fn fill_colors(tree: &ElementTree) -> Vec<[f32; 4]> {
    let mut painter = RecordingPainter::new();
    render_scene_graph(tree.scene_graph(), &mut painter);
    painter
        .into_ops()
        .into_iter()
        .filter_map(|op| match op {
            DrawOp::FillRect { color, .. } => Some(color),
            _ => None,
        })
        .collect()
}

/// All `RoundedRing` nodes (x, y, width, height, border_width) in the scene.
fn rounded_rings(tree: &ElementTree) -> Vec<(f32, f32, f32, f32, f32)> {
    tree.scene_graph()
        .iter()
        .filter_map(|(_, node)| match node.kind {
            NodeKind::RoundedRing {
                x,
                y,
                width,
                height,
                border_width,
                ..
            } => Some((x, y, width, height, border_width)),
            _ => None,
        })
        .collect()
}

/// A single element of `kind` sized 100×40 at the origin, laid out and rendered.
fn one_element(kind: ElementKind) -> (ElementTree, ElementId) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    let el = tree.element_create(2, kind);
    tree.element_set_style(
        el,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(40.0)),
        ],
    );
    tree.element_append_child(root, el);
    tree.render(0.0);
    (tree, el)
}

#[test]
fn pointer_focused_button_is_not_focus_visible() {
    let (mut tree, button) = one_element(ElementKind::Button);
    tree.on_pointer_down_on(button, 10.0, 10.0);

    assert_eq!(
        tree.focus_visible_element(),
        None,
        "a button focused by pointer must not show the native focus ring"
    );
}

#[test]
fn focus_visible_emits_a_ring_outside_the_box() {
    // The element box sits at (0,0,100,40); a native focus ring must wrap it from
    // the outside (top-left strictly negative, wider than the box).
    let (mut tree, input) = one_element(ElementKind::TextInput);
    tree.on_pointer_down_on(input, 10.0, 10.0);
    tree.render(16.0);

    let ring = rounded_rings(&tree)
        .into_iter()
        .find(|&(x, y, w, h, bw)| x < 0.0 && y < 0.0 && w > 100.0 && h > 40.0 && bw > 0.0);
    assert!(
        ring.is_some(),
        "expected a focus ring wrapping the box from outside, got rings: {:?}",
        rounded_rings(&tree)
    );
}

#[test]
fn focus_ring_is_not_clipped_by_the_elements_own_overflow() {
    // Chromium paints the focus outline outside the element's own clip. With
    // `overflow: hidden` the ring must therefore attach above the element's clip
    // node, not inside it (where it would be cropped to the box).
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    let input = tree.element_create(2, ElementKind::TextInput);
    tree.element_set_style(
        input,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::Overflow(hayate_core::OverflowValue::Hidden),
        ],
    );
    tree.element_append_child(root, input);
    tree.render(0.0);
    tree.on_pointer_down_on(input, 10.0, 10.0);
    tree.render(16.0);

    let sg = tree.scene_graph();
    let ring = sg
        .iter()
        .find(|(_, n)| matches!(n.kind, NodeKind::RoundedRing { .. }))
        .map(|(id, _)| id)
        .expect("focus ring present");
    let parent_is_clip = sg
        .parent_of(ring)
        .and_then(|p| sg.get(p))
        .is_some_and(|n| matches!(n.kind, NodeKind::Clip { .. }));
    assert!(
        !parent_is_clip,
        "focus ring must not be nested inside the element's own overflow clip"
    );
}

#[test]
fn app_focus_background_switch_still_works_alongside_the_ring() {
    const RED: Color = Color::new(1.0, 0.0, 0.0, 1.0);
    const GREEN: Color = Color::new(0.0, 1.0, 0.0, 1.0);

    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    let input = tree.element_create(2, ElementKind::TextInput);
    tree.element_set_style(
        input,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::BackgroundColor(RED),
        ],
    );
    tree.element_set_pseudo_style(input, PseudoState::Focus, &[StyleProp::BackgroundColor(GREEN)]);
    tree.element_append_child(root, input);
    tree.render(0.0);

    let green = GREEN.to_array_f32();
    assert!(!fill_colors(&tree).contains(&green), "unfocused: background is the base colour");

    tree.on_pointer_down_on(input, 10.0, 10.0);
    tree.render(16.0);

    assert!(
        fill_colors(&tree).contains(&green),
        "focused: the app's :focus background switch must still apply"
    );
    assert!(
        !rounded_rings(&tree).is_empty(),
        "focused: the native ring is drawn in addition to the :focus background"
    );
}

#[test]
fn pointer_focused_button_emits_no_ring() {
    // The button has no border of its own, so the scene must hold no ring at all.
    let (mut tree, button) = one_element(ElementKind::Button);
    tree.on_pointer_down_on(button, 10.0, 10.0);
    tree.render(16.0);

    assert!(
        rounded_rings(&tree).is_empty(),
        "a pointer-focused button must not draw a focus ring, got: {:?}",
        rounded_rings(&tree)
    );
}

#[test]
fn keyboard_focused_button_is_focus_visible() {
    let (mut tree, button) = one_element(ElementKind::Button);
    // A keyboard interaction (e.g. Tab) precedes the focus move.
    tree.on_key_down("Tab", 0);
    tree.on_focus(button);

    assert_eq!(
        tree.focus_visible_element(),
        Some(button),
        "a button focused after a keyboard interaction shows the native focus ring"
    );
}

#[test]
fn pointer_focused_text_input_is_focus_visible() {
    let (mut tree, input) = one_element(ElementKind::TextInput);
    tree.on_pointer_down_on(input, 10.0, 10.0);

    assert_eq!(
        tree.focus_visible_element(),
        Some(input),
        "a text input always shows the native focus ring, even on pointer focus"
    );
}
