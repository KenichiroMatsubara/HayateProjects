//! 選択クロムの Touch モダリティゲート（ADR-0104）。ドラッグハンドルと
//! フローティングツールバーは Touch のアフォーダンスで、最後のポインタ操作が
//! Touch のときだけ描かれる。Mouse/Pen は細いキャレットとドラッグ選択のみ
//! （デスクトップブラウザの挙動）。ハイライトの色味はゲートされず、どの
//! モダリティでも描かれる（ADR-0097、tint=Chromium）。
//!
//! 公開 `ElementTree` インターフェース経由で検証する（クロムのクエリ
//! `selection_toolbar` / `selection_handles` とレンダリング済み SceneGraph）。

use hayate_core::{
    Dimension, DrawOp, ElementId, ElementKind, ElementTree, PointerKind, RecordingPainter,
    StyleProp, render_scene_graph,
};

/// Material 選択の色味（ADR-0097）。選択ハイライト矩形の色。
const HIGHLIGHT_COLOR: [f32; 4] = [0.20, 0.45, 0.95, 0.35];

fn draw_ops(tree: &ElementTree) -> Vec<DrawOp> {
    let mut painter = RecordingPainter::new();
    render_scene_graph(tree.scene_graph(), &mut painter);
    painter.ops().to_vec()
}

fn has_highlight(tree: &ElementTree) -> bool {
    draw_ops(tree)
        .iter()
        .any(|op| matches!(op, DrawOp::FillRect { color, .. } if *color == HIGHLIGHT_COLOR))
}

/// 1行の `<view [selectable]><text "Hello world"></view>` を組み立て、
/// (tree, view, text) を返す。`selection_toolbar.rs` のハーネスと同型。
fn selectable_paragraph() -> (ElementTree, ElementId, ElementId) {
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
    tree.element_set_selectable(view, true);
    tree.render(0.0);
    (tree, view, text)
}

/// 指定したポインタモダリティで先頭範囲をドラッグ選択し、離す。
fn drag_select_with(tree: &mut ElementTree, kind: PointerKind) {
    tree.on_pointer_down_with_kind(2.0, 8.0, 0, kind);
    tree.on_pointer_move_with_kind(70.0, 8.0, kind);
    tree.on_pointer_up_with_kind(70.0, 8.0, kind);
}

#[test]
fn mouse_drag_select_shows_no_toolbar() {
    let (mut tree, _view, _text) = selectable_paragraph();
    drag_select_with(&mut tree, PointerKind::Mouse);

    assert!(
        tree.selected_text().is_some(),
        "a Mouse drag still makes a non-empty selection",
    );
    assert!(
        tree.selection_toolbar().is_none(),
        "a Mouse selection shows no floating toolbar (desktop behaviour)",
    );
}

#[test]
fn mouse_drag_select_raises_no_handles() {
    let (mut tree, _view, _text) = selectable_paragraph();
    drag_select_with(&mut tree, PointerKind::Mouse);

    assert!(
        tree.selection().is_some(),
        "a Mouse drag still makes a selection",
    );
    assert!(
        tree.selection_handles().is_none(),
        "a Mouse selection raises no drag handles (desktop behaviour)",
    );
}

#[test]
fn pen_drag_select_shows_no_chrome() {
    let (mut tree, _view, _text) = selectable_paragraph();
    drag_select_with(&mut tree, PointerKind::Pen);

    assert!(tree.selected_text().is_some(), "a Pen drag still selects");
    assert!(
        tree.selection_toolbar().is_none() && tree.selection_handles().is_none(),
        "Pen is a precise pointer like Mouse — no handles or toolbar",
    );
}

#[test]
fn touch_drag_select_shows_handles_and_toolbar() {
    let (mut tree, _view, _text) = selectable_paragraph();
    drag_select_with(&mut tree, PointerKind::Touch);

    assert!(
        tree.selection_toolbar().is_some(),
        "a Touch selection raises the floating toolbar",
    );
    assert!(
        tree.selection_handles().is_some(),
        "a Touch selection raises the drag handles",
    );
}

#[test]
fn mouse_selection_still_paints_the_highlight_tint() {
    // ハイライトの色味はモダリティでゲートされない（ADR-0097、tint=Chromium）。
    // Mouse 選択はクロムを描かないが、ハイライト帯は描く。
    let (mut tree, _view, _text) = selectable_paragraph();
    drag_select_with(&mut tree, PointerKind::Mouse);
    tree.render(0.0);

    assert!(
        tree.selection_toolbar().is_none(),
        "no chrome under Mouse modality",
    );
    assert!(
        has_highlight(&tree),
        "the selection highlight tint is drawn regardless of modality",
    );
}

#[test]
fn long_press_is_treated_as_touch_and_shows_chrome() {
    // 長押しはモバイルのジェスチャ。それが始める単語選択は Touch モダリティなので、
    // 直前のポインタ種別が未設定でもクロムが現れる。
    let (mut tree, _view, _text) = selectable_paragraph();
    tree.on_long_press(10.0, 8.0);

    assert_eq!(tree.last_pointer_kind(), PointerKind::Touch);
    assert!(
        tree.selection_toolbar().is_some() && tree.selection_handles().is_some(),
        "long-press chrome is shown (mobile gesture is Touch)",
    );
}
