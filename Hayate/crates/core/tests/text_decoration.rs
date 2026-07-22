//! text-decoration の underline / line-through を run ローカル座標で配置する。

use hayate_core::{
    Color, Dimension, ElementKind, ElementTree, NodeKind, StyleProp, TextDecorationValue,
};

fn text_run_placements(sg: &hayate_core::SceneGraph) -> Vec<(f32, f32, f32)> {
    sg.iter()
        .filter_map(|(_, n)| match &n.kind {
            NodeKind::TextRun { text_run, .. } => {
                let data = sg.resources().text_run(*text_run).ok()?;
                let min_glyph_y = data
                    .glyphs
                    .iter()
                    .map(|g| g.y)
                    .fold(f32::INFINITY, f32::min);
                let max_glyph_y = data
                    .glyphs
                    .iter()
                    .map(|g| g.y)
                    .fold(f32::NEG_INFINITY, f32::max);
                Some((
                    min_glyph_y,
                    max_glyph_y,
                    data.decorations.first().map(|d| d.y).unwrap_or(f32::NAN),
                ))
            }
            _ => None,
        })
        .collect()
}

fn underline_tree() -> ElementTree {
    let mut tree = ElementTree::new();
    let view = tree.element_create(1, ElementKind::View);
    let text = tree.element_create(2, ElementKind::Text);
    tree.set_root(view);
    tree.set_viewport(100.0, 100.0);
    tree.element_append_child(view, text);
    tree.element_set_style(
        view,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    tree.element_set_style(
        text,
        &[
            StyleProp::FontSize(24.0),
            StyleProp::Color(Color::new(0.0, 0.0, 0.0, 1.0)),
            StyleProp::TextDecoration(TextDecorationValue::Underline),
        ],
    );
    tree.element_set_text(text, "A");
    tree
}

fn line_through_tree() -> ElementTree {
    let mut tree = ElementTree::new();
    let view = tree.element_create(3, ElementKind::View);
    let text = tree.element_create(4, ElementKind::Text);
    tree.set_root(view);
    tree.set_viewport(100.0, 100.0);
    tree.element_append_child(view, text);
    tree.element_set_style(
        view,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
        ],
    );
    tree.element_set_style(
        text,
        &[
            StyleProp::FontSize(24.0),
            StyleProp::Color(Color::new(0.0, 0.0, 0.0, 1.0)),
            StyleProp::TextDecoration(TextDecorationValue::LineThrough),
        ],
    );
    tree.element_set_text(text, "O");
    tree
}

#[test]
fn underline_sits_below_glyph_body() {
    let mut tree = underline_tree();
    let sg = tree.render(0.0);
    let placements = text_run_placements(&sg);
    assert_eq!(placements.len(), 1, "expected one text run");
    let (_min_glyph_y, max_glyph_y, deco_y) = placements[0];
    assert!(
        deco_y > max_glyph_y + 1.0,
        "underline must sit below the glyph body (max_glyph_y={max_glyph_y}, deco_y={deco_y})"
    );
}

#[test]
fn line_through_sits_above_glyph_bottom() {
    let mut tree = line_through_tree();
    let sg = tree.render(0.0);
    let placements = text_run_placements(&sg);
    assert_eq!(placements.len(), 1, "expected one text run");
    let (_min_glyph_y, max_glyph_y, deco_y) = placements[0];
    assert!(
        deco_y < max_glyph_y - 2.0,
        "line-through must sit above the baseline anchor (max_glyph_y={max_glyph_y}, deco_y={deco_y})"
    );
}
