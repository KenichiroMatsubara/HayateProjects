//! ADR-0069: EditState via ElementTree public input handlers.

use hayate_core::{
    CompositionClause, CompositionUnderline, Dimension, Direction, EditIntent, ElementKind,
    ElementTree, Granularity, StyleProp,
};

const SHIFT: u32 = 1; // MODIFIER_SHIFT (proto/spec wire contract).

/// A focused text-input carrying `content` with the caret at its end. No layout
/// is needed for caret/selection-index assertions.
fn focused_input(content: &str) -> (ElementTree, hayate_core::ElementId) {
    let mut tree = ElementTree::new();
    let input = tree.element_create(100, ElementKind::TextInput);
    tree.set_root(input);
    tree.element_focus(input);
    tree.element_append_text_content(input, content);
    (tree, input)
}

#[test]
fn bare_arrow_moves_the_caret_one_grapheme() {
    // ADR-0103: the plain arrow keys move the caret (previously a no-op — only
    // Shift+Arrow did anything). "aあb" exercises a multibyte grapheme step.
    let (mut tree, input) = focused_input("aあb"); // caret at end (5)

    tree.on_key_down("ArrowLeft", 0);
    assert_eq!(tree.element_caret_byte_index(input), Some(4), "retreats past 'b'");
    tree.on_key_down("ArrowLeft", 0);
    assert_eq!(tree.element_caret_byte_index(input), Some(1), "retreats past 'あ'");

    tree.on_key_down("ArrowRight", 0);
    assert_eq!(tree.element_caret_byte_index(input), Some(4), "advances past 'あ'");

    // A bare arrow leaves the selection collapsed (a caret, not a range).
    assert!(tree.element_text_selection(input).is_none());
}

#[test]
fn bare_arrow_over_a_selection_collapses_to_its_edge() {
    // With a range selected, the plain arrow collapses to the edge rather than
    // stepping past it (Chromium <input> behavior).
    let (mut tree, input) = focused_input("hello"); // caret at end (5)
    tree.on_key_down("ArrowLeft", SHIFT);
    tree.on_key_down("ArrowLeft", SHIFT); // selects "lo" → range (3,5)
    assert_eq!(tree.element_text_selection(input), Some((3, 5)));

    tree.on_key_down("ArrowLeft", 0);
    assert_eq!(
        tree.element_caret_byte_index(input),
        Some(3),
        "collapses to the left edge of the former selection",
    );
    assert!(tree.element_text_selection(input).is_none());
}

#[test]
fn apply_edit_intent_is_the_os_independent_entry_point() {
    // The Platform Adapter maps an OS keystroke to an intent and drives this
    // seam directly; core never sees which key produced it (ADR-0103).
    let (mut tree, input) = focused_input("hello"); // caret at 5

    assert!(tree.apply_edit_intent(
        input,
        EditIntent::Move {
            granularity: Granularity::Grapheme,
            direction: Direction::Backward,
        },
    ));
    assert_eq!(tree.element_caret_byte_index(input), Some(4));

    assert!(tree.apply_edit_intent(
        input,
        EditIntent::Extend {
            granularity: Granularity::Grapheme,
            direction: Direction::Backward,
        },
    ));
    assert_eq!(tree.element_text_selection(input), Some((3, 4)));
}

#[test]
fn boundary_intents_move_and_extend_the_caret_to_the_field_ends() {
    // ADR-0103 / #360: Home/End and Ctrl+Home/End map (in the adapter) to
    // Line/Doc boundary intents; core applies them through the OS-independent
    // seam. In single-line semantics every boundary is the field end.
    let (mut tree, input) = focused_input("hello world"); // caret at end (11)

    // Home (Move/LineBoundary/Backward) collapses the caret to the start.
    assert!(tree.apply_edit_intent(
        input,
        EditIntent::Move {
            granularity: Granularity::LineBoundary,
            direction: Direction::Backward,
        },
    ));
    assert_eq!(tree.element_caret_byte_index(input), Some(0), "Home → field start");
    assert!(tree.element_text_selection(input).is_none(), "a Move stays collapsed");

    // Shift+End (Extend/LineBoundary/Forward) selects from the caret to the end.
    assert!(tree.apply_edit_intent(
        input,
        EditIntent::Extend {
            granularity: Granularity::LineBoundary,
            direction: Direction::Forward,
        },
    ));
    assert_eq!(
        tree.element_text_selection(input),
        Some((0, 11)),
        "Shift+End extends the selection to the field end, anchor fixed at 0",
    );

    // Ctrl+Home (Move/DocBoundary/Backward) collapses back to the start.
    assert!(tree.apply_edit_intent(
        input,
        EditIntent::Move {
            granularity: Granularity::DocBoundary,
            direction: Direction::Backward,
        },
    ));
    assert_eq!(tree.element_caret_byte_index(input), Some(0));
    assert!(tree.element_text_selection(input).is_none());
}

#[test]
fn arrow_keys_do_not_disturb_an_active_ime_composition() {
    // ADR-0103: a caret key while an IME preedit is active must not edit or break
    // the composition. The intent is not consumed; the preedit and content stay.
    let mut tree = ElementTree::new();
    let input = tree.element_create(101, ElementKind::TextInput);
    tree.set_root(input);
    tree.element_focus(input);
    tree.element_append_text_content(input, "ab"); // caret at 2
    tree.on_composition_start(input, "きゅ"); // active preedit

    let consumed = tree.apply_edit_intent(
        input,
        EditIntent::Move {
            granularity: Granularity::Grapheme,
            direction: Direction::Backward,
        },
    );
    assert!(!consumed, "intent is refused while composing");
    assert_eq!(tree.element_caret_byte_index(input), Some(2), "caret unmoved");
    assert_eq!(
        tree.element_get_text_content(input),
        "abきゅ",
        "the composition is preserved intact",
    );
}

#[test]
fn shift_arrow_extends_text_input_selection_then_typing_replaces_it() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(10, ElementKind::TextInput);
    tree.set_root(input);
    tree.element_focus(input);
    tree.element_append_text_content(input, "hello"); // caret at end

    // Shift+ArrowLeft twice selects the last two characters ("lo").
    tree.on_key_down("ArrowLeft", SHIFT);
    tree.on_key_down("ArrowLeft", SHIFT);

    // Typing over the range replaces it (replace-on-type).
    tree.on_text_input(input, "X");
    assert_eq!(tree.element_get_text_content(input), "helX");
}

/// A laid-out, focused text-input carrying `content`, ready for pointer/key
/// gestures. Returns (tree, input).
fn text_input_with(content: &str) -> (ElementTree, hayate_core::ElementId) {
    let mut tree = ElementTree::new();
    let input = tree.element_create(20, ElementKind::TextInput);
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
    tree.element_append_text_content(input, content);
    tree.element_focus(input);
    tree.render(0.0);
    (tree, input)
}

#[test]
fn drag_within_text_input_selects_a_range() {
    let (mut tree, input) = text_input_with("hello world");

    // Press near the start of the field and drag rightwards across glyphs.
    tree.on_pointer_down(2.0, 20.0);
    tree.on_pointer_move(60.0, 20.0);

    let (start, end) = tree
        .element_text_selection(input)
        .expect("a non-empty edit selection after dragging");
    assert!(start < end, "drag should select a non-empty range, got {start}..{end}");
}

/// A column holding a focused text-input above a `selectable` paragraph (its own
/// Selection Region). Returns (tree, input, paragraph-text). Both are laid out.
fn input_above_selectable_paragraph() -> (ElementTree, hayate_core::ElementId, hayate_core::ElementId)
{
    use hayate_core::FlexDirectionValue;
    let mut tree = ElementTree::new();
    let root = tree.element_create(30, ElementKind::View);
    let input = tree.element_create(31, ElementKind::TextInput);
    let region = tree.element_create(32, ElementKind::View);
    let text = tree.element_create(33, ElementKind::Text);
    // A spacer keeps the paragraph clear of the input selection's floating
    // toolbar (ADR-0097, #272), which — with the input anchored at the top —
    // flips below the input and would otherwise overlay the paragraph.
    let spacer = tree.element_create(34, ElementKind::View);
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
        input,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::FontSize(16.0),
        ],
    );
    tree.element_set_style(
        spacer,
        &[
            StyleProp::Width(Dimension::px(400.0)),
            StyleProp::Height(Dimension::px(80.0)),
        ],
    );
    tree.element_set_style(region, &[StyleProp::Width(Dimension::px(400.0))]);
    tree.element_set_style(text, &[StyleProp::Width(Dimension::px(400.0))]);
    tree.element_append_child(root, input);
    tree.element_append_child(root, spacer);
    tree.element_append_child(root, region);
    tree.element_append_child(region, text);
    tree.element_append_text_content(input, "edit me");
    tree.element_set_text(text, "Hello world");
    tree.element_set_selectable(region, true);
    tree.element_focus(input);
    tree.render(0.0);
    (tree, input, text)
}

#[test]
fn starting_a_text_input_selection_clears_the_selection_area_selection() {
    let (mut tree, input, text) = input_above_selectable_paragraph();
    let (_, ty, _, th) = tree.element_layout_rect(text).unwrap();

    // First select read-only text in the paragraph region.
    tree.on_pointer_down(2.0, ty + th / 2.0);
    tree.on_pointer_move(70.0, ty + th / 2.0);
    assert!(tree.selection().is_some(), "a SelectionArea selection exists");

    // Now drag inside the text-input: the document selection must clear.
    tree.on_pointer_down(2.0, 20.0);
    tree.on_pointer_move(50.0, 20.0);
    assert!(
        tree.selection().is_none(),
        "starting a text-input selection clears the SelectionArea selection",
    );
    assert!(
        tree.element_text_selection(input).is_some(),
        "the text-input now owns the active selection",
    );
}

#[test]
fn starting_a_selection_area_selection_clears_the_text_input_selection() {
    let (mut tree, input, text) = input_above_selectable_paragraph();

    // First select a range inside the text-input.
    tree.on_pointer_down(2.0, 20.0);
    tree.on_pointer_move(50.0, 20.0);
    tree.on_pointer_up(50.0, 20.0);
    assert!(
        tree.element_text_selection(input).is_some(),
        "the text-input has an active edit selection",
    );

    // Now select read-only text in the paragraph: the edit selection collapses.
    let (_, ty, _, th) = tree.element_layout_rect(text).unwrap();
    tree.on_pointer_down(2.0, ty + th / 2.0);
    tree.on_pointer_move(70.0, ty + th / 2.0);
    assert!(
        tree.element_text_selection(input).is_none(),
        "starting a SelectionArea selection collapses the text-input selection",
    );
    assert!(tree.selection().is_some(), "the SelectionArea now owns the selection");
}

#[test]
fn on_key_down_backspace_edits_focused_text_input() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(1, ElementKind::TextInput);
    tree.set_root(input);
    tree.element_focus(input);
    tree.element_append_text_content(input, "hello");

    tree.on_key_down("Backspace", 0);

    assert_eq!(tree.element_get_text_content(input), "hell");
}

#[test]
fn on_key_down_enter_inserts_newline_via_edit_state() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(2, ElementKind::TextInput);
    tree.set_root(input);
    tree.element_focus(input);
    tree.element_append_text_content(input, "a");

    tree.on_key_down("Enter", 0);

    assert_eq!(tree.element_get_text_content(input), "a\n");
}

#[test]
fn on_composition_end_commits_via_edit_state() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(3, ElementKind::TextInput);
    tree.set_root(input);
    tree.element_append_text_content(input, "abc");
    tree.on_composition_start(input, "DEF");
    tree.on_composition_end(input, "愛");

    assert_eq!(tree.element_get_text_content(input), "abc愛");
}

#[test]
fn composition_format_ranges_reach_core_as_underlines() {
    // EditContext `textformatupdate` → wire → core: the clause format ranges
    // delivered with a preedit update surface as display-text underline ranges
    // (ADR-0102, #336). "abc" committed (3 bytes) shifts the offsets.
    let mut tree = ElementTree::new();
    let input = tree.element_create(7, ElementKind::TextInput);
    tree.set_root(input);
    tree.element_append_text_content(input, "abc");
    tree.on_composition_start(input, "ぎゅうにゅう");

    let clauses = CompositionClause::from_wire(&[0, 9, 1, 9, 18, 0]);
    tree.on_composition_update_formatted(input, "ぎゅうにゅう", clauses);

    assert_eq!(
        tree.element_composition_underlines(input),
        vec![
            (3, 12, CompositionUnderline::Thick),
            (12, 21, CompositionUnderline::Thin),
        ],
    );

    // Finalizing the composition clears the underlines.
    tree.on_composition_end(input, "牛乳");
    assert!(tree.element_composition_underlines(input).is_empty());
}

#[test]
fn on_text_input_appends_via_edit_state() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(4, ElementKind::TextInput);
    tree.set_root(input);

    tree.on_text_input(input, "x");

    assert_eq!(tree.element_get_text_content(input), "x");
}

/// A focused, laid-out text-input with an active preedit. Returns its draw ops.
fn render_with_preedit(
    preedit: &str,
    clauses: Vec<CompositionClause>,
) -> Vec<hayate_core::DrawOp> {
    let mut tree = ElementTree::new();
    let input = tree.element_create(30, ElementKind::TextInput);
    tree.set_root(input);
    tree.set_viewport(300.0, 40.0);
    tree.element_set_style(
        input,
        &[
            StyleProp::Width(Dimension::px(300.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::FontSize(16.0),
        ],
    );
    tree.element_focus(input);
    tree.element_set_preedit_with_clauses(input, preedit, clauses);
    tree.render(0.0);

    let mut painter = hayate_core::RecordingPainter::new();
    hayate_core::render_scene_graph(tree.scene_graph(), &mut painter);
    painter.ops().to_vec()
}

/// Composition underline rects: short (≤3px tall) and wide (≥5px) — distinct from
/// the tall, hairline caret. Returned as (x, width, height) sorted left-to-right.
fn underline_rects(ops: &[hayate_core::DrawOp]) -> Vec<(f32, f32, f32)> {
    let mut rects: Vec<(f32, f32, f32)> = ops
        .iter()
        .filter_map(|op| match op {
            hayate_core::DrawOp::FillRect { x, width, height, .. }
                if *height <= 3.0 && *width >= 5.0 =>
            {
                Some((*x, *width, *height))
            }
            _ => None,
        })
        .collect();
    rects.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    rects
}

#[test]
fn unformatted_preedit_draws_a_single_underline() {
    // Pre-conversion: one thin underline spanning the whole composition.
    let ops = render_with_preedit("ぎゅうにゅう", Vec::new());
    let rects = underline_rects(&ops);
    assert_eq!(rects.len(), 1, "one underline over the whole preedit");
    assert!(rects[0].1 > 10.0, "it spans the composed text");
}

#[test]
fn clause_split_draws_a_thick_and_thin_underline() {
    // During conversion the active clause is thick, the rest thin (ADR-0102).
    let ops = render_with_preedit(
        "ぎゅうにゅう",
        vec![
            CompositionClause { start: 0, end: 9, underline: CompositionUnderline::Thick },
            CompositionClause { start: 9, end: 18, underline: CompositionUnderline::Thin },
        ],
    );
    let rects = underline_rects(&ops);
    assert_eq!(rects.len(), 2, "one underline per clause");
    let (thick, thin) = (rects[0], rects[1]);
    assert!(
        thick.2 > thin.2,
        "active clause underline is thicker than the determined one ({thick:?} vs {thin:?})",
    );
}

#[test]
fn committing_the_composition_removes_the_underline() {
    let ops = render_with_preedit("ぎゅう", Vec::new());
    assert_eq!(underline_rects(&ops).len(), 1);

    // After commit the preedit is empty: no composition underlines remain.
    let ops_after = render_with_preedit("", Vec::new());
    assert!(underline_rects(&ops_after).is_empty());
}

#[test]
fn element_character_bounds_available_after_layout() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(5, ElementKind::TextInput);
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
    tree.element_append_text_content(input, "hi");
    tree.render(0.0);

    let bounds = tree
        .element_character_bounds(input)
        .expect("character bounds after layout");
    assert!(bounds.width > 0.0);
    assert!(bounds.height > 0.0);
}

#[test]
fn element_character_bounds_respects_padding() {
    let mut tree = ElementTree::new();
    let input = tree.element_create(6, ElementKind::TextInput);
    tree.set_root(input);
    tree.set_viewport(200.0, 40.0);
    tree.element_set_style(
        input,
        &[
            StyleProp::Width(Dimension::px(200.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::PaddingLeft(Dimension::px(12.0)),
            StyleProp::PaddingTop(Dimension::px(8.0)),
            StyleProp::FontSize(13.0),
        ],
    );
    tree.element_append_text_content(input, "hi");
    tree.element_focus(input);
    let cursor_rect = {
        let sg = tree.render(0.0);
        sg.iter().find_map(|(_, n)| {
            if let hayate_core::NodeKind::Rect {
                x,
                y,
                width,
                height,
                corner_radius,
                ..
            } = &n.kind
            {
                if *width <= 2.0 && *height > 10.0 && *corner_radius == 0.0 {
                    Some((*x, *y))
                } else {
                    None
                }
            } else {
                None
            }
        })
    };

    let bounds = tree
        .element_character_bounds(input)
        .expect("character bounds with padding");
    if let Some((cursor_x, cursor_y)) = cursor_rect {
        assert!(
            (bounds.x - cursor_x).abs() < 0.5,
            "IME bounds x should match canvas cursor x"
        );
        assert!(
            (bounds.y - cursor_y).abs() < 0.5,
            "IME bounds y should match canvas cursor y"
        );
    }
    assert!(
        bounds.x >= 12.0,
        "IME bounds x should be inset by padding-left, got x={}",
        bounds.x
    );
}
