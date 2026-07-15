use hayate_core::element::style::{Dimension, StyleProp};
use hayate_core::{Color, ElementKind, ElementTree};

#[test]
fn frame_commit_returns_one_renderer_ready_view() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let scroll = tree.element_create(1, ElementKind::ScrollView);
    let content = tree.element_create(2, ElementKind::View);
    tree.element_append_child(root, scroll);
    tree.element_append_child(scroll, content);
    tree.set_root(root);
    tree.set_viewport(200.0, 100.0);
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
            StyleProp::Height(Dimension::px(400.0)),
            StyleProp::BackgroundColor(Color::new(0.2, 0.4, 0.6, 1.0)),
        ],
    );

    let frame = tree.commit_rendered_frame(0.0);

    assert!(!frame.scene().is_empty());
    assert_eq!(frame.layers().first(), Some(&root));
    assert!(frame.content_dirty_layers().contains(&root));
    assert!(frame
        .scroll_inputs()
        .iter()
        .any(|input| input.layer == scroll));
    assert_eq!(frame.has_pending_visual_work(), false);
}
