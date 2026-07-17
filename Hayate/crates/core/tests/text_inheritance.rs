//! ADR-0065: 2チャネルのテキスト継承（text-local + ambient default）。

use hayate_core::{Color, Dimension, ElementId, ElementKind, ElementTree, NodeKind, StyleProp};

fn text_run_font_sizes(sg: &hayate_core::SceneGraph) -> Vec<f32> {
    sg.iter()
        .filter_map(|(_, n)| match &n.kind {
            NodeKind::TextRun { data, .. } => Some(data.font_size),
            _ => None,
        })
        .collect()
}

fn setup_view_with_text(
    tree: &mut ElementTree,
    view_id: u64,
    text_id: u64,
) -> (ElementId, ElementId) {
    let view = tree.element_create(view_id, ElementKind::View);
    let text = tree.element_create(text_id, ElementKind::Text);
    tree.set_root(view);
    tree.set_viewport(400.0, 300.0);
    tree.element_append_child(view, text);
    tree.element_set_style(
        view,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(300.0)),
        ],
    );
    tree.element_set_text(text, "Hello");
    (view, text)
}

#[test]
fn view_font_size_does_not_leak_to_child_text() {
    let mut tree = ElementTree::new();
    let (view, _text) = setup_view_with_text(&mut tree, 1, 2);
    tree.element_set_style(view, &[StyleProp::FontSize(24.0)]);
    let sg = tree.render(0.0);
    let sizes = text_run_font_sizes(&sg);
    assert!(!sizes.is_empty(), "expected TextRun");
    assert!(
        sizes.iter().all(|s| (*s - 16.0).abs() < 0.5),
        "child text must use hard default 16px, not view font-size 24: {sizes:?}"
    );
}

#[test]
fn default_font_size_penetrates_block_to_text() {
    let mut tree = ElementTree::new();
    let (view, _text) = setup_view_with_text(&mut tree, 3, 4);
    tree.element_set_style(view, &[StyleProp::DefaultFontSize(20.0)]);
    let sg = tree.render(0.0);
    let sizes = text_run_font_sizes(&sg);
    assert!(
        sizes.iter().any(|s| (*s - 20.0).abs() < 0.5),
        "ambient default-font-size must reach child text: {sizes:?}"
    );
}

#[test]
fn text_to_text_font_size_inherits_in_ifc() {
    let mut tree = ElementTree::new();
    let view = tree.element_create(5, ElementKind::View);
    let ifc = tree.element_create(6, ElementKind::Text);
    let inline = tree.element_create(7, ElementKind::Text);
    tree.set_root(view);
    tree.set_viewport(400.0, 300.0);
    tree.element_append_child(view, ifc);
    tree.element_append_child(ifc, inline);
    tree.element_set_style(
        view,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(300.0)),
        ],
    );
    tree.element_set_style(ifc, &[StyleProp::FontSize(18.0)]);
    tree.element_set_text(ifc, "Hi ");
    tree.element_set_text(inline, "there");
    let sg = tree.render(0.0);
    let sizes = text_run_font_sizes(&sg);
    assert!(
        sizes.iter().any(|s| (*s - 18.0).abs() < 0.5),
        "inline text must inherit IFC root font-size: {sizes:?}"
    );
}

#[test]
fn own_default_color_applies_to_self_text() {
    // ADR-0065（解釈A）: 要素自身の `default-*` は self-inclusive。祖先が別の ambient 既定を
    // 供給していても、text 自身の `default-color` がその text 自身の glyph に効く。react-todo の
    // 完了ラベル相当で、DOM Renderer の挙動とも一致する。
    let mut tree = ElementTree::new();
    let view = tree.element_create(10, ElementKind::View);
    let text = tree.element_create(11, ElementKind::Text);
    tree.set_root(view);
    tree.set_viewport(400.0, 300.0);
    tree.element_append_child(view, text);
    tree.element_set_style(
        view,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(300.0)),
            // 祖先 ambient = 赤（祖先 ink 相当）。
            StyleProp::DefaultColor(Color::new(1.0, 0.0, 0.0, 1.0)),
        ],
    );
    // text 自身の default-color = 緑（muted 相当）。
    tree.element_set_style(
        text,
        &[StyleProp::DefaultColor(Color::new(0.0, 1.0, 0.0, 1.0))],
    );
    tree.element_set_text(text, "done");
    let sg = tree.render(0.0);
    let colors: Vec<[f32; 4]> = sg
        .iter()
        .filter_map(|(_, n)| match &n.kind {
            NodeKind::TextRun { color, .. } => Some(*color),
            _ => None,
        })
        .collect();
    assert!(!colors.is_empty());
    assert!(
        colors
            .iter()
            .any(|c| (c[1] - 1.0).abs() < 0.05 && (c[0] - 0.0).abs() < 0.05),
        "text 自身の default-color が自分の glyph に効くこと（self-inclusive）: {colors:?}"
    );
}

#[test]
fn own_default_font_size_applies_to_self_text() {
    // 解釈A: text 自身の default-font-size が自分の glyph に効く（祖先 ambient より優先）。
    let mut tree = ElementTree::new();
    let view = tree.element_create(12, ElementKind::View);
    let text = tree.element_create(13, ElementKind::Text);
    tree.set_root(view);
    tree.set_viewport(400.0, 300.0);
    tree.element_append_child(view, text);
    tree.element_set_style(
        view,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(300.0)),
            StyleProp::DefaultFontSize(14.0),
        ],
    );
    tree.element_set_style(text, &[StyleProp::DefaultFontSize(22.0)]);
    tree.element_set_text(text, "title");
    let sg = tree.render(0.0);
    let sizes = text_run_font_sizes(&sg);
    assert!(
        sizes.iter().any(|s| (*s - 22.0).abs() < 0.5),
        "text 自身の default-font-size が自分の glyph に効くこと: {sizes:?}"
    );
}

#[test]
fn default_color_penetrates_block_to_text() {
    let mut tree = ElementTree::new();
    let (view, _text) = setup_view_with_text(&mut tree, 8, 9);
    tree.element_set_style(
        view,
        &[StyleProp::DefaultColor(Color::new(1.0, 0.0, 0.0, 1.0))],
    );
    let sg = tree.render(0.0);
    let colors: Vec<[f32; 4]> = sg
        .iter()
        .filter_map(|(_, n)| match &n.kind {
            NodeKind::TextRun { color, .. } => Some(*color),
            _ => None,
        })
        .collect();
    assert!(!colors.is_empty());
    assert!(
        colors
            .iter()
            .any(|c| (c[0] - 1.0).abs() < 0.05 && (c[1] - 0.0).abs() < 0.05),
        "ambient default-color must reach child text: {colors:?}"
    );
}
