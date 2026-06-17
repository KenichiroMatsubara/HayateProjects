use hayate_core::{
    Color, CursorValue, Dimension, DocumentEventKind, ElementKind, ElementTree, Event,
    InputModality, PointerKind, PseudoState, StyleProp,
};

/// A root View filling a 200×200 viewport carrying a pseudo style for `state`,
/// rendered once so the dirty set is clean before the gesture under test runs.
/// The returned element has a `:hover`/`:active`/`:focus` box visual, so any
/// invalidation of that pseudo state shows up as the element going visual-dirty.
fn pseudo_styled_root(state: PseudoState) -> (ElementTree, hayate_core::ElementId) {
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
    tree.element_set_pseudo_style(
        root,
        state,
        &[StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0))],
    );
    // Render drains every dirty set, so a clean slate precedes the gesture.
    tree.render(0.0);
    assert!(
        !tree.test_visual_dirty_contains(root),
        "render() must drain the dirty set so the gesture's mark is observable"
    );
    (tree, root)
}

/// Like `pseudo_styled_root` but the pseudo block carries a shape-affecting
/// prop (`font-size`), so the invalidation lands in the *shape* set. Focus
/// transitions mark the element visual-dirty unconditionally for cursor blink
/// (ADR-0032); routing the assertion through the shape set isolates the
/// `:active`/`:focus` invalidation from that visual mark.
fn pseudo_shaping_root(state: PseudoState) -> (ElementTree, hayate_core::ElementId) {
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
    tree.element_set_pseudo_style(root, state, &[StyleProp::FontSize(20.0)]);
    tree.render(0.0);
    assert!(
        !tree.test_shape_dirty_contains(root),
        "render() must drain the shape set so the gesture's mark is observable"
    );
    (tree, root)
}

#[test]
fn hover_enter_marks_hover_pseudo_dirty() {
    // The HTML mouseenter path flips the hover set; ADR-0100 requires the
    // matching `:hover` invalidation to ride the same operation, so the element
    // re-lowers with its hover appearance instead of silently diverging.
    let (mut tree, root) = pseudo_styled_root(PseudoState::Hover);

    tree.on_hover_enter(root);

    assert!(
        tree.test_visual_dirty_contains(root),
        "entering :hover must invalidate the element's :hover styling atomically"
    );
}

#[test]
fn hover_leave_marks_hover_pseudo_dirty() {
    // Symmetric to enter: the HTML mouseleave path drops the element from the
    // hover set and must invalidate `:hover` in the same operation (ADR-0100).
    let (mut tree, root) = pseudo_styled_root(PseudoState::Hover);
    tree.on_hover_enter(root);
    tree.render(0.0); // drain the enter's mark so the leave's mark is isolated

    tree.on_hover_leave(root);

    assert!(
        tree.test_visual_dirty_contains(root),
        "leaving :hover must invalidate the element's :hover styling atomically"
    );
}

#[test]
fn pointer_move_hover_marks_hover_pseudo_dirty() {
    // The pointer-move (canvas) hover path must invalidate `:hover` in the same
    // step it updates the hover set — the same atomic guarantee as the HTML
    // mouseenter path, exercised from the coordinate-driven input surface.
    let (mut tree, root) = pseudo_styled_root(PseudoState::Hover);

    assert!(tree.on_pointer_move(10.0, 10.0).moved);

    assert!(
        tree.test_visual_dirty_contains(root),
        "moving the pointer onto an element must invalidate its :hover styling"
    );
}

#[test]
fn pointer_down_marks_active_pseudo_dirty() {
    // Pressing flips `:active`; the matching invalidation must ride the same
    // operation (ADR-0100). Asserted through the shape set so the focus path's
    // unconditional visual mark doesn't mask the `:active` invalidation.
    let (mut tree, root) = pseudo_shaping_root(PseudoState::Active);

    tree.on_pointer_down_on(root, 5.0, 5.0);

    assert!(
        tree.test_shape_dirty_contains(root),
        "a pointer-down must invalidate the pressed element's :active styling"
    );
}

#[test]
fn pointer_up_marks_active_pseudo_dirty() {
    // Releasing clears `:active`; clearing the state and invalidating its style
    // are one operation. A pointer-up changes no focus, so the element going
    // visual-dirty here can only be the `:active` invalidation.
    let (mut tree, root) = pseudo_styled_root(PseudoState::Active);
    tree.on_pointer_down_on(root, 5.0, 5.0);
    tree.render(0.0); // drain the press's marks so the release's mark is isolated

    tree.on_pointer_up_on(Some(root));

    assert!(
        tree.test_visual_dirty_contains(root),
        "a pointer-up must invalidate the released element's :active styling"
    );
}

#[test]
fn focus_marks_focus_pseudo_dirty() {
    // Focusing flips `:focus`; the invalidation rides the same operation. Shape
    // set isolates it from the cursor-blink visual mark element_focus emits.
    let (mut tree, root) = pseudo_shaping_root(PseudoState::Focus);

    tree.on_focus(root);

    assert!(
        tree.test_shape_dirty_contains(root),
        "focusing must invalidate the focused element's :focus styling"
    );
}

#[test]
fn blur_marks_focus_pseudo_dirty() {
    // Blurring clears `:focus` and invalidates its style in one operation.
    let (mut tree, root) = pseudo_shaping_root(PseudoState::Focus);
    tree.on_focus(root);
    tree.render(0.0); // drain the focus marks so the blur's mark is isolated

    tree.on_blur(root);

    assert!(
        tree.test_shape_dirty_contains(root),
        "blurring must invalidate the blurred element's :focus styling"
    );
}

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
fn pointer_move_resolves_pointer_cursor_over_button_by_kind() {
    // A button with no explicit `cursor` still shows the pointer cursor, from
    // the element-kind UA default (ADR-0105), matching the browser's `<button>`.
    let mut tree = ElementTree::new();
    let root = tree.element_create(50, ElementKind::View);
    let button = tree.element_create(51, ElementKind::Button);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_append_child(root, button);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(200.0)),
        ],
    );
    tree.element_set_style(
        button,
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
fn pointer_move_resolves_text_cursor_over_text_input_by_kind() {
    // A text-input with no explicit `cursor` shows the I-beam (text) cursor from
    // its element-kind UA default (ADR-0105), matching the browser's `<input>`.
    let mut tree = ElementTree::new();
    let root = tree.element_create(60, ElementKind::View);
    let input = tree.element_create(61, ElementKind::TextInput);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_append_child(root, input);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(200.0)),
        ],
    );
    tree.element_set_style(
        input,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(40.0)),
        ],
    );
    tree.render(0.0);

    let result = tree.on_pointer_move(10.0, 10.0);

    assert_eq!(result.resolved_cursor, CursorValue::Text);
}

#[test]
fn pointer_move_resolves_text_cursor_over_selectable_text() {
    // Selectable text shows the I-beam even without an explicit `cursor`
    // (ADR-0105) — the same UA default a text-input gets, so a read-only
    // Selection Region reads as text.
    let mut tree = ElementTree::new();
    let root = tree.element_create(70, ElementKind::View);
    let text = tree.element_create(71, ElementKind::Text);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_append_child(root, text);
    tree.element_set_selectable(text, true);
    tree.element_set_text(text, "hello");
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(200.0)),
        ],
    );
    tree.element_set_style(
        text,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(40.0)),
        ],
    );
    tree.render(0.0);

    let result = tree.on_pointer_move(10.0, 10.0);

    assert_eq!(result.resolved_cursor, CursorValue::Text);
}

#[test]
fn explicit_cursor_overrides_element_kind_default() {
    // An explicit `cursor` always wins over the element-kind default (ADR-0105):
    // a button styled `not-allowed` reads not-allowed, not the pointer default.
    let mut tree = ElementTree::new();
    let root = tree.element_create(80, ElementKind::View);
    let button = tree.element_create(81, ElementKind::Button);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_append_child(root, button);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(200.0)),
        ],
    );
    tree.element_set_style(
        button,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
            StyleProp::Cursor(CursorValue::NotAllowed),
        ],
    );
    tree.render(0.0);

    let result = tree.on_pointer_move(10.0, 10.0);

    assert_eq!(result.resolved_cursor, CursorValue::NotAllowed);
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

#[test]
fn last_pointer_kind_tracks_the_device_per_interaction() {
    // PointerKind { Mouse, Touch, Pen } rides each pointer interaction (#357).
    // Core retains the most recent kind so later slices (touch gates, I-beam
    // modality) can branch on it. It defaults to Mouse before any pointer event.
    let (mut tree, _root) = hoverable_root();
    assert_eq!(tree.last_pointer_kind(), PointerKind::Mouse);

    // A touch press records Touch.
    tree.on_pointer_down_with_kind(10.0, 10.0, 0, PointerKind::Touch);
    assert_eq!(tree.last_pointer_kind(), PointerKind::Touch);

    // A pen move within the same surface updates the kind (hybrid devices switch
    // mid-session — it is not latched at startup).
    tree.on_pointer_move_with_kind(20.0, 20.0, PointerKind::Pen);
    assert_eq!(tree.last_pointer_kind(), PointerKind::Pen);

    // A mouse release records Mouse again.
    tree.on_pointer_up_with_kind(20.0, 20.0, PointerKind::Mouse);
    assert_eq!(tree.last_pointer_kind(), PointerKind::Mouse);
}

#[test]
fn input_modality_is_a_separate_axis_from_pointer_kind() {
    // The `:focus-visible` InputModality (Pointer/Keyboard) is orthogonal to
    // PointerKind: a touch press is still InputModality::Pointer, and a key press
    // flips modality without touching the retained pointer kind (#357, #335).
    let (mut tree, root) = hoverable_root();
    tree.on_pointer_down_with_kind(10.0, 10.0, 0, PointerKind::Touch);
    assert_eq!(tree.last_input_modality(), InputModality::Pointer);
    assert_eq!(tree.last_pointer_kind(), PointerKind::Touch);

    tree.on_focus(root);
    tree.on_key_down("ArrowLeft", 0);
    assert_eq!(tree.last_input_modality(), InputModality::Keyboard);
    // The keyboard interaction left the retained pointer kind untouched.
    assert_eq!(tree.last_pointer_kind(), PointerKind::Touch);
}

#[test]
fn pointer_move_event_carries_the_pointer_kind() {
    // The emitted PointerMove wire event carries the device (#357) so the host
    // and later slices see which pointer drove the move, not just its coords.
    let (mut tree, _root) = hoverable_root();
    assert!(tree.on_pointer_move_with_kind(10.0, 10.0, PointerKind::Pen).moved);
    let saw_pen = tree
        .poll_events()
        .into_iter()
        .any(|e| matches!(e, Event::PointerMove { kind, .. } if kind == PointerKind::Pen));
    assert!(saw_pen, "PointerMove must carry PointerKind::Pen");
}
