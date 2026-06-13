use hayate_core::{
    CursorValue, Dimension, DocumentEventKind, ElementKind, ElementTree, Event, StyleProp,
};

/// A root View filling a 200×200 viewport, laid out so hit-testing has bounds.
fn hoverable_root() -> (ElementTree, hayate_core::ElementId) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(100, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(200.0)),
        ],
    );
    tree.render(0.0);
    (tree, root)
}

#[test]
fn pointer_leave_delivers_hover_leave_and_re_hover_re_enters() {
    let (mut tree, root) = hoverable_root();
    let enter = tree.register_listener(root, DocumentEventKind::HoverEnter);
    let leave = tree.register_listener(root, DocumentEventKind::HoverLeave);

    // Hover into the surface — HoverEnter for the root.
    assert!(tree.on_pointer_move(10.0, 10.0).moved);
    let entered: Vec<_> = tree
        .poll_deliveries()
        .into_iter()
        .filter(|d| matches!(&d.event, Event::HoverEnter { .. }))
        .map(|d| d.listener_id)
        .collect();
    assert_eq!(entered, vec![enter]);

    // Pointer leaves the surface — HoverLeave for the previously-hovered root.
    tree.on_pointer_leave();
    let left: Vec<_> = tree
        .poll_deliveries()
        .into_iter()
        .filter(|d| matches!(&d.event, Event::HoverLeave { .. }))
        .map(|d| d.listener_id)
        .collect();
    assert_eq!(left, vec![leave]);

    // Re-hovering reapplies `:hover` — HoverEnter fires again.
    assert!(tree.on_pointer_move(20.0, 20.0).moved);
    let re_entered: Vec<_> = tree
        .poll_deliveries()
        .into_iter()
        .filter(|d| matches!(&d.event, Event::HoverEnter { .. }))
        .map(|d| d.listener_id)
        .collect();
    assert_eq!(re_entered, vec![enter]);
}

#[test]
fn pointer_leave_resets_last_pointer_pos_so_repeat_coord_is_not_deduped() {
    let (mut tree, _root) = hoverable_root();

    // First move establishes last-pointer-position; an identical move coalesces.
    assert!(tree.on_pointer_move(30.0, 30.0).moved);
    assert!(!tree.on_pointer_move(30.0, 30.0).moved);

    // Leaving the surface clears the stored position, so re-entering at the
    // exact same coordinate is delivered rather than swallowed by the 1px dedup.
    tree.on_pointer_leave();
    assert!(tree.on_pointer_move(30.0, 30.0).moved);
}

#[test]
fn pointer_leave_does_not_push_phantom_pointer_move() {
    let (mut tree, _root) = hoverable_root();
    assert!(tree.on_pointer_move(40.0, 40.0).moved);
    let _ = tree.poll_events(); // drain the move from the enter

    tree.on_pointer_leave();
    assert!(
        !tree
            .poll_events()
            .iter()
            .any(|e| matches!(e, Event::PointerMove { .. })),
        "on_pointer_leave must not fabricate a PointerMove"
    );
}

#[test]
fn pointer_down_dispatches_click_to_listener() {
    let mut tree = ElementTree::new();
    let btn = tree.element_create(1, ElementKind::Button);
    tree.set_root(btn);
    let listener = tree.register_listener(btn, DocumentEventKind::Click);

    tree.on_pointer_down_on(btn, 10.0, 20.0);

    let deliveries = tree.poll_deliveries();
    assert_eq!(deliveries.len(), 1);
    assert_eq!(deliveries[0].listener_id, listener);
    assert!(matches!(
        &deliveries[0].event,
        Event::Click { target_id, x, y }
            if *target_id == btn && (*x - 10.0).abs() < f32::EPSILON && (*y - 20.0).abs() < f32::EPSILON
    ));
}

#[test]
fn pointer_down_sets_focus_on_tree_only() {
    let mut tree = ElementTree::new();
    let btn = tree.element_create(2, ElementKind::Button);
    tree.set_root(btn);

    tree.on_pointer_down_on(btn, 0.0, 0.0);

    assert_eq!(tree.focused_element(), Some(btn));
    assert_eq!(tree.active_element(), Some(btn));
}

#[test]
fn pointer_move_skips_duplicate_coordinates() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(3, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[hayate_core::StyleProp::Width(hayate_core::Dimension::px(200.0))],
    );
    tree.render(0.0);

    assert!(tree.on_pointer_move(1.0, 2.0).moved);
    assert!(!tree.on_pointer_move(1.0, 2.0).moved);
    assert!(tree.on_pointer_move(2.0, 2.0).moved);
}

#[test]
fn pointer_move_resolves_cursor_from_hovered_element() {
    let (mut tree, root) = hoverable_root();
    tree.element_set_style(root, &[StyleProp::Cursor(CursorValue::Pointer)]);
    tree.render(0.0);

    let result = tree.on_pointer_move(10.0, 10.0);

    assert!(result.moved, "a real move must not be coalesced");
    assert_eq!(result.resolved_cursor, CursorValue::Pointer);
}

#[test]
fn pointer_move_resolves_default_cursor_when_unset() {
    let (mut tree, _root) = hoverable_root();

    let result = tree.on_pointer_move(10.0, 10.0);

    assert_eq!(result.resolved_cursor, CursorValue::Default);
}

#[test]
fn pointer_move_inherits_cursor_from_ancestor() {
    // A child with no cursor of its own inherits its ancestor's `cursor`,
    // mirroring CSS inheritance, so hovering a button's text still shows the
    // pointer cursor set on the button.
    let mut tree = ElementTree::new();
    let root = tree.element_create(40, ElementKind::View);
    let child = tree.element_create(41, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_append_child(root, child);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(200.0)),
            StyleProp::Cursor(CursorValue::Pointer),
        ],
    );
    tree.element_set_style(
        child,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    tree.render(0.0);

    let result = tree.on_pointer_move(10.0, 10.0);

    assert_eq!(result.resolved_cursor, CursorValue::Pointer);
}

#[test]
fn pointer_down_on_miss_blurs_focused_element() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(10, ElementKind::View);
    let btn = tree.element_create(11, ElementKind::Button);
    tree.set_root(root);
    tree.on_pointer_down_on(btn, 0.0, 0.0);
    assert_eq!(tree.focused_element(), Some(btn));

    tree.on_pointer_down(999.0, 999.0);

    assert_eq!(tree.focused_element(), None);
}

#[test]
fn pointer_cancel_ends_active_press_and_clears_hover() {
    let (mut tree, root) = hoverable_root();
    let l_hover_leave = tree.register_listener(root, DocumentEventKind::HoverLeave);
    let l_active_end = tree.register_listener(root, DocumentEventKind::ActiveEnd);

    // Establish hover (pointer over the element) and an active press.
    tree.on_pointer_move(10.0, 10.0);
    tree.on_pointer_down(10.0, 10.0);
    assert_eq!(tree.active_element(), Some(root));
    let _ = tree.poll_deliveries(); // drain the enter/start deliveries

    tree.on_pointer_cancel();

    let deliveries = tree.poll_deliveries();
    assert!(
        deliveries.iter().any(|d| d.listener_id == l_active_end
            && matches!(&d.event, Event::ActiveEnd { target_id } if *target_id == root)),
        "expected ActiveEnd delivered to the active element"
    );
    assert!(
        deliveries.iter().any(|d| d.listener_id == l_hover_leave
            && matches!(&d.event, Event::HoverLeave { target_id } if *target_id == root)),
        "expected HoverLeave delivered for the cleared hover chain"
    );
    assert_eq!(tree.active_element(), None);
}

#[test]
fn pointer_cancel_does_not_fabricate_pointer_move() {
    let (mut tree, _root) = hoverable_root();

    tree.on_pointer_move(40.0, 40.0);
    let _ = tree.poll_events(); // drain the real PointerMove from the move above

    tree.on_pointer_cancel();

    let events = tree.poll_events();
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, Event::PointerMove { .. })),
        "on_pointer_cancel must not push a phantom PointerMove"
    );
}

#[test]
fn click_bubbles_to_ancestors() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(20, ElementKind::View);
    let leaf = tree.element_create(21, ElementKind::Button);
    tree.set_root(root);
    tree.element_append_child(root, leaf);

    let l_root = tree.register_listener(root, DocumentEventKind::Click);
    let l_leaf = tree.register_listener(leaf, DocumentEventKind::Click);

    tree.on_pointer_down_on(leaf, 4.0, 5.0);

    let ids: Vec<_> = tree
        .poll_deliveries()
        .into_iter()
        .filter(|d| matches!(&d.event, Event::Click { .. }))
        .map(|d| d.listener_id)
        .collect();
    assert_eq!(ids, vec![l_leaf, l_root]);
}
