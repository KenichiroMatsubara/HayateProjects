use hayate_core::{
    DocumentEventKind, ElementKind, ElementTree, Event,
};

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

    assert!(tree.on_pointer_move(1.0, 2.0));
    assert!(!tree.on_pointer_move(1.0, 2.0));
    assert!(tree.on_pointer_move(2.0, 2.0));
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
