use hayate_core::{ElementKind, ElementTree, StyleProp};

#[test]
fn ifc_root_shapes_concatenated_inline_text() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    let ifc = tree.element_create(2, ElementKind::Text);
    let inline = tree.element_create(3, ElementKind::Text);
    tree.set_root(root);
    tree.set_viewport(400.0, 100.0);
    tree.element_append_child(root, ifc);
    tree.element_append_child(ifc, inline);
    tree.element_set_text(ifc, "Hi ");
    tree.element_set_text(inline, "there");
    tree.render(0.0);

    let text = tree.element_get_text(ifc);
    assert_eq!(text, "Hi ");
    tree.render(0.0);
    assert!(
        tree.element_layout_rect(ifc).is_some(),
        "IFC root should have box geometry"
    );
}

#[test]
fn hit_test_resolves_inline_text_element_inside_ifc() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(10, ElementKind::View);
    let ifc = tree.element_create(11, ElementKind::Text);
    let inline = tree.element_create(12, ElementKind::Text);
    tree.set_root(root);
    tree.set_viewport(400.0, 100.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(hayate_core::Dimension::px(400.0)),
            StyleProp::Height(hayate_core::Dimension::px(100.0)),
        ],
    );
    tree.element_append_child(root, ifc);
    tree.element_append_child(ifc, inline);
    tree.element_set_text(ifc, "AAAA");
    tree.element_set_text(inline, "BBBB");
    tree.render(0.0);

    let (ex, ey, ew, eh) = tree
        .element_layout_rect(ifc)
        .expect("IFC layout");
    let hit_x = ex + ew * 0.85;
    let hit_y = ey + eh * 0.5;
    let hit = tree.hit_test(hit_x, hit_y);
    assert_eq!(
        hit,
        Some(inline),
        "point in inline span should resolve to inline text element"
    );
}
