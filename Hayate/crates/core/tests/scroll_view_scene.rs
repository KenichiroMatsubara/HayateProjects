//! ScrollView のシーングラフ lowering: 子アンカーは Clip/scroll ラッパー配下に置かれねばならない。

use hayate_core::{Color, Dimension, ElementKind, ElementTree, NodeKind, StyleProp};

fn has_ancestor_matching(
    tree: &ElementTree,
    node: hayate_core::NodeId,
    pred: impl Fn(&NodeKind) -> bool,
) -> bool {
    let sg = tree.scene_graph();
    let mut current = Some(node);
    while let Some(id) = current {
        let Some(n) = sg.get(id) else {
            break;
        };
        if pred(&n.kind) {
            return true;
        }
        current = sg.parent_of(id);
    }
    false
}

fn scrolled_scroll_view_tree() -> (ElementTree, hayate_core::ElementId, hayate_core::ElementId) {
    let mut tree = ElementTree::new();
    let scroll = tree.element_create(1, ElementKind::ScrollView);
    let content = tree.element_create(2, ElementKind::View);
    tree.set_root(scroll);
    tree.set_viewport(300.0, 300.0);
    tree.element_set_style(
        scroll,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    tree.element_set_style(
        content,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(500.0)),
            StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0)),
        ],
    );
    tree.element_append_child(scroll, content);
    tree.element_set_scroll_offset(scroll, 0.0, 50.0);
    (tree, scroll, content)
}

#[test]
fn scroll_view_child_anchor_is_descendant_of_clip_and_scroll_group() {
    let (mut tree, _scroll, content) = scrolled_scroll_view_tree();
    tree.render(0.0);

    let child_anchor = tree.test_element_anchor_id(content);
    assert!(
        has_ancestor_matching(&tree, child_anchor, |kind| {
            matches!(kind, NodeKind::Clip { .. })
        }),
        "child ElementAnchor must be nested under the ScrollView Clip node"
    );
    assert!(
        has_ancestor_matching(&tree, child_anchor, |kind| {
            matches!(kind, NodeKind::Group { .. })
        }),
        "scrolled ScrollView child must be nested under the scroll-offset Group"
    );
}

#[test]
fn scroll_view_relowering_preserves_child_under_clip_wrapper() {
    let (mut tree, scroll, content) = scrolled_scroll_view_tree();
    tree.render(0.0);
    let content_anchor_after_first = tree.test_element_anchor_id(content);
    assert!(
        has_ancestor_matching(&tree, content_anchor_after_first, |kind| {
            matches!(kind, NodeKind::Clip { .. })
        }),
        "initial build must nest content under Clip"
    );

    tree.element_set_scroll_offset(scroll, 0.0, 120.0);
    tree.render(0.0);

    let content_anchor_after_scroll = tree.test_element_anchor_id(content);
    assert_eq!(
        content_anchor_after_scroll, content_anchor_after_first,
        "content anchor must stay stable across re-lowering"
    );
    assert!(
        has_ancestor_matching(&tree, content_anchor_after_scroll, |kind| {
            matches!(kind, NodeKind::Clip { .. })
        }),
        "re-lowering after scroll change must keep content under Clip"
    );
    assert!(
        has_ancestor_matching(&tree, content_anchor_after_scroll, |kind| {
            matches!(kind, NodeKind::Group { .. })
        }),
        "re-lowering after scroll change must keep content under scroll-offset Group"
    );

    tree.element_set_style(
        content,
        &[StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0))],
    );
    tree.render(0.0);

    assert_eq!(
        tree.test_element_anchor_id(content),
        content_anchor_after_first,
        "content anchor must survive child visual re-lowering"
    );
    assert!(
        has_ancestor_matching(&tree, tree.test_element_anchor_id(content), |kind| {
            matches!(kind, NodeKind::Clip { .. })
        }),
        "child visual change must not detach content from Clip wrapper chain"
    );
}
