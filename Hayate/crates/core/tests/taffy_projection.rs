use hayate_core::{ElementKind, ElementTree, StyleProp};

/// インラインテキスト要素（親がテキストのテキスト）には Taffy ノードを与えず、
/// IFC ルートのみを計測対象のリーフとして射影する（ADR-0064）。
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

/// 構造変更は Taffy を即座に触らない。append 後のレイアウトも正しく動く。
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

/// 最初のレイアウト前に削除した部分木は panic せず reconcile できなければならない。
#[test]
fn remove_lazy_subtree_before_first_layout_does_not_panic() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(25, ElementKind::View);
    let branch = tree.element_create(26, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(400.0, 300.0);
    tree.element_append_child(root, branch);
    tree.element_remove(branch);
    tree.commit_frame();
}

/// 兄弟の枝が射影されたまま片方の枝を削除しても panic してはならない。
#[test]
fn remove_subtree_with_sibling_branch_does_not_panic_on_reconcile() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(50, ElementKind::View);
    let branch_a = tree.element_create(51, ElementKind::View);
    let branch_b = tree.element_create(52, ElementKind::View);
    let leaf_a = tree.element_create(53, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(400.0, 300.0);
    tree.element_append_child(root, branch_a);
    tree.element_append_child(root, branch_b);
    tree.element_append_child(branch_a, leaf_a);
    tree.render(0.0);

    tree.element_remove(branch_a);
    tree.commit_frame();

    assert!(
        tree.element_has_taffy_node(branch_b),
        "sibling branch must remain projected"
    );
    assert!(!tree.element_has_taffy_node(branch_a));
    assert!(!tree.element_has_taffy_node(leaf_a));
}

/// レイアウト後に射影済みの部分木を削除しても reconcile で panic してはならない。
#[test]
fn remove_projected_subtree_does_not_panic_on_reconcile() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(30, ElementKind::View);
    let branch = tree.element_create(31, ElementKind::View);
    let leaf = tree.element_create(32, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(400.0, 300.0);
    tree.element_append_child(root, branch);
    tree.element_append_child(branch, leaf);
    tree.render(0.0);

    tree.element_remove(branch);
    tree.commit_frame();
}

/// レイアウト後にインラインテキストを含む部分木を削除しても Taffy ノードを二重削除してはならない。
#[test]
fn remove_subtree_with_inline_text_does_not_panic_on_reconcile() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(40, ElementKind::View);
    let ifc_root = tree.element_create(41, ElementKind::Text);
    let inline = tree.element_create(42, ElementKind::Text);
    tree.set_root(root);
    tree.set_viewport(400.0, 300.0);
    tree.element_append_child(root, ifc_root);
    tree.element_append_child(ifc_root, inline);
    tree.element_set_text(ifc_root, "Hello ");
    tree.element_set_text(inline, "world");
    tree.render(0.0);

    tree.element_remove(ifc_root);
    tree.commit_frame();
}

/// テキストをテキストの下に付け替えると射影クラスが切り替わる（ブロック IFC ルート → インライン）。
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
