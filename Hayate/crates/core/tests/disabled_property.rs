//! ADR-0071: disabled はヒットテストとインタラクションを抑止する。

use hayate_core::{
    Dimension, ElementKind, ElementTree, StyleProp,
};

#[test]
fn disabled_element_is_not_hit_tested() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(100.0, 100.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    let button = tree.element_create(2, ElementKind::Button);
    tree.element_append_child(root, button);
    tree.element_set_style(
        button,
        &[
            StyleProp::Width(Dimension::px(40.0)),
            StyleProp::Height(Dimension::px(20.0)),
        ],
    );

    tree.render(0.0);
    assert_eq!(tree.hit_test(10.0, 10.0), Some(button));

    tree.element_set_disabled(button, true);
    assert_ne!(tree.hit_test(10.0, 10.0), Some(button));
}
