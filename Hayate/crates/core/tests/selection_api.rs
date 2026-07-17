//! プログラム的な選択 API と `selection-change` 通知（ADR-0097）。
//! 統一 `Selection` はドキュメント全体で core 所有。ポインタ/キーボード操作ではなく
//! 公開ランタイム API 経由で動作を検証する。

use hayate_core::{
    Dimension, ElementId, ElementKind, ElementTree, Event, SelectionPoint, StyleProp,
};

/// `<view [selectable]><text "Hello world"></view>` を組み立て (tree, view, text) を返す。
/// text 要素が IFC ルート。
fn selectable_paragraph(selectable: bool) -> (ElementTree, ElementId, ElementId) {
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
    if selectable {
        tree.element_set_selectable(view, true);
    }
    tree.render(0.0);
    (tree, view, text)
}

#[test]
fn set_selection_range_makes_the_range_the_active_selection() {
    let (mut tree, _view, text) = selectable_paragraph(true);

    let applied =
        tree.set_selection_range(SelectionPoint::new(text, 0), SelectionPoint::new(text, 5));

    assert!(applied, "a range within a selectable region should apply");
    let sel = tree
        .selection()
        .expect("an active selection after set_selection_range");
    assert_eq!(sel.anchor, SelectionPoint::new(text, 0));
    assert_eq!(sel.focus, SelectionPoint::new(text, 5));
}

#[test]
fn set_selection_range_in_boundary_free_text_applies() {
    // 明示的な `selectable` 領域が無くても、境界フリーの既定（ADR-0108）では
    // 素のテキストへのプログラム的レンジが適用される。両端点が境界なしの
    // ドキュメント領域を共有するため。
    let (mut tree, _view, text) = selectable_paragraph(false);

    let applied =
        tree.set_selection_range(SelectionPoint::new(text, 0), SelectionPoint::new(text, 5));

    assert!(applied, "boundary-free plain text: the range should apply");
    let sel = tree
        .selection()
        .expect("an active selection after set_selection_range");
    assert_eq!(sel.anchor, SelectionPoint::new(text, 0));
    assert_eq!(sel.focus, SelectionPoint::new(text, 5));
}

#[test]
fn set_selection_range_over_user_select_none_is_rejected() {
    // `user-select: none` はテキストを選択対象から除外する（ADR-0108）ため、
    // それを狙ったプログラム的レンジは拒否され、既存の選択はそのまま残る。
    let (mut tree, _view, text) = selectable_paragraph(false);
    tree.element_set_user_select(text, hayate_core::UserSelectValue::None);
    tree.render(0.0);

    let applied =
        tree.set_selection_range(SelectionPoint::new(text, 0), SelectionPoint::new(text, 5));

    assert!(!applied, "user-select: none: the range should be rejected");
    assert!(
        tree.selection().is_none(),
        "a rejected range must leave the selection untouched",
    );
}

#[test]
fn clear_selection_drops_the_active_selection() {
    let (mut tree, _view, text) = selectable_paragraph(true);
    tree.set_selection_range(SelectionPoint::new(text, 0), SelectionPoint::new(text, 5));
    assert!(
        tree.selection().is_some(),
        "precondition: a selection is active"
    );

    tree.clear_selection();

    assert!(
        tree.selection().is_none(),
        "clear_selection should drop the active selection",
    );
}

#[test]
fn changing_the_selection_emits_a_selection_change_event() {
    let (mut tree, _view, text) = selectable_paragraph(true);

    tree.set_selection_range(SelectionPoint::new(text, 0), SelectionPoint::new(text, 5));

    let events = tree.poll_events();
    assert!(
        events.iter().any(|e| matches!(e, Event::SelectionChange)),
        "a selection change should emit a SelectionChange event, got {events:?}",
    );
}

#[test]
fn redundant_set_selection_range_emits_no_new_event() {
    let (mut tree, _view, text) = selectable_paragraph(true);
    tree.set_selection_range(SelectionPoint::new(text, 0), SelectionPoint::new(text, 5));
    let _ = tree.poll_events(); // 最初の変更通知を取り除く

    // 完全に同一のレンジを再設定: 実際には何も変わっていない。
    tree.set_selection_range(SelectionPoint::new(text, 0), SelectionPoint::new(text, 5));

    let events = tree.poll_events();
    assert!(
        !events.iter().any(|e| matches!(e, Event::SelectionChange)),
        "a redundant set must not re-emit SelectionChange, got {events:?}",
    );
}
