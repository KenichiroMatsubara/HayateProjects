//! Canvas Mode で非入力テキストのタップが文字入力判定になる問題。
//!
//! 根本原因: タップは当たった要素を focus する（`pointer_down_on_target` →
//! `transition_focus`）。これは正しく、Chromium もボタン等を pointer-down で
//! focus する（ADR-0102）。バグはソフトキーボード／IME を*生の focus*に紐づけて
//! いた点で、編集可能なのは `text-input` だけなのにプレーンな `text`（やボタン）の
//! タップでキーボードが立ち上がっていた。
//!
//! 修正: focus セマンティクスは不変のまま、編集可能性を別のデータ駆動な軸
//! （[`ElementKind::accepts_text_input`]、出所は `proto/spec/element_kinds.json`）
//! とし、アダプタは `focused_element` でなく [`ElementTree::focused_text_input`]
//! でキーボードをゲートする。

use hayate_core::{Dimension, ElementId, ElementKind, ElementTree, PointerKind, StyleProp};

/// 1 行を占める `<view><text "Hello world"></view>`。(tree, view, text) を返す。
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

/// `<view><text-input "hi"></view>`。(tree, input) を返す。
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

/// `accepts_text_input` が真なのは `text-input` のみ。キーボードゲートの背後に
/// あるデータ駆動な編集可能性の軸。プレーンな `text` はスタイルを担う
/// （Text-Local Carrier）が編集はできない。
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

/// 修正点: 非入力のプレーン `text` への Touch タップは依然それを focus する
/// （Chromium の pointer-focus パリティ、ADR-0102）が、text input ではないので
/// `focused_text_input()` は `None` となり、アダプタはソフトキーボードを上げない。
/// これがまさに本件のリグレッション。
#[test]
fn plain_text_tap_does_not_arm_the_keyboard() {
    let (mut tree, _view, text) = plain_text_doc();
    assert_eq!(tree.focused_text_input(), None, "タップ前はどのフィールドも武装していない");

    tree.on_pointer_down_with_kind(20.0, 8.0, 0, PointerKind::Touch);

    assert_eq!(
        tree.focused_element(),
        Some(text),
        "タップは依然 text 要素を focus する（focus セマンティクスは不変）",
    );
    assert_eq!(
        tree.focused_text_input(),
        None,
        "プレーンな `text` は編集不可なので、ソフトキーボードは上げないこと",
    );
}

/// 対照: `text-input` のタップはフィールドを focus し、かつキーボードを武装する。
/// これは正規の IME 経路で、動作し続けねばならない。
#[test]
fn text_input_tap_arms_the_keyboard() {
    let (mut tree, input) = text_input_doc();
    tree.on_pointer_down_with_kind(20.0, 20.0, 0, PointerKind::Touch);
    assert_eq!(
        tree.focused_element(),
        Some(input),
        "text-input のタップはフィールドを focus すること",
    );
    assert_eq!(
        tree.focused_text_input(),
        Some(input),
        "text-input のタップはソフトキーボード／IME を武装する",
    );
}
