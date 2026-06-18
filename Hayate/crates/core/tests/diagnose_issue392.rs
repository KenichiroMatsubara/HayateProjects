//! #392 — Canvas Mode で非入力テキストのタップが文字入力判定になる。
//!
//! Root cause: a tap focuses whatever it hits (`pointer_down_on_target` →
//! `transition_focus`), which is correct — Chromium focuses buttons and other
//! widgets on pointer-down too (ADR-0102). The bug was that the soft keyboard /
//! IME was keyed on *raw focus*, so tapping a plain `text` (or a button) raised
//! the keyboard even though only a `text-input` is editable.
//!
//! Fix: focus semantics are unchanged; editability is a separate, data-driven
//! axis ([`ElementKind::accepts_text_input`], sourced from
//! `proto/spec/element_kinds.json`), and adapters gate the keyboard on
//! [`ElementTree::focused_text_input`] instead of `focused_element`.

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

/// `accepts_text_input` is true for `text-input` only — the data-driven
/// editability axis behind the keyboard gate. Plain `text` carries styles
/// (Text-Local Carrier) but is not editable.
#[test]
fn only_text_input_accepts_text_input() {
    assert!(ElementKind::TextInput.accepts_text_input());
    for kind in [
        ElementKind::View,
        ElementKind::Text,
        ElementKind::Image,
        ElementKind::Button,
        ElementKind::ScrollView,
    ] {
        assert!(
            !kind.accepts_text_input(),
            "{kind:?} must not accept text input",
        );
    }
}

/// The fix: a Touch tap on a plain non-input `text` still focuses it (Chromium
/// pointer-focus parity, ADR-0102), but it is NOT a text input, so
/// `focused_text_input()` is `None` and the adapter leaves the soft keyboard
/// down. This is exactly the #392 regression.
#[test]
fn plain_text_tap_does_not_arm_the_keyboard() {
    let (mut tree, _view, text) = plain_text_doc();
    assert_eq!(tree.focused_text_input(), None, "no field armed before the tap");

    tree.on_pointer_down_with_kind(20.0, 8.0, 0, PointerKind::Touch);

    assert_eq!(
        tree.focused_element(),
        Some(text),
        "the tap still focuses the text element (focus semantics unchanged)",
    );
    assert_eq!(
        tree.focused_text_input(),
        None,
        "a plain `text` is not editable, so the soft keyboard must stay down (#392)",
    );
}

/// Control: a `text-input` tap focuses the field AND arms the keyboard. This is
/// the legitimate IME path and must keep working.
#[test]
fn text_input_tap_arms_the_keyboard() {
    let (mut tree, input) = text_input_doc();
    tree.on_pointer_down_with_kind(20.0, 20.0, 0, PointerKind::Touch);
    assert_eq!(
        tree.focused_element(),
        Some(input),
        "a text-input tap must focus the field",
    );
    assert_eq!(
        tree.focused_text_input(),
        Some(input),
        "a text-input tap arms the soft keyboard / IME",
    );
}
