//! ビジュアル無効化の到達範囲における prop-class 精度。

use hayate_core::{
    Color, Dimension, ElementKind, ElementTree, PseudoState, StyleProp,
};

fn tree_with_parent_and_children() -> (ElementTree, hayate_core::ElementId, Vec<hayate_core::ElementId>) {
    let mut tree = ElementTree::new();
    let parent = tree.element_create(1, ElementKind::View);
    let mut children = Vec::new();
    for i in 0..3 {
        let child = tree.element_create(10 + i, ElementKind::View);
        tree.element_append_child(parent, child);
        tree.element_set_style(
            child,
            &[
                StyleProp::Width(Dimension::px(20.0)),
                StyleProp::Height(Dimension::px(20.0)),
                StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
            ],
        );
        children.push(child);
    }
    tree.set_root(parent);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        parent,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
        ],
    );
    (tree, parent, children)
}

#[test]
fn self_only_hover_background_relowers_only_own_nodes() {
    let (mut tree, parent, _children) = tree_with_parent_and_children();
    tree.element_set_pseudo_style(
        parent,
        PseudoState::Hover,
        &[StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0))],
    );
    tree.render(0.0);

    tree.update_pointer_hover(Some(parent));
    tree.render(0.0);

    assert_eq!(
        tree.test_scene_lowering_walk_count(),
        1,
        "self-only :hover background must re-lower only the element's own nodes"
    );
}

#[test]
fn z_index_change_reorders_siblings_without_relowering_sibling_internals() {
    let mut tree = ElementTree::new();
    let parent = tree.element_create(100, ElementKind::View);
    let back = tree.element_create(101, ElementKind::View);
    let front = tree.element_create(102, ElementKind::View);
    tree.set_root(parent);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        parent,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    tree.element_append_child(parent, back);
    tree.element_append_child(parent, front);
    for (id, z, color) in [
        (back, 0, Color::new(1.0, 0.0, 0.0, 1.0)),
        (front, 1, Color::new(0.0, 0.0, 1.0, 1.0)),
    ] {
        tree.element_set_style(
            id,
            &[
                StyleProp::Width(Dimension::px(40.0)),
                StyleProp::Height(Dimension::px(40.0)),
                StyleProp::BackgroundColor(color),
                StyleProp::ZIndex(z),
            ],
        );
    }
    tree.render(0.0);

    tree.element_set_style(front, &[StyleProp::ZIndex(5)]);
    tree.render(0.0);

    assert_eq!(
        tree.test_scene_lowering_walk_count(),
        1,
        "z-index change must re-lower only the changed element, not siblings"
    );
}

#[test]
fn ch1_text_local_color_relowers_only_ifc_text_descendants() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(200, ElementKind::View);
    let ifc_root = tree.element_create(201, ElementKind::Text);
    let inline = tree.element_create(202, ElementKind::Text);
    let sibling_view = tree.element_create(203, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(400.0, 100.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(300.0)),
            StyleProp::Height(Dimension::px(80.0)),
        ],
    );
    tree.element_append_child(root, ifc_root);
    tree.element_append_child(root, sibling_view);
    tree.element_append_child(ifc_root, inline);
    tree.element_set_text(ifc_root, "Hello ");
    tree.element_set_text(inline, "world");
    tree.element_set_style(ifc_root, &[StyleProp::Color(Color::new(1.0, 0.0, 0.0, 1.0))]);
    tree.element_set_style(
        sibling_view,
        &[
            StyleProp::Width(Dimension::px(40.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
        ],
    );
    tree.render(0.0);

    tree.element_set_style(
        ifc_root,
        &[StyleProp::Color(Color::new(0.0, 1.0, 0.0, 1.0))],
    );
    tree.render(0.0);

    assert_eq!(
        tree.test_scene_lowering_walk_count(),
        2,
        "ch1 text-local color must re-lower IFC root and inline text only"
    );
}

#[test]
fn ch2_ambient_default_color_relowers_whole_subtree() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(300, ElementKind::View);
    let child = tree.element_create(301, ElementKind::View);
    let grandchild = tree.element_create(302, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    tree.element_append_child(root, child);
    tree.element_append_child(child, grandchild);
    for id in [child, grandchild] {
        tree.element_set_style(
            id,
            &[
                StyleProp::Width(Dimension::px(30.0)),
                StyleProp::Height(Dimension::px(30.0)),
                StyleProp::BackgroundColor(Color::new(0.5, 0.5, 0.5, 1.0)),
            ],
        );
    }
    tree.render(0.0);

    tree.element_set_style(
        root,
        &[StyleProp::DefaultColor(Color::new(1.0, 0.0, 0.0, 1.0))],
    );
    tree.render(0.0);

    assert_eq!(
        tree.test_scene_lowering_walk_count(),
        3,
        "ch2 ambient default-* must re-lower the whole subtree"
    );
}
