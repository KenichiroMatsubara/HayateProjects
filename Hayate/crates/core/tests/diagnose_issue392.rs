//! Diagnose #392 — Canvas Mode で非入力テキストのタップが文字入力判定になる。
//!
//! This is a *diagnostic* harness (the issue's Phase 1/2 feedback loop). It does
//! not yet assert the fixed behaviour; it pins the *current* core behaviour so
//! the root-cause claim is empirical: a plain `text` tap drives `transition_focus`
//! exactly like a `text-input` tap, because `pointer_down_on_target` focuses any
//! hit element with no focusability model.

use hayate_core::{Dimension, ElementId, ElementKind, ElementTree, PointerKind, StyleProp};

/// `<view><text "Hello world"></view>` filling a line; returns (tree, view, text).
fn plain_text_doc() -> (ElementTree, ElementId, ElementId) {
    let mut tree = ElementTree::new();
    let view = tree.element_create(1, ElementKind::View);
    let text = tree.element_create(2, ElementKind::Text);
    tree.set_root(view);
    tree.set_viewport(400.0, 200.0);
    tree.element_set_style(
        view,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(200.0)),
        ],
    );
    tree.element_set_style(text, &[StyleProp::Width(Dimension::px(400.0))]);
    tree.element_append_child(view, text);
    tree.element_set_text(text, "Hello world");
    tree.render(0.0);
    (tree, view, text)
}

/// `<view><text-input "hi"></view>`; returns (tree, input).
fn text_input_doc() -> (ElementTree, ElementId) {
    let mut tree = ElementTree::new();
    let view = tree.element_create(1, ElementKind::View);
    let input = tree.element_create(2, ElementKind::TextInput);
    tree.set_root(view);
    tree.set_viewport(400.0, 200.0);
    tree.element_set_style(
        view,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(200.0)),
        ],
    );
    tree.element_set_style(
        input,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::FontSize(16.0),
        ],
    );
    tree.element_append_child(view, input);
    tree.element_append_text_content(input, "hi");
    tree.render(0.0);
    (tree, input)
}

/// REGRESSION TARGET for the fix (red today, `#[ignore]`d until #392 is fixed).
///
/// DESIRED (DOM-parity) behaviour: a Touch tap on a plain non-input `text` must
/// NOT focus anything. Today it focuses the text element, because
/// `pointer_down_on_target` → `transition_focus` focuses any hit element with no
/// focusability model. The Canvas adapter then keys its always-attached
/// EditContext / soft-keyboard off `focused_element_id() != 0`, so this stray
/// focus is what raises the IME on a tap that should be inert. DOM Renderer is
/// immune only because `text` materialises as a non-focusable `<span>`.
///
/// Un-`#[ignore]` this when the focusability fix lands; it asserts the fixed
/// state directly.
#[test]
#[ignore = "diagnose #392: red regression target — plain text must not be focusable (fix pending)"]
fn plain_text_tap_must_not_focus() {
    let (mut tree, _view, _text) = plain_text_doc();
    assert_eq!(tree.focused_element(), None, "nothing focused before the tap");

    tree.on_pointer_down_with_kind(20.0, 8.0, 0, PointerKind::Touch);

    assert_eq!(
        tree.focused_element(),
        None,
        "a plain `text` tap must leave focus untouched (DOM parity: text is not \
         focusable), so the Canvas EditContext / soft-keyboard never activates",
    );
}

/// Control: a `text-input` tap legitimately focuses the field. This must keep
/// working after any focusability fix.
#[test]
fn text_input_tap_focuses_the_field() {
    let (mut tree, input) = text_input_doc();
    tree.on_pointer_down_with_kind(20.0, 20.0, 0, PointerKind::Touch);
    assert_eq!(
        tree.focused_element(),
        Some(input),
        "a text-input tap must focus the field",
    );
}
