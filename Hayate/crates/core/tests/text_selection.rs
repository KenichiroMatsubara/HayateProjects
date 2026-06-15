//! Drag selection within a single selectable IFC (ADR-0097, issue #266) and
//! across multiple blocks within one Selection Region (issue #269).

use hayate_core::{
    DrawOp, Dimension, ElementId, ElementKind, ElementTree, FlexDirectionValue, RecordingPainter,
    StyleProp, render_scene_graph,
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
fn drag_outside_selectable_region_does_not_start_a_selection() {
    let (mut tree, _view, _text) = selectable_paragraph(false);

    tree.on_pointer_down(2.0, 8.0);
    tree.on_pointer_move(70.0, 8.0);

    assert!(
        tree.selection().is_none(),
        "no Selection Region established, so no selection",
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
