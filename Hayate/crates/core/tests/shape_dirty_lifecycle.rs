use hayate_core::{Dimension, DisplayValue, ElementKind, ElementTree, StyleProp};

#[test]
fn commit_frame_consumes_empty_text_shape_request() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    let text = tree.element_create(2, ElementKind::Text);
    tree.set_root(root);
    tree.set_viewport(300.0, 200.0);
    tree.element_set_style(root, &[StyleProp::Width(Dimension::px(300.0))]);
    tree.element_append_child(root, text);
    tree.element_set_text(text, "");

    assert!(tree.test_shape_dirty_contains(text));

    tree.commit_frame();

    assert!(
        !tree.test_shape_dirty_contains(text),
        "an empty text request is resolved even though it retains no glyph layout"
    );
}

#[test]
fn commit_frame_consumes_hidden_unmeasured_text_shape_request() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(10, ElementKind::View);
    let text = tree.element_create(11, ElementKind::Text);
    tree.set_root(root);
    tree.set_viewport(300.0, 200.0);
    tree.element_set_style(root, &[StyleProp::Width(Dimension::px(300.0))]);
    tree.element_set_style(text, &[StyleProp::Display(DisplayValue::None)]);
    tree.element_append_child(root, text);
    tree.element_set_text(text, "not rendered");

    assert!(tree.test_shape_dirty_contains(text));

    tree.commit_frame();

    assert!(
        !tree.test_shape_dirty_contains(text),
        "a hidden text request must not survive merely because Taffy did not measure it"
    );
}

#[test]
fn commit_frame_consumes_inline_text_request_at_its_ifc_root() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(20, ElementKind::View);
    let ifc_root = tree.element_create(21, ElementKind::Text);
    let inline = tree.element_create(22, ElementKind::Text);
    tree.set_root(root);
    tree.set_viewport(300.0, 200.0);
    tree.element_set_style(root, &[StyleProp::Width(Dimension::px(300.0))]);
    tree.element_append_child(root, ifc_root);
    tree.element_append_child(ifc_root, inline);
    tree.element_set_text(ifc_root, "Hello ");
    tree.element_set_text(inline, "world");

    assert!(tree.test_shape_dirty_contains(ifc_root));
    assert!(!tree.test_shape_dirty_contains(inline));

    tree.commit_frame();

    assert_eq!(
        tree.test_shaped_text(ifc_root).as_deref(),
        Some("Hello world")
    );
    assert!(
        !tree.test_shape_dirty_contains(ifc_root),
        "the IFC root must consume the inline text request it absorbed"
    );
}

#[test]
fn stable_render_does_not_relower_from_a_stale_shape_request() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(30, ElementKind::View);
    let text = tree.element_create(31, ElementKind::Text);
    tree.set_root(root);
    tree.set_viewport(300.0, 200.0);
    tree.element_set_style(root, &[StyleProp::Width(Dimension::px(300.0))]);
    tree.element_append_child(root, text);
    tree.element_set_text(text, "");

    tree.render(0.0);
    tree.render(16.0);

    assert!(
        tree.frame_layer_dirty().is_empty(),
        "an unchanged frame must not raster because of an already resolved shape request"
    );
    assert_eq!(
        tree.test_scene_lowering_walk_count(),
        0,
        "an unchanged frame must not start document lowering from stale shape_dirty"
    );
}
