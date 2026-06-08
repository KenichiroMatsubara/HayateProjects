//! ADR-0067: shared effective visual resolver + query API.

use hayate_core::{
    Color, Dimension, ElementKind, ElementTree, PseudoState, StyleProp,
};

#[test]
fn element_effective_visual_applies_hover_pseudo() {
    let mut tree = ElementTree::new();
    let id = tree.element_create(1, ElementKind::View);
    tree.set_root(id);
    tree.element_set_style(
        id,
        &[StyleProp::BackgroundColor(Color::new(1.0, 1.0, 1.0, 1.0))],
    );
    tree.element_set_pseudo_style(
        id,
        PseudoState::Hover,
        &[StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0))],
    );

    let base = tree.element_effective_visual(id).unwrap();
    assert_eq!(base.background_color, Some(Color::new(1.0, 1.0, 1.0, 1.0)));

    tree.update_pointer_hover(Some(id));
    let hovered = tree.element_effective_visual(id).unwrap();
    assert_eq!(
        hovered.background_color,
        Some(Color::new(0.0, 0.0, 1.0, 1.0)),
        ":hover pseudo must apply via element_effective_visual"
    );
}

#[test]
fn element_effective_visual_resolves_ambient_default_on_text() {
    let mut tree = ElementTree::new();
    let view = tree.element_create(2, ElementKind::View);
    let text = tree.element_create(3, ElementKind::Text);
    tree.set_root(view);
    tree.set_viewport(200.0, 100.0);
    tree.element_append_child(view, text);
    tree.element_set_style(
        view,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(100.0)),
            StyleProp::DefaultColor(Color::new(1.0, 0.0, 0.0, 1.0)),
        ],
    );
    tree.element_set_text(text, "hi");

    let visual = tree.element_effective_visual(text).unwrap();
    assert_eq!(
        visual.text_color,
        Some(Color::new(1.0, 0.0, 0.0, 1.0)),
        "ambient default-color must resolve on text element"
    );
}

#[test]
fn element_effective_visual_text_to_text_inheritance() {
    let mut tree = ElementTree::new();
    let view = tree.element_create(4, ElementKind::View);
    let ifc = tree.element_create(5, ElementKind::Text);
    let inline = tree.element_create(6, ElementKind::Text);
    tree.set_root(view);
    tree.element_append_child(view, ifc);
    tree.element_append_child(ifc, inline);
    tree.element_set_style(ifc, &[StyleProp::FontSize(20.0)]);
    tree.element_set_text(ifc, "A");
    tree.element_set_text(inline, "B");

    let visual = tree.element_effective_visual(inline).unwrap();
    assert!(
        (visual.font_size.unwrap() - 20.0).abs() < 0.1,
        "inline text must inherit IFC root font-size"
    );
}

#[test]
fn view_font_size_does_not_leak_via_effective_visual() {
    let mut tree = ElementTree::new();
    let view = tree.element_create(7, ElementKind::View);
    let text = tree.element_create(8, ElementKind::Text);
    tree.set_root(view);
    tree.element_append_child(view, text);
    tree.element_set_style(view, &[StyleProp::FontSize(24.0)]);
    tree.element_set_text(text, "x");

    let visual = tree.element_effective_visual(text).unwrap();
    assert!(
        (visual.font_size.unwrap() - 16.0).abs() < 0.1,
        "view font-size must not leak to child text"
    );
}
