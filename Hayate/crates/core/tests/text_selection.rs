//! Drag selection within a single selectable IFC (ADR-0097, issue #266).

use hayate_core::{
    DrawOp, Dimension, ElementId, ElementKind, ElementTree, RecordingPainter, StyleProp,
    render_scene_graph,
};

fn draw_ops(tree: &ElementTree) -> Vec<DrawOp> {
    let mut painter = RecordingPainter::new();
    render_scene_graph(tree.scene_graph(), &mut painter);
    painter.ops().to_vec()
}

/// Build `<view [selectable]><text "Hello world"></view>` on one line and
/// return (tree, view, text). The text element is the IFC root.
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
fn drag_within_selectable_selects_anchor_to_focus_range() {
    let (mut tree, _view, text) = selectable_paragraph(true);

    // Press near the start of the line, drag rightwards across several glyphs.
    tree.on_pointer_down(2.0, 8.0);
    tree.on_pointer_move(70.0, 8.0);

    let sel = tree.selection().expect("a selection after dragging");
    let (start, end) = sel
        .range_within(text)
        .expect("both endpoints in the text element");
    assert!(start < end, "expected a non-empty range, got {start}..{end}");
    assert_eq!(start, sel.anchor.offset.min(sel.focus.offset));
    assert!(
        sel.focus.offset > sel.anchor.offset,
        "focus should advance past the anchor when dragging rightwards",
    );
}

#[test]
fn pointer_down_in_selectable_collapses_to_a_caret() {
    let (mut tree, _view, text) = selectable_paragraph(true);

    tree.on_pointer_down(40.0, 8.0);

    let sel = tree.selection().expect("a caret on press");
    assert!(sel.is_caret(), "press without drag is a collapsed caret");
    assert_eq!(sel.anchor.element, text);
}

#[test]
fn selected_range_lowers_a_highlight_rect_behind_the_text_run() {
    let (mut tree, _view, _text) = selectable_paragraph(true);

    // No selection yet: no highlight rect under the (background-less) paragraph.
    let before = draw_ops(&tree);
    let rects_before = before
        .iter()
        .filter(|op| matches!(op, DrawOp::FillRect { .. }))
        .count();

    tree.on_pointer_down(2.0, 8.0);
    tree.on_pointer_move(70.0, 8.0);
    tree.render(0.0);

    let ops = draw_ops(&tree);
    let first_rect = ops
        .iter()
        .position(|op| matches!(op, DrawOp::FillRect { .. }));
    let first_text = ops
        .iter()
        .position(|op| matches!(op, DrawOp::DrawTextRun { .. }))
        .expect("the paragraph text run");

    let rect_idx = first_rect.expect("a highlight rect once a range is selected");
    assert!(
        ops.iter()
            .filter(|op| matches!(op, DrawOp::FillRect { .. }))
            .count()
            > rects_before,
        "selecting should add a highlight rect",
    );
    assert!(
        rect_idx < first_text,
        "highlight must paint behind (before) the text run",
    );
    if let DrawOp::FillRect { width, height, .. } = ops[rect_idx] {
        assert!(width > 0.0 && height > 0.0, "highlight has a visible area");
    }
}

/// The substring currently selected within `text`, for asserting gesture ranges.
fn selected_text<'a>(tree: &ElementTree, text: ElementId, content: &'a str) -> &'a str {
    let sel = tree.selection().expect("a selection");
    let (start, end) = sel.range_within(text).expect("both endpoints in text");
    &content[start..end]
}

#[test]
fn double_click_selects_the_word_under_the_pointer() {
    let (mut tree, _view, text) = selectable_paragraph(true);

    // Two presses at the same spot inside "Hello" expand to the whole word.
    tree.on_pointer_down(15.0, 8.0);
    tree.on_pointer_up(15.0, 8.0);
    tree.on_pointer_down(15.0, 8.0);

    assert_eq!(selected_text(&tree, text, "Hello world"), "Hello");
}

#[test]
fn triple_click_selects_the_whole_paragraph() {
    let (mut tree, _view, text) = selectable_paragraph(true);

    tree.on_pointer_down(15.0, 8.0);
    tree.on_pointer_up(15.0, 8.0);
    tree.on_pointer_down(15.0, 8.0);
    tree.on_pointer_up(15.0, 8.0);
    tree.on_pointer_down(15.0, 8.0);

    assert_eq!(selected_text(&tree, text, "Hello world"), "Hello world");
}

const SHIFT: u32 = 1; // MODIFIER_SHIFT (proto/spec wire contract).
const CTRL: u32 = 2; // MODIFIER_CTRL.

#[test]
fn select_all_covers_the_whole_region() {
    let (mut tree, _view, text) = selectable_paragraph(true);

    // A caret must exist in the region first (click to place it), then Ctrl+A.
    tree.on_pointer_down(15.0, 8.0);
    tree.on_pointer_up(15.0, 8.0);
    tree.on_key_down("a", CTRL);

    let sel = tree.selection().expect("a selection after Ctrl+A");
    let (start, end) = sel.range_within(text).expect("both endpoints in text");
    assert_eq!((start, end), (0, "Hello world".len()), "whole region selected");
}

#[test]
fn shift_arrow_extends_the_focus_by_one_character() {
    let (mut tree, _view, text) = selectable_paragraph(true);

    tree.on_pointer_down(8.0, 8.0);
    let anchor = tree.selection().unwrap().anchor;
    let caret = tree.selection().unwrap().focus.offset;
    tree.on_pointer_up(8.0, 8.0);

    tree.on_key_down("ArrowRight", SHIFT);
    let sel = tree.selection().expect("a selection after Shift+ArrowRight");
    assert_eq!(sel.anchor, anchor, "anchor stays fixed");
    assert!(sel.focus.offset > caret, "focus advances one character right");

    // Shift+ArrowLeft contracts back toward (and onto) the anchor.
    let extended = sel.focus.offset;
    tree.on_key_down("ArrowLeft", SHIFT);
    let sel = tree.selection().unwrap();
    assert!(sel.focus.offset < extended, "focus retreats, contracting the range");
    let _ = text;
}

#[test]
fn shift_click_extends_focus_keeping_the_anchor_fixed() {
    let (mut tree, _view, text) = selectable_paragraph(true);

    // Drop a caret near the start, then Shift+click further along the line.
    tree.on_pointer_down(8.0, 8.0);
    let anchor = tree.selection().unwrap().anchor;
    tree.on_pointer_up(8.0, 8.0);

    tree.on_pointer_down_with(70.0, 8.0, SHIFT);

    let sel = tree.selection().expect("a selection after shift+click");
    assert_eq!(sel.anchor, anchor, "anchor must stay where the caret was");
    assert!(
        sel.focus.offset > sel.anchor.offset,
        "focus should extend past the anchor toward the shift+click",
    );
    let (start, end) = sel.range_within(text).expect("both endpoints in text");
    assert!(start < end, "shift+click should produce a non-empty range");
}

#[test]
fn selected_text_returns_the_dragged_substring() {
    let (mut tree, _view, _text) = selectable_paragraph(true);

    // Drag from the start across the first word.
    tree.on_pointer_down(2.0, 8.0);
    tree.on_pointer_move(40.0, 8.0);

    let copied = tree.selected_text().expect("text under the selection");
    let sel = tree.selection().unwrap();
    let (start, end) = sel.range_within(sel.anchor.element).unwrap();
    assert_eq!(copied, &"Hello world"[start..end]);
    assert!(!copied.is_empty(), "a non-empty drag yields some text");
}

/// `<view [selectable]><text "Hello "><text "world" (bigger)></view>`: one IFC
/// made of two inline children with different styles. Returns (tree, ifc root).
fn two_run_paragraph() -> (ElementTree, ElementId) {
    let mut tree = ElementTree::new();
    let view = tree.element_create(1, ElementKind::View);
    let lead = tree.element_create(2, ElementKind::Text);
    let tail = tree.element_create(3, ElementKind::Text);
    tree.set_root(view);
    tree.set_viewport(400.0, 200.0);
    tree.element_set_style(
        view,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(200.0)),
        ],
    );
    tree.element_set_style(lead, &[StyleProp::Width(Dimension::px(400.0))]);
    tree.element_append_child(view, lead);
    tree.element_append_child(lead, tail);
    tree.element_set_text(lead, "Hello ");
    tree.element_set_text(tail, "world");
    tree.element_set_style(tail, &[StyleProp::FontSize(24.0)]);
    tree.element_set_selectable(view, true);
    tree.render(0.0);
    (tree, lead)
}

#[test]
fn selected_text_joins_across_styled_inline_runs() {
    let (mut tree, _ifc) = two_run_paragraph();

    // Select the whole paragraph, which crosses the "Hello " / "world" run
    // boundary (two different font sizes within one IFC).
    tree.on_pointer_down(15.0, 8.0);
    tree.on_pointer_up(15.0, 8.0);
    tree.on_pointer_down(15.0, 8.0);
    tree.on_pointer_up(15.0, 8.0);
    tree.on_pointer_down(15.0, 8.0); // triple-click selects the paragraph

    assert_eq!(
        tree.selected_text().as_deref(),
        Some("Hello world"),
        "the copied text joins both styled runs in document order",
    );
}

/// A `Clipboard` impl that records writes, so a test can assert what core
/// pushed across the Platform Adapter boundary without a real OS clipboard.
#[derive(Default, Clone)]
struct RecordingClipboard {
    writes: std::rc::Rc<std::cell::RefCell<Vec<String>>>,
}

impl hayate_core::Clipboard for RecordingClipboard {
    fn write_text(&self, text: &str) {
        self.writes.borrow_mut().push(text.to_string());
    }
}

#[test]
fn primary_c_writes_the_selection_through_the_clipboard_adapter() {
    let (mut tree, _view, _text) = selectable_paragraph(true);
    let clipboard = RecordingClipboard::default();
    tree.set_clipboard(Box::new(clipboard.clone()));

    // Select a range, then Ctrl/Cmd+C.
    tree.on_pointer_down(2.0, 8.0);
    tree.on_pointer_move(40.0, 8.0);
    let expected = tree.selected_text().expect("a non-empty selection");
    tree.on_pointer_up(40.0, 8.0);
    tree.on_key_down("c", CTRL);

    assert_eq!(
        clipboard.writes.borrow().as_slice(),
        &[expected],
        "the selected text is written once to the clipboard",
    );
}

#[test]
fn primary_c_without_a_selection_writes_nothing() {
    let (mut tree, _view, _text) = selectable_paragraph(true);
    let clipboard = RecordingClipboard::default();
    tree.set_clipboard(Box::new(clipboard.clone()));

    // A caret (collapsed) selects nothing, so copy is a no-op.
    tree.on_pointer_down(40.0, 8.0);
    tree.on_pointer_up(40.0, 8.0);
    tree.on_key_down("c", CTRL);

    assert!(
        clipboard.writes.borrow().is_empty(),
        "copying an empty/caret selection must not write to the clipboard",
    );
}

#[test]
fn selected_text_is_none_for_a_collapsed_caret() {
    let (mut tree, _view, _text) = selectable_paragraph(true);

    // A plain press drops a caret (collapsed selection) — nothing to copy.
    tree.on_pointer_down(40.0, 8.0);
    assert!(tree.selection().unwrap().is_caret());
    assert_eq!(tree.selected_text(), None);
}

#[test]
fn drag_outside_selectable_region_does_not_start_a_selection() {
    let (mut tree, _view, _text) = selectable_paragraph(false);

    tree.on_pointer_down(2.0, 8.0);
    tree.on_pointer_move(70.0, 8.0);

    assert!(
        tree.selection().is_none(),
        "no Selection Region established, so no selection",
    );
}
