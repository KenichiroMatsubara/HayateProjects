//! ADR-0069: EditState via ElementTree public input handlers.

use hayate_core::{Dimension, ElementKind, ElementTree, StyleProp};

#[test]
fn on_key_down_backspace_edits_focused_text_input() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(1, ElementKind::TextInput);
    tree.set_root(input);
    tree.element_focus(input);
    tree.element_append_text_content(input, "hello");

    tree.on_key_down("Backspace", 0);

    assert_eq!(tree.element_get_text_content(input), "hell");
}

#[test]
fn on_key_down_enter_inserts_newline_via_edit_state() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(2, ElementKind::TextInput);
    tree.set_root(input);
    tree.element_focus(input);
    tree.element_append_text_content(input, "a");

    tree.on_key_down("Enter", 0);

    assert_eq!(tree.element_get_text_content(input), "a\n");
}

#[test]
fn on_composition_end_commits_via_edit_state() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(3, ElementKind::TextInput);
    tree.set_root(input);
    tree.element_append_text_content(input, "abc");
    tree.on_composition_start(input, "DEF");
    tree.on_composition_end(input, "愛");

    assert_eq!(tree.element_get_text_content(input), "abc愛");
}

#[test]
fn on_text_input_appends_via_edit_state() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(4, ElementKind::TextInput);
    tree.set_root(input);

    tree.on_text_input(input, "x");

    assert_eq!(tree.element_get_text_content(input), "x");
}

#[test]
fn element_character_bounds_available_after_layout() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(5, ElementKind::TextInput);
    tree.set_root(input);
    tree.set_viewport(200.0, 40.0);
    tree.element_set_style(
        input,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::FontSize(16.0),
        ],
    );
    tree.element_append_text_content(input, "hi");
    tree.render(0.0);

    let bounds = tree
        .element_character_bounds(input)
        .expect("character bounds after layout");
    assert!(bounds.width > 0.0);
    assert!(bounds.height > 0.0);
}
