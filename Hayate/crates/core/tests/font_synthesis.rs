//! Element Document パイプラインを通したフォント合成の伝播（ADR-0085）。

use hayate_core::{
    Dimension, ElementKind, ElementTree, FontStyleValue, NodeKind, StyleProp,
};

fn italic_text_tree() -> ElementTree {
    static NOTO_SANS_BYTES: &[u8] = include_bytes!("../assets/fonts/NotoSansJP.ttf");
    let mut tree = ElementTree::new();
    tree.register_font("Noto Sans", NOTO_SANS_BYTES.to_vec());
    let root = tree.element_create(1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(100.0, 100.0);
    let text = tree.element_create(2, ElementKind::Text);
    tree.element_append_child(root, text);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    tree.element_set_style(
        text,
        &[
            StyleProp::FontSize(24.0),
            StyleProp::FontStyle(FontStyleValue::Italic),
        ],
    );
    tree.element_set_text(text, "A");
    tree
}

#[test]
fn element_tree_italic_text_run_carries_faux_skew_synthesis() {
    let mut tree = italic_text_tree();
    let sg = tree.render(0.0);
    let skewed = sg.iter().any(|(_, node)| {
        matches!(&node.kind, NodeKind::TextRun { data, .. } if data.synthesis.skew() == Some(14.0))
    });
    assert!(skewed, "expected faux italic skew on scene graph text run");
}
