//! ADR-0069: EditState via ElementTree public input handlers.

use hayate_core::{
    Clipboard, CompositionClause, CompositionUnderline, Dimension, Direction, EditIntent,
    ElementKind, ElementTree, Granularity, PointerKind, StyleProp,
};
use std::cell::RefCell;
use std::rc::Rc;

const SHIFT: u32 = 1; // MODIFIER_SHIFT (proto/spec wire contract).
const ALT: u32 = 4; // MODIFIER_ALT — the macOS Option "by word" modifier.

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
fn delete_keys_do_not_disturb_an_active_ime_composition() {
    // ADR-0103: a Backspace/Delete while an IME preedit is active must not edit
    // the committed content or break the composition — the intent is refused at
    // the seam while composing, so the preedit and content stay intact.
    let mut tree = ElementTree::new();
    let input = tree.element_create(102, ElementKind::TextInput);
    tree.set_root(input);
    tree.element_focus(input);
    tree.element_append_text_content(input, "ab"); // caret at 2
    tree.on_composition_start(input, "きゅ"); // active preedit

    tree.on_key_down("Backspace", 0);
    tree.on_key_down("Delete", 0);

    assert_eq!(
        tree.element_get_text_content(input),
        "abきゅ",
        "neither key altered the committed text or the composition",
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

#[test]
fn double_click_in_text_input_selects_the_word_under_the_pointer() {
    // #366: a desktop double-click expands the edit selection to the whole word,
    // mirroring the read-only SelectionArea multi-click (begin_selection_at).
    let (mut tree, input) = text_input_with("hello world");

    // Two presses at the same spot inside "hello" expand to the word, 0..5.
    tree.on_pointer_down(15.0, 20.0);
    tree.on_pointer_up(15.0, 20.0);
    tree.on_pointer_down(15.0, 20.0);

    assert_eq!(
        tree.element_text_selection(input),
        Some((0, 5)),
        "double-click selects 'hello'",
    );
}

#[test]
fn triple_click_in_text_input_selects_the_line() {
    // #366: a third press at the same spot expands from word to the whole line
    // (paragraph). With no newline the line is the entire single-line content.
    let (mut tree, input) = text_input_with("hello world");

    tree.on_pointer_down(15.0, 20.0);
    tree.on_pointer_up(15.0, 20.0);
    tree.on_pointer_down(15.0, 20.0);
    tree.on_pointer_up(15.0, 20.0);
    tree.on_pointer_down(15.0, 20.0);

    assert_eq!(
        tree.element_text_selection(input),
        Some((0, 11)),
        "triple-click selects the whole line",
    );
}

#[test]
fn single_click_in_text_input_places_a_caret_not_a_word() {
    // #366 regression guard: a lone press still drops a collapsed caret; the
    // multi-click word/line expansion must not fire on the first press.
    let (mut tree, input) = text_input_with("hello world");

    tree.on_pointer_down(15.0, 20.0);

    assert!(
        tree.element_text_selection(input).is_none(),
        "single click leaves the selection collapsed (a caret)",
    );
    assert!(
        tree.element_caret_byte_index(input).is_some(),
        "the caret is placed in the field",
    );
}

#[test]
fn a_relayout_after_a_click_does_not_move_the_caret_or_forge_a_selection() {
    // Canvas-mode root cause: the layout pass rebuilds a text-input's shaped
    // content on every relayout (style change, resize, a selection-driven
    // repaint) and used to force `cursor_byte_index` to the text end while
    // leaving `selection_anchor` where a click had just placed it. The next
    // frame then read a phantom `(click..end)` selection with the caret snapped
    // to the end — "a plain click selects from the click point to the last
    // character", and Shift+click collapsed to nothing. A relayout must preserve
    // the caret the click placed (only clamping it if the text shrank).
    let (mut tree, input) = text_input_with("hello world"); // caret at end

    tree.on_pointer_down(15.0, 20.0); // caret lands mid-word
    let caret = tree.element_caret_byte_index(input);
    assert!(tree.element_text_selection(input).is_none(), "click is a caret");

    // Force the input to re-lay-out, as a steady-state rAF frame does.
    tree.element_set_style(input, &[StyleProp::FontSize(16.0)]);
    tree.render(16.0);

    assert!(
        tree.element_text_selection(input).is_none(),
        "a relayout after a click must not manufacture a selection",
    );
    assert_eq!(
        tree.element_caret_byte_index(input),
        caret,
        "a relayout must not snap the caret back to the text end",
    );
}

#[test]
fn click_lands_the_caret_at_the_clicked_point_not_a_glyph_left_edge() {
    // A click resolves to a byte offset via Parley `Cursor::from_point`, which
    // honours which half of the glyph was hit. The earlier `byte_index_at_point`
    // returned the hit cluster's *start* unconditionally, so a press on the
    // trailing half snapped the caret back to the glyph's leading edge and a
    // press past the last glyph never reached the text end — the caret could not
    // follow the click. Guard the two extremes: a far-left press sits before the
    // first glyph (0) and a far-right press reaches the end (len).
    let (mut tree, input) = text_input_with("hello");

    tree.on_pointer_down(2.0, 20.0);
    assert_eq!(
        tree.element_caret_byte_index(input),
        Some(0),
        "a press at the left edge sits before the first glyph",
    );

    tree.on_pointer_up(2.0, 20.0);
    tree.on_pointer_down(190.0, 20.0);
    assert_eq!(
        tree.element_caret_byte_index(input),
        Some(5),
        "a press past the last glyph reaches the text end, not its left edge",
    );
}

#[test]
fn double_click_under_touch_modality_stays_a_caret() {
    // #366: word/line expansion is a Mouse/Pen gesture (ADR-0104). Under Touch
    // the double press stays a caret, so it never competes with the long-press
    // word selection. Mouse double-click (other tests) still expands.
    let (mut tree, input) = text_input_with("hello world");

    tree.on_pointer_down_with_kind(15.0, 20.0, 0, PointerKind::Touch);
    tree.on_pointer_up_with_kind(15.0, 20.0, PointerKind::Touch);
    tree.on_pointer_down_with_kind(15.0, 20.0, 0, PointerKind::Touch);

    assert!(
        tree.element_text_selection(input).is_none(),
        "a Touch double press does not expand to a word",
    );
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
fn delete_key_removes_the_grapheme_after_the_caret() {
    // ADR-0103: Delete (forward) was previously a complete no-op; now it routes
    // through the EditIntent seam and removes the char to the caret's right.
    let (mut tree, input) = focused_input("hello"); // caret at end (5)
    tree.on_key_down("ArrowLeft", 0);
    tree.on_key_down("ArrowLeft", 0); // caret at 3 (before "lo")

    tree.on_key_down("Delete", 0);
    assert_eq!(tree.element_get_text_content(input), "helo", "removes the 'l' to the right");
    assert_eq!(tree.element_caret_byte_index(input), Some(3), "caret stays at the deletion point");
    assert!(tree.element_text_selection(input).is_none());
}

#[test]
fn backspace_key_removes_the_grapheme_before_the_caret() {
    let (mut tree, input) = focused_input("hello"); // caret at end (5)
    tree.on_key_down("ArrowLeft", 0); // caret at 4 (before "o")

    tree.on_key_down("Backspace", 0);
    assert_eq!(tree.element_get_text_content(input), "helo", "removes the 'l' to the left");
    assert_eq!(tree.element_caret_byte_index(input), Some(3), "caret retreats to where 'l' began");
}

#[test]
fn backspace_and_delete_over_a_selection_remove_the_whole_range() {
    let (mut tree, input) = focused_input("hello"); // caret at end (5)
    tree.on_key_down("ArrowLeft", SHIFT);
    tree.on_key_down("ArrowLeft", SHIFT); // selects "lo" → (3,5)
    assert_eq!(tree.element_text_selection(input), Some((3, 5)));

    tree.on_key_down("Delete", 0);
    assert_eq!(tree.element_get_text_content(input), "hel", "the range goes, not one char");
    assert_eq!(tree.element_caret_byte_index(input), Some(3));
    assert!(tree.element_text_selection(input).is_none());
}

#[test]
fn ctrl_backspace_deletes_the_word_before_the_caret() {
    // #363: Ctrl+Backspace (Win/Linux) removes the whole word to the caret's
    // left, not a single grapheme — reusing `selection.rs`'s `prev_word`.
    let (mut tree, input) = focused_input("hello world"); // caret at end (11)

    tree.on_key_down("Backspace", CTRL);
    assert_eq!(tree.element_get_text_content(input), "hello ", "the trailing word goes");
    assert_eq!(tree.element_caret_byte_index(input), Some(6), "caret lands at the word start");
    assert!(tree.element_text_selection(input).is_none());
}

#[test]
fn ctrl_delete_deletes_the_word_after_the_caret() {
    // #363: Ctrl+Delete removes the whole word to the caret's right via
    // `next_word`, leaving the caret in place.
    let (mut tree, input) = focused_input("hello world"); // caret at end (11)
    tree.on_key_down("ArrowLeft", CTRL); // word back to 6
    tree.on_key_down("ArrowLeft", CTRL); // word back to the field start (0)
    assert_eq!(tree.element_caret_byte_index(input), Some(0));

    tree.on_key_down("Delete", CTRL);
    assert_eq!(tree.element_get_text_content(input), " world", "the leading word goes");
    assert_eq!(tree.element_caret_byte_index(input), Some(0), "caret stays at the deletion point");
    assert!(tree.element_text_selection(input).is_none());
}

#[test]
fn alt_backspace_and_delete_delete_by_word_on_macos() {
    // #363: macOS uses Option (Alt) as the "by word" modifier; it removes whole
    // words exactly as Ctrl does on Win/Linux.
    let (mut tree, input) = focused_input("alpha beta"); // caret at end (10)

    tree.on_key_down("Backspace", ALT);
    assert_eq!(tree.element_get_text_content(input), "alpha ", "Option+Backspace drops 'beta'");

    tree.on_key_down("ArrowLeft", ALT); // word back to the field start (0)
    tree.on_key_down("Delete", ALT);
    assert_eq!(tree.element_get_text_content(input), " ", "Option+Delete drops 'alpha'");
}

#[test]
fn ctrl_backspace_word_boundary_matches_the_shared_word_logic_in_mixed_text() {
    // #363 acceptance: word deletion honours `selection.rs`'s word boundaries,
    // where the boundary between a CJK run and an English run is the separating
    // space (CJK and ASCII letters both classify as word chars). "こんにちは world"
    // is two words; one Ctrl+Backspace removes only the trailing English word.
    let (mut tree, input) = focused_input("こんにちは world"); // 15 + 1 + 5 = 21 bytes

    tree.on_key_down("Backspace", CTRL);
    assert_eq!(
        tree.element_get_text_content(input),
        "こんにちは ",
        "only the English word goes, the CJK word is left intact",
    );

    // A second Ctrl+Backspace crosses the space and removes the CJK word.
    tree.on_key_down("Backspace", CTRL);
    assert_eq!(tree.element_get_text_content(input), "", "the CJK word goes next");
}

#[test]
fn ctrl_arrow_moves_and_extends_the_caret_by_word() {
    // #363 acceptance: word-granularity caret movement and selection extension
    // reach a focused field through the same `on_key_down` seam (ElementTree
    // integration, complementing the EditState unit coverage).
    let (mut tree, input) = focused_input("hello world"); // caret at end (11)

    tree.on_key_down("ArrowLeft", CTRL); // back over "world"
    assert_eq!(tree.element_caret_byte_index(input), Some(6), "lands at the start of 'world'");

    tree.on_key_down("ArrowLeft", CTRL | SHIFT); // extend back over "hello "
    assert_eq!(tree.element_text_selection(input), Some((0, 6)), "selection grows by a word");
}

#[test]
fn enter_in_a_multiline_field_inserts_a_newline_at_the_caret() {
    // #362: a multi-line field treats Enter as a newline inserted at the caret,
    // not appended to the end (fixing the old append bug).
    let mut tree = ElementTree::new();
    let input = tree.element_create(2, ElementKind::TextInput);
    tree.set_root(input);
    tree.element_focus(input);
    tree.element_set_multiline(input, true);
    tree.element_append_text_content(input, "ab");
    tree.on_key_down("ArrowLeft", 0); // caret between 'a' and 'b'

    tree.on_key_down("Enter", 0);

    assert_eq!(tree.element_get_text_content(input), "a\nb", "newline at the caret");
    assert_eq!(tree.element_caret_byte_index(input), Some(2), "caret after the newline");
}

#[test]
fn enter_in_a_single_line_field_does_not_insert_a_newline_and_signals_submit() {
    // #362: the default (single-line) field leaves the text untouched on Enter;
    // the KeyDown event is the app's submit signal and no TextInput is emitted.
    let mut tree = ElementTree::new();
    let input = tree.element_create(2, ElementKind::TextInput);
    tree.set_root(input);
    tree.element_focus(input);
    tree.element_append_text_content(input, "ab");

    let key_listener =
        tree.register_listener(input, hayate_core::DocumentEventKind::KeyDown);
    let text_listener =
        tree.register_listener(input, hayate_core::DocumentEventKind::TextInput);

    tree.on_key_down("Enter", 0);

    assert_eq!(tree.element_get_text_content(input), "ab", "no newline inserted");
    let deliveries = tree.poll_deliveries();
    assert!(
        deliveries.iter().any(|d| d.listener_id == key_listener
            && matches!(&d.event, hayate_core::Event::KeyDown { key, .. } if key == "Enter")),
        "Enter still reaches the app as a KeyDown (the submit signal)",
    );
    assert!(
        !deliveries.iter().any(|d| d.listener_id == text_listener),
        "a single-line field never emits a TextInput on Enter",
    );
}

#[test]
fn enter_in_a_multiline_field_replaces_the_selection() {
    // replace-on-type: Enter over a selection drops the range and inserts the
    // newline in its place (#362).
    let mut tree = ElementTree::new();
    let input = tree.element_create(2, ElementKind::TextInput);
    tree.set_root(input);
    tree.element_focus(input);
    tree.element_set_multiline(input, true);
    tree.element_append_text_content(input, "hello"); // caret at end (5)
    tree.on_key_down("ArrowLeft", SHIFT);
    tree.on_key_down("ArrowLeft", SHIFT); // selects "lo" → (3,5)
    assert_eq!(tree.element_text_selection(input), Some((3, 5)));

    tree.on_key_down("Enter", 0);

    assert_eq!(tree.element_get_text_content(input), "hel\n", "the range is replaced by the newline");
    assert_eq!(tree.element_caret_byte_index(input), Some(4));
    assert!(tree.element_text_selection(input).is_none());
}

// ── Multi-line vertical motion + display-line Home/End (#368) ─────────────────
// ↑/↓ move between display lines keeping a sticky goal column; Home/End snap to
// the display-line ends. These need Parley line geometry, so the field is laid
// out first. Width is generous so hard-newline cases do not also soft-wrap.

/// A laid-out, focused multi-line text-input carrying `content`. `width` lets a
/// caller force soft-wrapping; `content` may contain `\n` for hard lines.
fn multiline_input(content: &str, width: f32) -> (ElementTree, hayate_core::ElementId) {
    let mut tree = ElementTree::new();
    let input = tree.element_create(40, ElementKind::TextInput);
    tree.set_root(input);
    tree.set_viewport(width.max(200.0), 200.0);
    tree.element_set_multiline(input, true);
    tree.element_set_style(
        input,
        &[
            StyleProp::Width(Dimension::px(width)),
            StyleProp::Height(Dimension::px(200.0)),
            StyleProp::FontSize(16.0),
        ],
    );
    tree.element_append_text_content(input, content);
    tree.element_focus(input);
    tree.render(0.0);
    (tree, input)
}

fn move_vertical(d: Direction) -> EditIntent {
    EditIntent::Move {
        granularity: Granularity::Grapheme,
        direction: d,
    }
}

#[test]
fn arrow_up_down_moves_between_display_lines_at_the_same_column() {
    // #368: two identical hard lines. From the end of line 2, ↑ lands at the same
    // column on line 1 (its end); ↓ returns to the end of line 2.
    let (mut tree, input) = multiline_input("abcdef\nabcdef", 400.0); // caret at 13
    assert_eq!(tree.element_caret_byte_index(input), Some(13));

    assert!(tree.apply_edit_intent(input, move_vertical(Direction::Up)));
    assert_eq!(
        tree.element_caret_byte_index(input),
        Some(6),
        "↑ lands at the end of the line above (same column)",
    );

    assert!(tree.apply_edit_intent(input, move_vertical(Direction::Down)));
    assert_eq!(
        tree.element_caret_byte_index(input),
        Some(13),
        "↓ returns to the end of the line below",
    );
}

#[test]
fn vertical_motion_keeps_the_goal_column_across_a_short_line() {
    // #368 sticky goal column: caret at the end of a long line, then ↑ through a
    // short line ("hi") and ↑ again to another long line. The column is preserved
    // across the short line, so the final caret is at the long line's end (5),
    // not where the short line clamped it (which would land near column 2).
    let (mut tree, input) = multiline_input("world\nhi\nworld", 400.0); // caret at 14
    assert_eq!(tree.element_caret_byte_index(input), Some(14));

    assert!(tree.apply_edit_intent(input, move_vertical(Direction::Up)));
    assert_eq!(
        tree.element_caret_byte_index(input),
        Some(8),
        "↑ onto the short line clamps to its end",
    );

    assert!(tree.apply_edit_intent(input, move_vertical(Direction::Up)));
    assert_eq!(
        tree.element_caret_byte_index(input),
        Some(5),
        "↑ again returns to the original column on the long line (goal kept)",
    );
}

#[test]
fn single_line_arrow_up_down_jumps_to_the_field_ends() {
    // #368: a single-line field has no rows, so ↑ jumps to the field start and ↓
    // to the field end (Chromium `<input>`), resolved by the pure EditState seam.
    let (mut tree, input) = focused_input("hello"); // caret at end (5)

    assert!(tree.apply_edit_intent(input, move_vertical(Direction::Up)));
    assert_eq!(tree.element_caret_byte_index(input), Some(0), "↑ → field start");

    assert!(tree.apply_edit_intent(input, move_vertical(Direction::Down)));
    assert_eq!(tree.element_caret_byte_index(input), Some(5), "↓ → field end");
}

#[test]
fn multiline_home_end_snap_to_the_display_line_ends() {
    // #368: in a multi-line field Home/End move to the *display line* ends, not
    // the whole-field ends. Caret on the third line → Home lands at that line's
    // start (9), not the field start (0); End at its end (14).
    let (mut tree, input) = multiline_input("world\nhi\nworld", 400.0); // caret at 14

    assert!(tree.apply_edit_intent(
        input,
        EditIntent::Move {
            granularity: Granularity::LineBoundary,
            direction: Direction::Backward,
        },
    ));
    assert_eq!(
        tree.element_caret_byte_index(input),
        Some(9),
        "Home → start of the current display line, not the field start",
    );

    assert!(tree.apply_edit_intent(
        input,
        EditIntent::Move {
            granularity: Granularity::LineBoundary,
            direction: Direction::Forward,
        },
    ));
    assert_eq!(tree.element_caret_byte_index(input), Some(14), "End → display-line end");
}

#[test]
fn shift_arrow_down_extends_the_selection_across_lines() {
    // #368: Shift+↑/↓ extends the selection by a row, keeping the anchor.
    let (mut tree, input) = multiline_input("abcdef\nabcdef", 400.0); // caret at 13

    // Caret up to the end of line 1 (anchor point for the extension).
    assert!(tree.apply_edit_intent(input, move_vertical(Direction::Up)));
    assert_eq!(tree.element_caret_byte_index(input), Some(6));

    assert!(tree.apply_edit_intent(
        input,
        EditIntent::Extend {
            granularity: Granularity::Grapheme,
            direction: Direction::Down,
        },
    ));
    assert_eq!(
        tree.element_text_selection(input),
        Some((6, 13)),
        "Shift+↓ selects from line 1's end down to line 2's end (row-spanning)",
    );
}

#[test]
fn on_key_down_arrow_up_moves_the_caret_up_a_line() {
    // #368: the raw key path maps a bare ↑ to vertical motion, so a multi-line
    // field moves the caret to the line above without going through the adapter.
    let (mut tree, input) = multiline_input("abcdef\nabcdef", 400.0); // caret at 13

    tree.on_key_down("ArrowUp", 0);

    assert_eq!(tree.element_caret_byte_index(input), Some(6), "↑ key moved up a line");
}

#[test]
fn vertical_motion_follows_soft_wrapped_lines() {
    // #368 acceptance: with no hard breaks, a narrow field soft-wraps into several
    // display lines. ↑ from the end moves the caret onto a higher visual row — its
    // y drops by roughly a line height — proving wrap geometry drives the motion.
    let (mut tree, input) = multiline_input("aaaa bbbb cccc dddd eeee", 70.0);

    let before = tree
        .element_character_bounds(input)
        .expect("caret bounds before moving");

    assert!(tree.apply_edit_intent(input, move_vertical(Direction::Up)));

    let after = tree
        .element_character_bounds(input)
        .expect("caret bounds after moving");
    assert!(
        after.y < before.y - 1.0,
        "↑ moved the caret to a higher wrapped line (y {} → {})",
        before.y,
        after.y,
    );
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

// ── Clipboard key path (ADR-0103 §5③, #361) ──────────────────────────────────
// Ctrl/Cmd+A/C/X/V reach a focused text-input through the same EditIntent seam
// as the arrows. The primary modifier is Ctrl on Win/Linux (Cmd maps to it in
// the adapter keymap); the core test drives the Ctrl bit directly.
const CTRL: u32 = 2; // MODIFIER_CTRL (proto/spec wire contract).

/// A `Clipboard` double recording writes and serving a preset read value, so a
/// test can assert what crossed the Platform Adapter boundary (mirrors the
/// harness in `selection_toolbar.rs`).
#[derive(Default, Clone)]
struct FakeClipboard {
    writes: Rc<RefCell<Vec<String>>>,
    read: Rc<RefCell<Option<String>>>,
}

impl Clipboard for FakeClipboard {
    fn write_text(&self, text: &str) {
        self.writes.borrow_mut().push(text.to_string());
    }
    fn read_text(&self) -> Option<String> {
        self.read.borrow().clone()
    }
}

#[test]
fn ctrl_a_selects_all_in_the_focused_text_input() {
    // ADR-0103: Ctrl/Cmd+A used to be swallowed by the document-selection path
    // (which only fires when a read-only Selection exists); a focused text-input
    // now receives it as a SelectAll EditIntent and selects its whole content.
    let (mut tree, input) = focused_input("hello"); // caret collapsed at end (5)

    tree.on_key_down("a", CTRL);

    assert_eq!(
        tree.element_text_selection(input),
        Some((0, 5)),
        "Ctrl+A selects the entire field content",
    );
}

#[test]
fn ctrl_c_copies_the_text_input_selection_to_the_clipboard() {
    // Ctrl/Cmd+C on a focused text-input writes its selected text through the
    // Platform Adapter clipboard, leaving the selection in place (Chromium).
    let (mut tree, input) = focused_input("hello"); // caret at end
    let clipboard = FakeClipboard::default();
    tree.set_clipboard(Box::new(clipboard.clone()));
    tree.on_key_down("a", CTRL); // select "hello"

    tree.on_key_down("c", CTRL);

    assert_eq!(clipboard.writes.borrow().as_slice(), &["hello".to_string()]);
    assert_eq!(
        tree.element_text_selection(input),
        Some((0, 5)),
        "Copy leaves the selection in place",
    );
}

#[test]
fn ctrl_x_cuts_the_text_input_selection() {
    // Ctrl/Cmd+X writes the selection to the clipboard and removes it from the
    // field, collapsing the caret to the cut point (ADR-0097, ADR-0103 §5③).
    let (mut tree, input) = focused_input("hello world"); // caret at end
    let clipboard = FakeClipboard::default();
    tree.set_clipboard(Box::new(clipboard.clone()));
    // Select the trailing "world": Shift+Left five times from the end.
    for _ in 0..5 {
        tree.on_key_down("ArrowLeft", SHIFT);
    }
    assert_eq!(tree.element_text_selection(input), Some((6, 11)));

    tree.on_key_down("x", CTRL);

    assert_eq!(clipboard.writes.borrow().as_slice(), &["world".to_string()]);
    assert_eq!(
        tree.element_get_text_content(input),
        "hello ",
        "Cut removes exactly the selected range",
    );
    assert!(
        tree.element_text_selection(input).is_none(),
        "Cut collapses the caret",
    );
}

#[test]
fn ctrl_v_pastes_clipboard_text_replacing_the_selection() {
    // Ctrl/Cmd+V pulls text through the (synchronous) clipboard read and inserts
    // it, replacing any selected range (replace-on-type, ADR-0097).
    let (mut tree, input) = focused_input("hello world"); // caret at end
    let clipboard = FakeClipboard::default();
    *clipboard.read.borrow_mut() = Some("X".to_string());
    tree.set_clipboard(Box::new(clipboard.clone()));
    // Select the trailing "world".
    for _ in 0..5 {
        tree.on_key_down("ArrowLeft", SHIFT);
    }

    tree.on_key_down("v", CTRL);

    assert_eq!(
        tree.element_get_text_content(input),
        "hello X",
        "paste replaces the selected range with the clipboard text",
    );
}

#[test]
fn ctrl_v_pastes_at_a_collapsed_caret_in_an_empty_field() {
    // The keyboard paste targets the focused field directly, so it works even
    // with no selection (a collapsed caret in an empty field) — the toolbar's
    // selection-gated paste could not reach this case.
    let (mut tree, input) = focused_input(""); // empty, caret at 0
    let clipboard = FakeClipboard::default();
    *clipboard.read.borrow_mut() = Some("pasted".to_string());
    tree.set_clipboard(Box::new(clipboard.clone()));

    tree.on_key_down("v", CTRL);

    assert_eq!(tree.element_get_text_content(input), "pasted");
}
