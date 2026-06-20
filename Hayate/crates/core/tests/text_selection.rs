//! Drag selection within a single selectable IFC (ADR-0097, issue #266) and
//! across multiple blocks within one Selection Region (issue #269).

use hayate_core::{
    DrawOp, Dimension, ElementId, ElementKind, ElementTree, FlexDirectionValue, RecordingPainter,
    SelectionPoint, StyleProp, UserSelectValue, render_scene_graph,
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

#[test]
fn text_input_range_selection_lowers_a_highlight_behind_the_text() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(1, ElementKind::TextInput);
    tree.set_root(input);
    tree.set_viewport(200.0, 40.0);
    tree.element_set_style(
        input,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::FontSize(16.0),
        ],
    );
    tree.element_append_text_content(input, "hello world");
    tree.element_focus(input);
    tree.render(0.0);

    // A caret alone draws no highlight band.
    assert!(
        highlight_bands(&tree).is_empty(),
        "a collapsed caret shows no selection highlight",
    );

    // Drag across several glyphs, then render: a highlight band appears.
    tree.on_pointer_down(2.0, 20.0);
    tree.on_pointer_move(70.0, 20.0);
    tree.render(0.0);

    let bands = highlight_bands(&tree);
    assert!(
        !bands.is_empty(),
        "selecting a range inside the text-input lowers a highlight",
    );

    let ops = draw_ops(&tree);
    let first_highlight = ops
        .iter()
        .position(|op| matches!(op, DrawOp::FillRect { color, .. } if *color == HIGHLIGHT_COLOR))
        .expect("a highlight rect");
    let first_text = ops
        .iter()
        .position(|op| matches!(op, DrawOp::DrawTextRun { .. }))
        .expect("the field's text run");
    assert!(
        first_highlight < first_text,
        "the highlight must paint behind the text run",
    );
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
fn drag_over_user_select_none_does_not_start_a_selection() {
    // Selection is boundary-free by default (ADR-0108 decision 3), so the absence
    // of a `selectable` region no longer blocks it — opting out is now explicit,
    // via `user-select: none`, which excludes the subtree (decision 2). A drag
    // over such a paragraph starts nothing. (The boundary-free *positive* case —
    // plain text selects on drag — lives in `plain_text_selection.rs`.)
    let (mut tree, _view, text) = selectable_paragraph(false);
    tree.element_set_user_select(text, UserSelectValue::None);
    tree.render(0.0);

    tree.on_pointer_down(2.0, 8.0);
    tree.on_pointer_move(70.0, 8.0);

    assert!(
        tree.selection().is_none(),
        "user-select: none text must not start a selection",
    );
}

// --- Cross-block selection within one Selection Region (issue #269) ---

/// Build a column `<view [selectable]>` stacking two paragraphs (separate IFC
/// blocks). Returns (tree, view, first, second). Each paragraph is one line.
fn two_block_region(selectable: bool) -> (ElementTree, ElementId, ElementId, ElementId) {
    let mut tree = ElementTree::new();
    let view = tree.element_create(1, ElementKind::View);
    let first = tree.element_create(2, ElementKind::Text);
    let second = tree.element_create(3, ElementKind::Text);
    tree.set_root(view);
    tree.set_viewport(400.0, 200.0);
    tree.element_set_style(
        view,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(200.0)),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
        ],
    );
    tree.element_set_style(first, &[StyleProp::Width(Dimension::px(400.0))]);
    tree.element_set_style(second, &[StyleProp::Width(Dimension::px(400.0))]);
    tree.element_append_child(view, first);
    tree.element_append_child(view, second);
    tree.element_set_text(first, "First block");
    tree.element_set_text(second, "Second block");
    if selectable {
        tree.element_set_selectable(view, true);
    }
    tree.render(0.0);
    (tree, view, first, second)
}

/// The vertical center of a paragraph's line, for clicking into it.
fn block_mid_y(tree: &ElementTree, block: ElementId) -> f32 {
    let (_, y, _, h) = tree.element_layout_rect(block).expect("a laid-out block");
    y + h / 2.0
}

#[test]
fn dragging_backwards_across_blocks_normalizes_to_document_order() {
    let (mut tree, _view, first, second) = two_block_region(true);

    // Press inside the *second* block and drag up into the *first*: the anchor
    // is in the later block, the focus in the earlier one.
    tree.on_pointer_down(60.0, block_mid_y(&tree, second));
    tree.on_pointer_move(20.0, block_mid_y(&tree, first));

    let sel = tree.selection().expect("a cross-block selection");
    assert_eq!(sel.anchor.element, second, "anchor stays where the drag began");
    assert_eq!(sel.focus.element, first, "focus follows the drag into block one");

    let (start, end) = tree
        .selection_ordered()
        .expect("ordered endpoints for an active selection");
    assert_eq!(
        start.element, first,
        "document order puts the earlier block's point first",
    );
    assert_eq!(end.element, second, "the later block's point comes last");
}

#[test]
fn selected_text_joins_cross_block_selection_with_a_newline() {
    let (mut tree, _view, first, second) = two_block_region(true);

    // Select from the start of the first block through the end of the second,
    // so the range spans the block-box (IFC root) boundary between them.
    let applied = tree.set_selection_range(
        SelectionPoint::new(first, 0),
        SelectionPoint::new(second, "Second block".len()),
    );
    assert!(applied, "both blocks share one Selection Region");

    assert_eq!(
        tree.selected_text().as_deref(),
        Some("First block\nSecond block"),
        "a cross-block copy joins blocks in document order with a single \\n at the block boundary",
    );
}

#[test]
fn cross_block_copy_follows_document_order_not_anchor_first() {
    let (mut tree, _view, first, second) = two_block_region(true);

    // Anchor in the *later* block, focus in the earlier one (a backward drag).
    // The copied text must still read in document order, not anchor-first.
    let applied = tree.set_selection_range(
        SelectionPoint::new(second, "Second block".len()),
        SelectionPoint::new(first, 0),
    );
    assert!(applied, "both blocks share one Selection Region");

    assert_eq!(
        tree.selected_text().as_deref(),
        Some("First block\nSecond block"),
        "copy joins blocks in document order regardless of drag direction",
    );
}

/// Build a selectable column of three stacked paragraphs (three IFC blocks).
/// Returns (tree, view, first, middle, last). Each paragraph is one line.
fn three_block_region() -> (ElementTree, ElementId, ElementId, ElementId, ElementId) {
    let mut tree = ElementTree::new();
    let view = tree.element_create(1, ElementKind::View);
    let first = tree.element_create(2, ElementKind::Text);
    let middle = tree.element_create(3, ElementKind::Text);
    let last = tree.element_create(4, ElementKind::Text);
    tree.set_root(view);
    tree.set_viewport(400.0, 300.0);
    tree.element_set_style(
        view,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(300.0)),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
        ],
    );
    for &block in &[first, middle, last] {
        tree.element_set_style(block, &[StyleProp::Width(Dimension::px(400.0))]);
        tree.element_append_child(view, block);
    }
    tree.element_set_text(first, "First block");
    tree.element_set_text(middle, "Middle block");
    tree.element_set_text(last, "Third block");
    tree.element_set_selectable(view, true);
    tree.render(0.0);
    (tree, view, first, middle, last)
}

#[test]
fn user_select_none_block_is_excluded_from_the_copied_text() {
    let (mut tree, _view, first, middle, last) = three_block_region();
    // The middle paragraph opts out of selection (CSS `user-select: none`).
    tree.element_set_user_select(middle, UserSelectValue::None);
    tree.render(0.0);

    let applied = tree.set_selection_range(
        SelectionPoint::new(first, 0),
        SelectionPoint::new(last, "Third block".len()),
    );
    assert!(applied, "first and last share one Selection Region");

    assert_eq!(
        tree.selected_text().as_deref(),
        Some("First block\nThird block"),
        "a user-select:none block contributes no text and leaves no orphan newline",
    );
}

// --- `user-select: contains` containment boundary (ADR-0108 decision 3, #400) ---

/// A `selectable` outer column holding `inside` — a paragraph wrapped in a
/// `user-select: contains` box — above `outside`, a bare paragraph that shares
/// the outer Selection Region. With `contains` the inner box is its own tighter
/// boundary; without it both paragraphs span freely. Returns
/// (tree, contains-box, inside-paragraph, outside-paragraph).
fn contains_inside_region(boundary: bool) -> (ElementTree, ElementId, ElementId, ElementId) {
    let mut tree = ElementTree::new();
    let outer = tree.element_create(1, ElementKind::View);
    let boundary_box = tree.element_create(2, ElementKind::View);
    let inside = tree.element_create(3, ElementKind::Text);
    let outside = tree.element_create(4, ElementKind::Text);
    tree.set_root(outer);
    tree.set_viewport(400.0, 200.0);
    tree.element_set_style(
        outer,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(200.0)),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
        ],
    );
    tree.element_set_style(
        boundary_box,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
        ],
    );
    tree.element_set_style(inside, &[StyleProp::Width(Dimension::px(400.0))]);
    tree.element_set_style(outside, &[StyleProp::Width(Dimension::px(400.0))]);
    tree.element_append_child(outer, boundary_box);
    tree.element_append_child(boundary_box, inside);
    tree.element_append_child(outer, outside);
    tree.element_set_text(inside, "Inside box");
    tree.element_set_text(outside, "Outside box");
    tree.element_set_selectable(outer, true);
    if boundary {
        tree.element_set_user_select(boundary_box, UserSelectValue::Contains);
    }
    tree.render(0.0);
    (tree, boundary_box, inside, outside)
}

#[test]
fn contains_box_clamps_a_drag_inside_its_boundary() {
    let (mut tree, _box, inside, outside) = contains_inside_region(true);

    // Begin the drag in the `contains` box and pull down into the sibling that
    // lies outside it (but still inside the outer selectable region).
    tree.on_pointer_down(20.0, block_mid_y(&tree, inside));
    tree.on_pointer_move(80.0, block_mid_y(&tree, outside));

    let sel = tree
        .selection()
        .expect("a selection started inside the contains box");
    assert_eq!(
        sel.focus.element, inside,
        "focus must stay clamped inside the `user-select: contains` boundary",
    );
}

/// A `selectable` outer column whose middle child is a `user-select: contains`
/// box holding two stacked paragraphs (`in_a`, `in_b`); an `outside` paragraph
/// follows the box in the same outer region. Returns
/// (tree, in_a, in_b, outside).
fn contains_box_with_two_blocks() -> (ElementTree, ElementId, ElementId, ElementId) {
    let mut tree = ElementTree::new();
    let outer = tree.element_create(1, ElementKind::View);
    let boundary_box = tree.element_create(2, ElementKind::View);
    let in_a = tree.element_create(3, ElementKind::Text);
    let in_b = tree.element_create(4, ElementKind::Text);
    let outside = tree.element_create(5, ElementKind::Text);
    tree.set_root(outer);
    tree.set_viewport(400.0, 300.0);
    tree.element_set_style(
        outer,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(300.0)),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
        ],
    );
    tree.element_set_style(
        boundary_box,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
        ],
    );
    for &block in &[in_a, in_b, outside] {
        tree.element_set_style(block, &[StyleProp::Width(Dimension::px(400.0))]);
    }
    tree.element_append_child(outer, boundary_box);
    tree.element_append_child(boundary_box, in_a);
    tree.element_append_child(boundary_box, in_b);
    tree.element_append_child(outer, outside);
    tree.element_set_text(in_a, "Alpha box");
    tree.element_set_text(in_b, "Beta box");
    tree.element_set_text(outside, "Gamma out");
    tree.element_set_selectable(outer, true);
    tree.element_set_user_select(boundary_box, UserSelectValue::Contains);
    tree.render(0.0);
    (tree, in_a, in_b, outside)
}

#[test]
fn contains_boundary_excludes_outside_blocks_from_copied_text() {
    let (mut tree, in_a, in_b, outside) = contains_box_with_two_blocks();

    // A selection spanning the two paragraphs inside the box is honoured: the
    // copied text joins them in document order with a single block-boundary `\n`.
    let inside = tree.set_selection_range(
        SelectionPoint::new(in_a, 0),
        SelectionPoint::new(in_b, "Beta box".len()),
    );
    assert!(inside, "both paragraphs lie inside the same `contains` boundary");
    assert_eq!(
        tree.selected_text().as_deref(),
        Some("Alpha box\nBeta box"),
        "the two in-box paragraphs join; copy stays within the boundary",
    );

    // A range that would cross the boundary into the outside paragraph is
    // refused outright, so the outside text is never concatenated.
    let leaked = tree.set_selection_range(
        SelectionPoint::new(in_a, 0),
        SelectionPoint::new(outside, "Gamma out".len()),
    );
    assert!(
        !leaked,
        "a range crossing the `contains` boundary is refused, never copied",
    );
}

#[test]
fn without_contains_a_drag_spans_freely_across_the_box() {
    // The regression contrast to `contains_box_clamps_a_drag_inside_its_boundary`:
    // the identical layout with the box left as a plain view (no `contains`) lets
    // a drag begun inside it run on into the sibling paragraph — the default is a
    // free cross-element span, and `contains` is the only thing that clamps it.
    let (mut tree, _box, inside, outside) = contains_inside_region(false);

    tree.on_pointer_down(20.0, block_mid_y(&tree, inside));
    tree.on_pointer_move(80.0, block_mid_y(&tree, outside));

    let sel = tree.selection().expect("a selection started inside the box");
    assert_eq!(
        sel.focus.element, outside,
        "with no `contains` boundary the focus follows the drag into the sibling",
    );
}

/// Two nested `user-select: contains` boxes: an outer boundary holding
/// `outer_block` above an inner boundary holding `inner_block`. Both are
/// containment boundaries, but they are distinct regions (nearest wins).
/// Returns (tree, outer_block, inner_block).
fn nested_contains() -> (ElementTree, ElementId, ElementId) {
    let mut tree = ElementTree::new();
    let outer = tree.element_create(1, ElementKind::View);
    let outer_block = tree.element_create(2, ElementKind::Text);
    let inner = tree.element_create(3, ElementKind::View);
    let inner_block = tree.element_create(4, ElementKind::Text);
    tree.set_root(outer);
    tree.set_viewport(400.0, 200.0);
    tree.element_set_style(
        outer,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(200.0)),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
        ],
    );
    tree.element_set_style(
        inner,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
        ],
    );
    tree.element_set_style(outer_block, &[StyleProp::Width(Dimension::px(400.0))]);
    tree.element_set_style(inner_block, &[StyleProp::Width(Dimension::px(400.0))]);
    tree.element_append_child(outer, outer_block);
    tree.element_append_child(outer, inner);
    tree.element_append_child(inner, inner_block);
    tree.element_set_text(outer_block, "Outer box");
    tree.element_set_text(inner_block, "Inner box");
    tree.element_set_user_select(outer, UserSelectValue::Contains);
    tree.element_set_user_select(inner, UserSelectValue::Contains);
    tree.render(0.0);
    (tree, outer_block, inner_block)
}

#[test]
fn nested_contains_uses_the_innermost_boundary() {
    let (mut tree, outer_block, inner_block) = nested_contains();

    // A drag begun in the outer box must not extend into the nested box: the
    // inner `contains` is the nearer boundary of `inner_block`.
    tree.on_pointer_down(20.0, block_mid_y(&tree, outer_block));
    tree.on_pointer_move(80.0, block_mid_y(&tree, inner_block));

    let sel = tree.selection().expect("a selection in the outer box");
    assert_eq!(
        sel.focus.element, outer_block,
        "focus stays in the outer box; the nested `contains` is its own boundary",
    );

    // Conversely, a fresh drag begun inside the nested box anchors there.
    tree.on_pointer_down(20.0, block_mid_y(&tree, inner_block));
    let nested = tree.selection().expect("a caret in the nested box");
    assert_eq!(
        nested.anchor.element, inner_block,
        "a press in the nested box anchors to the innermost boundary",
    );
}

/// Material selection tint (ADR-0097): identifies highlight rects among draw ops.
const HIGHLIGHT_COLOR: [f32; 4] = [0.20, 0.45, 0.95, 0.35];

/// The vertical bands (y_min..y_max) of every selection-highlight rect.
fn highlight_bands(tree: &ElementTree) -> Vec<(f32, f32)> {
    draw_ops(tree)
        .iter()
        .filter_map(|op| match op {
            DrawOp::FillRect { y, height, color, .. } if *color == HIGHLIGHT_COLOR => {
                Some((*y, *y + *height))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn dragging_across_blocks_highlights_every_covered_block() {
    let (mut tree, _view, first, second) = two_block_region(true);

    // Drag from the first paragraph down into the second: the selection spans
    // both blocks, so each must show its own highlight run.
    tree.on_pointer_down(20.0, block_mid_y(&tree, first));
    tree.on_pointer_move(80.0, block_mid_y(&tree, second));
    tree.render(0.0);

    let bands = highlight_bands(&tree);
    let (_, fy, _, fh) = tree.element_layout_rect(first).unwrap();
    let (_, sy, _, sh) = tree.element_layout_rect(second).unwrap();
    let covers = |y0: f32, y1: f32| {
        bands
            .iter()
            .any(|&(by0, by1)| by1 > y0 && by0 < y1)
    };
    assert!(covers(fy, fy + fh), "the first block must be highlighted");
    assert!(covers(sy, sy + sh), "the second block must be highlighted");
}

#[test]
fn user_select_none_block_shows_no_highlight() {
    let (mut tree, _view, first, middle, last) = three_block_region();
    tree.element_set_user_select(middle, UserSelectValue::None);

    // Select across all three blocks; the middle one opts out of selection.
    tree.set_selection_range(
        SelectionPoint::new(first, 0),
        SelectionPoint::new(last, "Third block".len()),
    );
    tree.render(0.0);

    let bands = highlight_bands(&tree);
    // Test a band over each block's vertical *center*, so a neighbouring band
    // grazing a box edge by a pixel (line metrics vs box geometry) is not
    // mistaken for the middle block being highlighted.
    let covered = |block: ElementId| {
        let mid = block_mid_y(&tree, block);
        bands.iter().any(|&(by0, by1)| by0 <= mid && mid <= by1)
    };
    assert!(covered(first), "the first block must be highlighted");
    assert!(covered(last), "the last block must be highlighted");
    assert!(
        !covered(middle),
        "a user-select:none block carries no highlight, just as it copies no text",
    );
}

#[test]
fn dragging_across_two_text_blocks_highlights_both_and_copies_them_joined() {
    // The headline cross-element case (ADR-0108 decision 5): one drag spanning
    // two text blocks must both paint a highlight over each block *and* copy the
    // two joined in document order — highlight and copy land together.
    let (mut tree, _view, first, second) = two_block_region(true);

    // Drag from before the first paragraph to past the end of the second.
    tree.on_pointer_down(2.0, block_mid_y(&tree, first));
    tree.on_pointer_move(398.0, block_mid_y(&tree, second));
    tree.render(0.0);

    let bands = highlight_bands(&tree);
    let covered = |block: ElementId| {
        let mid = block_mid_y(&tree, block);
        bands.iter().any(|&(by0, by1)| by0 <= mid && mid <= by1)
    };
    assert!(covered(first), "the first block is highlighted by the drag");
    assert!(covered(second), "the second block is highlighted by the drag");

    // The same drag copies both blocks, joined in document order by exactly one
    // `\n` at the block boundary. The drag's far end is pixel-dependent, so the
    // exact full join is pinned by the programmatic test above; here we assert
    // the structural shape: first block whole, second block from its start.
    let copied = tree.selected_text().expect("a cross-block drag copies text");
    assert_eq!(copied.matches('\n').count(), 1, "one block-boundary newline: {copied:?}");
    let (lead, tail) = copied.split_once('\n').unwrap();
    assert_eq!(lead, "First block", "the first block is copied whole");
    assert!(
        !tail.is_empty() && "Second block".starts_with(tail),
        "the second block is copied from its start, got {tail:?}",
    );
}

/// A non-selectable column root holding `inner` — a `selectable` view with one
/// paragraph — above `outside`, a paragraph in no Selection Region. Returns
/// (tree, inner-paragraph, outside-paragraph).
fn region_with_outside_block() -> (ElementTree, ElementId, ElementId) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    let inner = tree.element_create(2, ElementKind::View);
    let inside = tree.element_create(3, ElementKind::Text);
    let outside = tree.element_create(4, ElementKind::Text);
    tree.set_root(root);
    tree.set_viewport(400.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(200.0)),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
        ],
    );
    tree.element_set_style(
        inner,
        &[StyleProp::Width(Dimension::px(400.0)), StyleProp::FlexDirection(FlexDirectionValue::Column)],
    );
    tree.element_set_style(inside, &[StyleProp::Width(Dimension::px(400.0))]);
    tree.element_set_style(outside, &[StyleProp::Width(Dimension::px(400.0))]);
    tree.element_append_child(root, inner);
    tree.element_append_child(inner, inside);
    tree.element_append_child(root, outside);
    tree.element_set_text(inside, "Inside region");
    tree.element_set_text(outside, "Outside region");
    tree.element_set_selectable(inner, true);
    tree.render(0.0);
    (tree, inside, outside)
}

#[test]
fn selection_does_not_leak_past_the_selectable_boundary() {
    let (mut tree, inside, outside) = region_with_outside_block();

    // Start inside the region, drag down into the block that lies outside it.
    tree.on_pointer_down(20.0, block_mid_y(&tree, inside));
    tree.on_pointer_move(80.0, block_mid_y(&tree, outside));
    tree.render(0.0);

    let sel = tree.selection().expect("a selection started inside the region");
    assert_eq!(
        sel.focus.element, inside,
        "focus must stay clamped inside the Selection Region",
    );

    // The outside block carries no highlight band.
    let (_, oy, _, oh) = tree.element_layout_rect(outside).unwrap();
    let leaked = highlight_bands(&tree)
        .iter()
        .any(|&(by0, by1)| by1 > oy && by0 < oy + oh);
    assert!(!leaked, "no highlight may appear outside the Selection Region");
}

/// An `outer` selectable column holding `outer_block` above a *nested* selectable
/// view that holds `inner_block`. Both blocks are selectable, but they belong to
/// different Selection Regions (nearest ancestor wins). Returns
/// (tree, outer_block, inner_block).
fn nested_regions() -> (ElementTree, ElementId, ElementId) {
    let mut tree = ElementTree::new();
    let outer = tree.element_create(1, ElementKind::View);
    let outer_block = tree.element_create(2, ElementKind::Text);
    let inner = tree.element_create(3, ElementKind::View);
    let inner_block = tree.element_create(4, ElementKind::Text);
    tree.set_root(outer);
    tree.set_viewport(400.0, 200.0);
    tree.element_set_style(
        outer,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(200.0)),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
        ],
    );
    tree.element_set_style(
        inner,
        &[StyleProp::Width(Dimension::px(400.0)), StyleProp::FlexDirection(FlexDirectionValue::Column)],
    );
    tree.element_set_style(outer_block, &[StyleProp::Width(Dimension::px(400.0))]);
    tree.element_set_style(inner_block, &[StyleProp::Width(Dimension::px(400.0))]);
    tree.element_append_child(outer, outer_block);
    tree.element_append_child(outer, inner);
    tree.element_append_child(inner, inner_block);
    tree.element_set_text(outer_block, "Outer region");
    tree.element_set_text(inner_block, "Nested region");
    tree.element_set_selectable(outer, true);
    tree.element_set_selectable(inner, true);
    tree.render(0.0);
    (tree, outer_block, inner_block)
}

#[test]
fn nested_region_uses_the_nearest_selectable_ancestor() {
    let (mut tree, outer_block, inner_block) = nested_regions();

    // A drag begun in the outer region must not extend into the nested region:
    // the nested `selectable` is the nearer ancestor of `inner_block`.
    tree.on_pointer_down(20.0, block_mid_y(&tree, outer_block));
    tree.on_pointer_move(80.0, block_mid_y(&tree, inner_block));

    let sel = tree.selection().expect("a selection in the outer region");
    assert_eq!(
        sel.focus.element, outer_block,
        "focus stays in the outer region; the nested region is its own boundary",
    );

    // Conversely, a fresh drag begun inside the nested region selects there.
    tree.on_pointer_down(20.0, block_mid_y(&tree, inner_block));
    let nested = tree.selection().expect("a caret in the nested region");
    assert_eq!(
        nested.anchor.element, inner_block,
        "a press in the nested region anchors to the nested block",
    );
}
