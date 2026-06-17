//! Touch-modality gate for selection chrome (ADR-0104 decision 2, #365). The
//! drag handles and floating toolbar are a Touch affordance: they are drawn only
//! when the last pointer interaction came from Touch. Mouse/Pen get the thin
//! caret and drag-select only (desktop-browser behaviour). The highlight tint is
//! *not* gated — it paints under every modality (ADR-0097, tint=Chromium).
//!
//! Exercised through the public `ElementTree` interface: the chrome queries
//! (`selection_toolbar` / `selection_handles`) and the rendered SceneGraph.

use hayate_core::{
    Dimension, DrawOp, ElementId, ElementKind, ElementTree, PointerKind, RecordingPainter,
    StyleProp, render_scene_graph,
};

/// Material selection tint (ADR-0097): the colour of a selection highlight rect.
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

/// Build `<view [selectable]><text "Hello world"></view>` on one line and
/// return (tree, view, text). Mirrors the harness in `selection_toolbar.rs`.
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

/// Drag-select a leading range with the given pointer modality, then release.
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
    // The highlight tint is not modality-gated (ADR-0097, tint=Chromium): a Mouse
    // selection draws no chrome but still paints its highlight band.
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
    // A long-press is a mobile gesture: the word selection it starts is Touch
    // modality, so its chrome appears even with no prior pointer kind set (#365).
    let (mut tree, _view, _text) = selectable_paragraph();
    tree.on_long_press(10.0, 8.0);

    assert_eq!(tree.last_pointer_kind(), PointerKind::Touch);
    assert!(
        tree.selection_toolbar().is_some() && tree.selection_handles().is_some(),
        "long-press chrome is shown (mobile gesture is Touch)",
    );
}
