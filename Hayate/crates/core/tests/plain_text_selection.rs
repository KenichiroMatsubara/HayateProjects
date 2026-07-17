//! 非編集テキスト（kind=`text`）は既定で選択可能であり、明示的な
//! `selectable`/`user-select: contains` 領域なしでもホバーで I-beam を出す
//! （ADR-0108, ADR-0105）。これらのテストはオプトアウト方式かつ境界不要の
//! 既定挙動（選択とカーソルの両方）を固定する。

use hayate_core::{CursorValue, Dimension, ElementId, ElementKind, ElementTree, StyleProp};

/// 1行の `<view><text "Hello world"></view>` を、明示的な `selectable` も
/// `user-select` も付けずに構築する（element-kind の UA 既定 `text` = 選択可能、
/// ADR-0108 のみに依存）。(tree, view, text) を返す。text 要素が IFC ルート。
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
    // 素の `text` 要素は kind 既定の `user-select: text` を持つため、明示的な
    // `cursor` や `selectable` 領域なしでも選択可能テキストとして I-beam を出す
    // （ADR-0105, ADR-0108）。
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
    // 選択は既定で境界不要（ADR-0108）。明示的な Selection Region のない素の
    // 段落でも、ドラッグで選択が始まる。
    let (mut tree, _view, text) = plain_paragraph();

    tree.on_pointer_down(2.0, 8.0);
    tree.on_pointer_move(70.0, 8.0);

    let sel = tree
        .selection()
        .expect("a selection after dragging over plain text");
    let (start, end) = sel
        .range_within(text)
        .expect("both endpoints in the text element");
    assert!(
        start < end,
        "expected a non-empty range, got {start}..{end}"
    );
}

#[test]
fn pressing_plain_text_drops_a_caret() {
    // 既定で選択可能なテキスト内での素の押下はキャレットに collapse する
    // （明示的な Selection Region と同じ挙動、`text_selection.rs`）。
    let (mut tree, _view, text) = plain_paragraph();

    tree.on_pointer_down(40.0, 8.0);

    let sel = tree.selection().expect("a caret on press over plain text");
    assert!(sel.is_caret(), "press without drag is a collapsed caret");
    assert_eq!(sel.anchor.element, text);
}

#[test]
fn dragging_plain_text_yields_copyable_text() {
    // 選択範囲はそのままコピーできる（ADR-0108）。「ドラッグできるがコピー
    // できない」中間状態を作らず、選択とコピーは同時に成立する。
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
    // オプトアウトの極性を守る。kind 既定が `text` でも、テキストへの
    // `user-select: none` は選択から除外し I-beam を落とす（ADR-0108）。
    // 既定選択可能化が「すべてを選択可能」にしないための歯止め。
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
    // `view` も kind 既定は `user-select: text` だが、テキストを持たない。空の
    // 領域へのホバーは矢印のままで、I-beam にしない（ブラウザは I-beam を
    // テキスト上でのみ出す）。カーソル判定をテキスト保持要素に限定する。
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
