//! Non-editing text (kind=`text`) is selectable by default and shows the I-beam
//! on hover, with no explicit `selectable`/`user-select: contains` region
//! (ADR-0108 decisions 1+3, ADR-0105). Before ADR-0108 was finished, selection
//! and the I-beam both gated on a `selectable` Selection Region *root*
//! (ADR-0097), so a plain paragraph neither selected on drag nor switched the
//! cursor — these tests lock the opt-out, boundary-free default for both at once.
//!
//! Companion to `text_selection.rs` (which drives selection inside an explicit
//! region) and the `interaction.rs` cursor cases (which gate on `selectable`).

use hayate_core::{CursorValue, Dimension, ElementId, ElementKind, ElementTree, StyleProp};

/// Build `<view><text "Hello world"></view>` on one line with NO explicit
/// `selectable` flag and NO explicit `user-select` — only the element-kind UA
/// default (`text` = selectable, ADR-0108). Returns (tree, view, text). The text
/// element is the IFC root.
fn plain_paragraph() -> (ElementTree, ElementId, ElementId) {
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

#[test]
fn hovering_plain_text_resolves_the_i_beam_cursor() {
    // A plain `text` element carries the kind-default `user-select: text`, so it
    // reads as selectable text and shows the I-beam even without an explicit
    // `cursor` or a `selectable` region (ADR-0105 "選択可能テキスト = text",
    // ADR-0108 default-selectable).
    let (mut tree, _view, _text) = plain_paragraph();

    let result = tree.on_pointer_move(20.0, 8.0);

    assert_eq!(
        result.resolved_cursor,
        CursorValue::Text,
        "plain selectable text must show the I-beam on hover",
    );
}

#[test]
fn dragging_plain_text_starts_a_selection() {
    // Selection is boundary-free by default (ADR-0108 decision 3): a drag over a
    // plain paragraph with no explicit Selection Region still starts a selection.
    let (mut tree, _view, text) = plain_paragraph();

    tree.on_pointer_down(2.0, 8.0);
    tree.on_pointer_move(70.0, 8.0);

    let sel = tree
        .selection()
        .expect("a selection after dragging over plain text");
    let (start, end) = sel
        .range_within(text)
        .expect("both endpoints in the text element");
    assert!(start < end, "expected a non-empty range, got {start}..{end}");
}

#[test]
fn pressing_plain_text_drops_a_caret() {
    // A plain press inside default-selectable text collapses to a caret, the same
    // gesture an explicit Selection Region gets (`text_selection.rs`).
    let (mut tree, _view, text) = plain_paragraph();

    tree.on_pointer_down(40.0, 8.0);

    let sel = tree.selection().expect("a caret on press over plain text");
    assert!(sel.is_caret(), "press without drag is a collapsed caret");
    assert_eq!(sel.anchor.element, text);
}

#[test]
fn dragging_plain_text_yields_copyable_text() {
    // The covered range copies out (ADR-0108 decision 5): selection and copy land
    // together, no "can drag but can't copy" intermediate state.
    let (mut tree, _view, _text) = plain_paragraph();

    tree.on_pointer_down(2.0, 8.0);
    tree.on_pointer_move(70.0, 8.0);

    let copied = tree
        .selected_text()
        .expect("selected text is copyable from a plain paragraph");
    assert!(!copied.is_empty(), "copied text must be non-empty");
    assert!(
        "Hello world".starts_with(&copied),
        "copied prefix should come from the paragraph, got {copied:?}",
    );
}

#[test]
fn user_select_none_text_is_not_selectable_and_keeps_the_default_cursor() {
    // Guard the opt-out polarity: `user-select: none` on the text excludes it
    // from selection and drops the I-beam, even though the kind default is `text`
    // (ADR-0108 decision 2). This keeps the default-selectable change from making
    // *everything* selectable.
    use hayate_core::UserSelectValue;
    let (mut tree, _view, text) = plain_paragraph();
    tree.element_set_user_select(text, UserSelectValue::None);
    tree.render(0.0);

    let hover = tree.on_pointer_move(20.0, 8.0);
    assert_eq!(
        hover.resolved_cursor,
        CursorValue::Default,
        "user-select: none text shows no I-beam",
    );

    tree.on_pointer_down(2.0, 8.0);
    tree.on_pointer_move(70.0, 8.0);
    assert!(
        tree.selection().is_none(),
        "user-select: none text must not start a selection",
    );
}

#[test]
fn hovering_an_empty_view_keeps_the_default_cursor() {
    // A `view` is kind-default `user-select: text` too, but it is not text-bearing
    // — hovering its empty area must stay the arrow, not the I-beam (browser shows
    // the I-beam only over text). Locks the cursor gate to text-bearing elements.
    let mut tree = ElementTree::new();
    let view = tree.element_create(1, ElementKind::View);
    tree.set_root(view);
    tree.set_viewport(400.0, 200.0);
    tree.element_set_style(
        view,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(200.0)),
        ],
    );
    tree.render(0.0);

    let result = tree.on_pointer_move(20.0, 8.0);

    assert_eq!(
        result.resolved_cursor,
        CursorValue::Default,
        "an empty view shows the arrow, not the I-beam",
    );
}
