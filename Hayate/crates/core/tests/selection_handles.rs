//! Material drag handles + long-press selection — mobile-flavored selection
//! chrome (ADR-0097, issue #273). Handles, their geometry, handle-drag endpoint
//! adjustment and long-press word selection are exercised through the public
//! `ElementTree` interface.

use hayate_core::{
    Dimension, DrawOp, ElementId, ElementKind, ElementTree, PointerKind, RecordingPainter,
    SelectionHandleEnd, StyleProp, render_scene_graph,
};

fn draw_ops(tree: &ElementTree) -> Vec<DrawOp> {
    let mut painter = RecordingPainter::new();
    render_scene_graph(tree.scene_graph(), &mut painter);
    painter.ops().to_vec()
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

/// Touch drag-select a leading range, then release. Leaves a non-empty selection
/// under Touch modality, so its drag handles are raised (ADR-0104, #365).
fn select_a_range(tree: &mut ElementTree) {
    tree.on_pointer_down_with_kind(2.0, 8.0, 0, PointerKind::Touch);
    tree.on_pointer_move(70.0, 8.0);
    tree.on_pointer_up(70.0, 8.0);
}

#[test]
fn selection_raises_a_handle_at_each_end() {
    let (mut tree, _view, _text) = selectable_paragraph();
    select_a_range(&mut tree);

    let handles = tree
        .selection_handles()
        .expect("a non-empty selection raises drag handles");
    assert_eq!(handles.start.end, SelectionHandleEnd::Start);
    assert_eq!(handles.end.end, SelectionHandleEnd::End);
    // The start handle sits left of the end handle for a left-to-right range.
    assert!(
        handles.start.knob_x < handles.end.knob_x,
        "start handle is left of the end handle",
    );
    // Both knobs hang below the single text line.
    assert!(handles.start.knob_y > 0.0);
}

#[test]
fn no_handles_without_a_selection() {
    let (tree, _view, _text) = selectable_paragraph();
    assert!(
        tree.selection_handles().is_none(),
        "no selection means no handles",
    );
}

#[test]
fn chrome_style_switch_recolors_the_handles_and_is_additive() {
    use hayate_core::SelectionChromeStyle;

    let knob_color = |style: SelectionChromeStyle| -> [f32; 4] {
        let (mut tree, _v, _t) = selectable_paragraph();
        tree.set_selection_chrome_style(style);
        select_a_range(&mut tree);
        tree.render(0.0);
        let h = tree.selection_handles().expect("handles");
        draw_ops(&tree)
            .into_iter()
            .find_map(|op| match op {
                DrawOp::FillRect { x, y, width, height, corner_radius, color }
                    if (x + width / 2.0 - h.start.knob_x).abs() < 0.5
                        && (y + height / 2.0 - h.start.knob_y).abs() < 0.5
                        && (corner_radius - width / 2.0).abs() < 0.5 =>
                {
                    Some(color)
                }
                _ => None,
            })
            .expect("the start handle knob rect")
    };

    // Material is the default; switching to Cupertino is additive (the same
    // handle model, a different theme) and recolors the knob.
    assert_eq!(SelectionChromeStyle::default(), SelectionChromeStyle::Material);
    assert_ne!(
        knob_color(SelectionChromeStyle::Material),
        knob_color(SelectionChromeStyle::Cupertino),
        "the chrome style enum drives a visibly different handle",
    );
}

/// A filled circular knob (a square FillRect with a corner radius equal to half
/// its side) centered at `(kx, ky)`, regardless of color.
fn knob_drawn_at(ops: &[DrawOp], kx: f32, ky: f32) -> bool {
    ops.iter().any(|op| {
        matches!(op,
            DrawOp::FillRect { x, y, width, height, corner_radius, .. }
                if (x + width / 2.0 - kx).abs() < 0.5
                    && (y + height / 2.0 - ky).abs() < 0.5
                    && (width - height).abs() < 0.5
                    && (corner_radius - width / 2.0).abs() < 0.5
                    && *corner_radius > 0.0)
    })
}

#[test]
fn handles_are_drawn_by_core_during_selection() {
    let (mut tree, _v, _t) = selectable_paragraph();
    select_a_range(&mut tree);
    tree.render(0.0);

    let handles = tree.selection_handles().expect("handles after selecting");
    let ops = draw_ops(&tree);
    assert!(
        knob_drawn_at(&ops, handles.start.knob_x, handles.start.knob_y),
        "the start handle knob is drawn at its position",
    );
    assert!(
        knob_drawn_at(&ops, handles.end.knob_x, handles.end.knob_y),
        "the end handle knob is drawn at its position",
    );
}

#[test]
fn handles_disappear_from_the_scene_when_the_selection_clears() {
    let (mut tree, _v, _t) = selectable_paragraph();
    select_a_range(&mut tree);
    tree.render(0.0);
    let handles = tree.selection_handles().expect("handles");
    let (sx, sy) = (handles.start.knob_x, handles.start.knob_y);
    assert!(knob_drawn_at(&draw_ops(&tree), sx, sy), "knob present while selecting");

    // Tap empty space to clear the selection, then re-render.
    tree.on_pointer_down(2.0, 150.0);
    tree.on_pointer_up(2.0, 150.0);
    tree.render(0.0);

    assert!(tree.selection_handles().is_none(), "selection cleared");
    assert!(
        !knob_drawn_at(&draw_ops(&tree), sx, sy),
        "the handle overlay is removed once the selection clears",
    );
}

/// Like `selectable_paragraph` but with headroom above the text so the floating
/// toolbar settles *above* the selection, leaving the drag handles below it
/// unobstructed — the common mobile layout, and the one a handle-drag test needs
/// so the press lands on a handle and not a toolbar button. Text sits near y=88.
fn selectable_paragraph_with_headroom() -> (ElementTree, ElementId, ElementId) {
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
            StyleProp::PaddingTop(Dimension::px(80.0)),
        ],
    );
    tree.element_set_style(text, &[StyleProp::Width(Dimension::px(400.0))]);
    tree.element_append_child(view, text);
    tree.element_set_text(text, "Hello world");
    tree.element_set_selectable(view, true);
    tree.render(0.0);
    (tree, view, text)
}

#[test]
fn dragging_the_end_handle_extends_the_range() {
    let (mut tree, _view, text) = selectable_paragraph_with_headroom();
    // Touch-select a short leading range near the text line (~y=88).
    tree.on_pointer_down_with_kind(2.0, 88.0, 0, PointerKind::Touch);
    tree.on_pointer_move(40.0, 88.0);
    tree.on_pointer_up(40.0, 88.0);
    let before = tree.selection().unwrap().range_within(text).unwrap();

    // Grab the end handle and drag it to the far right edge of the text.
    let handles = tree.selection_handles().expect("handles after selecting");
    tree.on_pointer_down(handles.end.knob_x, handles.end.knob_y);
    tree.on_pointer_move(398.0, 88.0);
    tree.on_pointer_up(398.0, 88.0);

    let after = tree.selection().unwrap().range_within(text).unwrap();
    assert_eq!(after.0, before.0, "the left edge stays put");
    assert!(after.1 > before.1, "dragging the end handle extends the range");
}

#[test]
fn dragging_the_start_handle_moves_the_left_edge() {
    let (mut tree, _view, text) = selectable_paragraph_with_headroom();
    // Touch-select the whole first word range.
    tree.on_pointer_down_with_kind(2.0, 88.0, 0, PointerKind::Touch);
    tree.on_pointer_move(90.0, 88.0);
    tree.on_pointer_up(90.0, 88.0);
    let before = tree.selection().unwrap().range_within(text).unwrap();

    // Grab the start handle and drag it rightward, shrinking from the left.
    let handles = tree.selection_handles().expect("handles after selecting");
    tree.on_pointer_down(handles.start.knob_x, handles.start.knob_y);
    tree.on_pointer_move(40.0, 88.0);
    tree.on_pointer_up(40.0, 88.0);

    let after = tree.selection().unwrap().range_within(text).unwrap();
    assert_eq!(after.1, before.1, "the right edge stays put");
    assert!(after.0 > before.0, "dragging the start handle moves the left edge in");
}

#[test]
fn long_press_starts_a_word_selection_with_handles_and_toolbar() {
    let (mut tree, _view, text) = selectable_paragraph();

    // Long-press inside the first word "Hello" (bytes 0..5).
    tree.on_long_press(10.0, 8.0);

    let sel = tree.selection().expect("long-press selects a word");
    assert_eq!(
        sel.range_within(text),
        Some((0, 5)),
        "the word under the long-press is selected",
    );
    assert_eq!(tree.selected_text().as_deref(), Some("Hello"));
    assert!(
        tree.selection_handles().is_some(),
        "word selection raises drag handles",
    );
    assert!(
        tree.selection_toolbar().is_some(),
        "word selection raises the floating toolbar",
    );
}

#[test]
fn long_press_outside_a_selectable_region_selects_nothing() {
    let (mut tree, _view, _text) = selectable_paragraph();
    // Far below the single text line but still over the (selectable) view edge,
    // and past the right of the viewport — no glyph to anchor a word on.
    tree.on_long_press(2000.0, 8.0);
    assert!(tree.selection().is_none(), "no word, no selection");
}

#[test]
fn a_collapsed_caret_raises_no_handles() {
    // A single tap drops a caret (collapsed selection) but no two-ended handles.
    let (mut tree, _view, _text) = selectable_paragraph();
    tree.on_pointer_down(2.0, 8.0);
    tree.on_pointer_up(2.0, 8.0);
    assert!(
        tree.selection_handles().is_none(),
        "a collapsed caret shows no range handles",
    );
}
