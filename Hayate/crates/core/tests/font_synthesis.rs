//! Element Document パイプラインを通したフォント合成の伝播（ADR-0085 / ADR-0054）。
//!
//! 合成値は core scene lowering で解決され、`TextRunData.synthesis` が painter に
//! ready-to-apply な値（スキュー tangent・太らせ量）を載せる。レンダリングバックエンド
//! 無しで、この導出を直接検証する。

use hayate_core::{
    Dimension, ElementKind, ElementTree, FontStyleValue, NodeKind, StyleProp, TextSynthesis,
};

fn styled_text_tree(text_style: &[StyleProp]) -> ElementTree {
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
    let mut style = vec![StyleProp::FontSize(24.0)];
    style.extend_from_slice(text_style);
    tree.element_set_style(text, &style);
    tree.element_set_text(text, "A");
    tree
}

fn text_run_synthesis(tree: &mut ElementTree) -> TextSynthesis {
    let sg = tree.render(0.0);
    sg.iter()
        .find_map(|(_, node)| match &node.kind {
            NodeKind::TextRun { data, .. } => Some(data.synthesis),
            _ => None,
        })
        .expect("expected a text run in the scene graph")
}

#[test]
fn italic_text_run_carries_ready_to_apply_skew_tangent() {
    let mut tree = styled_text_tree(&[StyleProp::FontStyle(FontStyleValue::Italic)]);
    let synthesis = text_run_synthesis(&mut tree);

    let expected_tangent = 14.0_f32.to_radians().tan();
    let tangent = synthesis
        .skew_tangent
        .expect("italic run should carry a skew tangent");
    assert!(
        (tangent - expected_tangent).abs() < 1e-5,
        "expected skew tangent {expected_tangent}, got {tangent}"
    );
    assert!(
        synthesis.embolden.is_none(),
        "italic (non-bold) run should not carry an embolden amount"
    );
}

#[test]
fn regular_text_run_carries_no_synthesis() {
    let mut tree = styled_text_tree(&[]);
    let synthesis = text_run_synthesis(&mut tree);

    assert_eq!(synthesis, TextSynthesis::default());
}
