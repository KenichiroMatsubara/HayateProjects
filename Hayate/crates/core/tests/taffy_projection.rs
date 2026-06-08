use hayate_core::{ElementKind, ElementTree, StyleProp};

/// ADR-0064 / LAY-04: inline text elements (text whose parent is text) must not
/// receive a Taffy node; only the IFC root is projected as a measured leaf.
#[test]
fn inline_text_element_has_no_taffy_node_after_layout() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    let ifc_root = tree.element_create(2, ElementKind::Text);
    let inline = tree.element_create(3, ElementKind::Text);
    tree.set_root(root);
    tree.set_viewport(400.0, 200.0);
    tree.element_append_child(root, ifc_root);
    tree.element_append_child(ifc_root, inline);
    tree.element_set_text(ifc_root, "Hello ");
    tree.element_set_text(inline, "world");
    tree.render(0.0);

    assert!(
        tree.element_has_taffy_node(ifc_root),
        "IFC root must be projected to Taffy"
    );
    assert!(
        !tree.element_has_taffy_node(inline),
        "inline text element must not be projected to Taffy"
    );
    assert!(
        tree.element_layout_rect(ifc_root).is_some(),
        "IFC root must have layout geometry"
    );
    assert!(
        tree.element_layout_rect(inline).is_none(),
        "inline text element must not have box layout cache"
    );
}

/// Structure mutations must not eagerly touch Taffy; layout after append still works.
#[test]
fn append_before_layout_still_produces_valid_geometry() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(10, ElementKind::View);
    let child = tree.element_create(11, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(300.0, 200.0);
    tree.element_set_style(
        child,
        &[
            StyleProp::Width(hayate_core::Dimension::px(80.0)),
            StyleProp::Height(hayate_core::Dimension::px(40.0)),
        ],
    );
    tree.element_append_child(root, child);
    tree.render(0.0);

    let rect = tree
        .element_layout_rect(child)
        .expect("child must have layout after lazy reconcile");
    assert!((rect.2 - 80.0).abs() < 0.5);
    assert!((rect.3 - 40.0).abs() < 0.5);
}

/// Reparenting text under text flips projection class (block IFC root → inline).
#[test]
fn reparent_text_under_text_clears_taffy_node() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(20, ElementKind::View);
    let outer = tree.element_create(21, ElementKind::Text);
    let inner = tree.element_create(22, ElementKind::Text);
    tree.set_root(root);
    tree.set_viewport(400.0, 200.0);
    tree.element_append_child(root, outer);
    tree.element_set_text(outer, "block");
    tree.render(0.0);
    assert!(tree.element_has_taffy_node(outer));

    tree.element_append_child(outer, inner);
    tree.element_set_text(inner, "inline");
    tree.render(0.0);

    assert!(
        tree.element_has_taffy_node(outer),
        "IFC root keeps Taffy node when gaining inline child"
    );
    assert!(
        !tree.element_has_taffy_node(inner),
        "newly nested text becomes inline and loses Taffy projection"
    );
}
