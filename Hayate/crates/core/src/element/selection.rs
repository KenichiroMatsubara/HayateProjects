//! Unified text-selection model (ADR-0097).
//!
//! A single `Selection` is owned by the Element Document Runtime
//! (`ElementTree`); at most one is active across the whole document. Both
//! endpoints are `(ElementId, byte offset)` pairs. A degenerate selection with
//! `anchor == focus` is a caret — the growth point that subsumes
//! `EditState::cursor_byte_index` (ADR-0069).

use crate::element::id::ElementId;

/// Modifier-key bit flags carried by `modifiers: u32` in pointer/key intake,
/// matching the wire `MODIFIER_*` contract (proto/spec): SHIFT=1, CTRL=2,
/// ALT=4, META=8.
pub const MOD_SHIFT: u32 = 1;
pub const MOD_CTRL: u32 = 2;
pub const MOD_ALT: u32 = 4;
pub const MOD_META: u32 = 8;

/// The primary command modifier — Ctrl on Windows/Linux, Cmd (Meta) on macOS —
/// for chords like select-all (#267).
pub const MOD_PRIMARY: u32 = MOD_CTRL | MOD_META;

/// One end of a selection: a byte offset within a specific element's text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectionPoint {
    pub element: ElementId,
    pub offset: usize,
}

impl SelectionPoint {
    pub fn new(element: ElementId, offset: usize) -> Self {
        Self { element, offset }
    }
}

/// A continuous text selection between `anchor` (where the drag started) and
/// `focus` (where the pointer currently is). Document order is not implied by
/// field order; callers normalize via [`Selection::range_within`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub anchor: SelectionPoint,
    pub focus: SelectionPoint,
}

impl Selection {
    /// A degenerate (collapsed) selection at a single point — a caret.
    pub fn caret(point: SelectionPoint) -> Self {
        Self {
            anchor: point,
            focus: point,
        }
    }

    /// True when the selection is collapsed (anchor == focus): a caret.
    pub fn is_caret(&self) -> bool {
        self.anchor == self.focus
    }

    /// The selected byte range within `element`, normalized to document order
    /// (`start <= end`). Returns `None` unless both endpoints lie in `element`
    /// (the single-IFC tracer case; cross-element selection is a growth point).
    pub fn range_within(&self, element: ElementId) -> Option<(usize, usize)> {
        if self.anchor.element != element || self.focus.element != element {
            return None;
        }
        let a = self.anchor.offset;
        let b = self.focus.offset;
        Some((a.min(b), a.max(b)))
    }
}

/// Character class for word segmentation. Double-click (word) selection groups a
/// maximal run of characters sharing a class, mirroring how a desktop text view
/// expands a word: letters/digits, whitespace, and everything else are distinct.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CharClass {
    Word,
    Space,
    Other,
}

fn classify(c: char) -> CharClass {
    if c.is_whitespace() {
        CharClass::Space
    } else if c.is_alphanumeric() || c == '_' {
        CharClass::Word
    } else {
        CharClass::Other
    }
}

/// Snap `offset` down to the nearest `char` boundary at or before it (clamped to
/// the text length), so callers can pass raw byte offsets safely.
fn floor_boundary(text: &str, offset: usize) -> usize {
    let mut o = offset.min(text.len());
    while o > 0 && !text.is_char_boundary(o) {
        o -= 1;
    }
    o
}

/// Byte range of the "word" containing `offset`: the maximal run of characters
/// sharing the class (word / whitespace / other) of the character at `offset`.
/// At end-of-text the preceding character defines the class (ADR-0097, #267).
pub fn word_bounds(text: &str, offset: usize) -> (usize, usize) {
    let len = text.len();
    if len == 0 {
        return (0, 0);
    }
    let offset = floor_boundary(text, offset);
    // The char whose class defines the word: the one starting at `offset`, or the
    // last char when `offset` is at end-of-text.
    let pivot_start = if offset < len {
        offset
    } else {
        text.char_indices().next_back().map(|(i, _)| i).unwrap()
    };
    let class = classify(text[pivot_start..].chars().next().unwrap());

    let mut end = pivot_start;
    for (i, c) in text[pivot_start..].char_indices() {
        if classify(c) == class {
            end = pivot_start + i + c.len_utf8();
        } else {
            break;
        }
    }
    let mut start = pivot_start;
    for (i, c) in text[..pivot_start].char_indices().rev() {
        if classify(c) == class {
            start = i;
        } else {
            break;
        }
    }
    (start, end)
}

/// Byte range of the paragraph (hard line) containing `offset`, bounded by `\n`
/// line breaks which are themselves excluded. Triple-click selection (#267).
pub fn line_bounds(text: &str, offset: usize) -> (usize, usize) {
    let offset = floor_boundary(text, offset);
    let start = text[..offset].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let end = text[offset..]
        .find('\n')
        .map(|i| offset + i)
        .unwrap_or(text.len());
    (start, end)
}

/// Byte offset of the next `char` after `offset` (clamped at end-of-text).
/// Shift+Arrow moves the focus one character at a time (#267).
pub fn next_grapheme(text: &str, offset: usize) -> usize {
    let offset = floor_boundary(text, offset);
    text[offset..]
        .chars()
        .next()
        .map(|c| offset + c.len_utf8())
        .unwrap_or(offset)
}

/// Byte offset of the previous `char` before `offset` (clamped at 0).
pub fn prev_grapheme(text: &str, offset: usize) -> usize {
    let offset = floor_boundary(text, offset);
    text[..offset]
        .chars()
        .next_back()
        .map(|c| offset - c.len_utf8())
        .unwrap_or(0)
}

/// Resolve a horizontal caret step for a Left/Right arrow key: one grapheme, or
/// one word when `by_word` (Alt on macOS / Ctrl on Win/Linux). `None` for keys
/// that are not arrows, so callers can fall through to other handling. Shared by
/// the read-only SelectionArea and text-input edit selection (ADR-0097).
pub fn arrow_step(text: &str, key: &str, offset: usize, by_word: bool) -> Option<usize> {
    Some(match (key, by_word) {
        ("ArrowRight", false) => next_grapheme(text, offset),
        ("ArrowLeft", false) => prev_grapheme(text, offset),
        ("ArrowRight", true) => next_word(text, offset),
        ("ArrowLeft", true) => prev_word(text, offset),
        _ => return None,
    })
}

/// Byte offset at the end of the next word after `offset`: skip any non-word run,
/// then consume the following word run. Word-granularity Shift+Arrow (#267).
pub fn next_word(text: &str, offset: usize) -> usize {
    let len = text.len();
    let mut o = floor_boundary(text, offset);
    while o < len {
        let c = text[o..].chars().next().unwrap();
        if classify(c) == CharClass::Word {
            break;
        }
        o += c.len_utf8();
    }
    while o < len {
        let c = text[o..].chars().next().unwrap();
        if classify(c) != CharClass::Word {
            break;
        }
        o += c.len_utf8();
    }
    o
}

/// Byte offset at the start of the previous word before `offset`: skip any
/// non-word run leftwards, then consume the preceding word run.
pub fn prev_word(text: &str, offset: usize) -> usize {
    let mut o = floor_boundary(text, offset);
    while o > 0 {
        let c = text[..o].chars().next_back().unwrap();
        if classify(c) == CharClass::Word {
            break;
        }
        o -= c.len_utf8();
    }
    while o > 0 {
        let c = text[..o].chars().next_back().unwrap();
        if classify(c) != CharClass::Word {
            break;
        }
        o -= c.len_utf8();
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(id: u64, offset: usize) -> SelectionPoint {
        SelectionPoint::new(ElementId::from_u64(id), offset)
    }

    #[test]
    fn word_bounds_spans_the_word_under_an_interior_offset() {
        // "Hello world" — offset 2 is inside "Hello" → 0..5.
        assert_eq!(word_bounds("Hello world", 2), (0, 5));
        // Inside "world" → 6..11.
        assert_eq!(word_bounds("Hello world", 8), (6, 11));
    }

    #[test]
    fn word_bounds_at_a_boundary_takes_the_following_word() {
        // Offset 6 sits at the start of "world".
        assert_eq!(word_bounds("Hello world", 6), (6, 11));
        // Offset on the space groups the whitespace run.
        assert_eq!(word_bounds("Hello world", 5), (5, 6));
    }

    #[test]
    fn word_bounds_at_end_of_text_uses_the_preceding_word() {
        let text = "Hello world";
        assert_eq!(word_bounds(text, text.len()), (6, 11));
    }

    #[test]
    fn word_bounds_handles_multibyte_words() {
        // "あ い" — first word is the 3-byte "あ".
        let text = "あ い";
        assert_eq!(word_bounds(text, 0), (0, 3));
    }

    #[test]
    fn line_bounds_spans_the_paragraph_between_newlines() {
        let text = "one\ntwo\nthree";
        // Within "two": bytes 4..7 (newlines excluded).
        assert_eq!(line_bounds(text, 5), (4, 7));
        // First paragraph.
        assert_eq!(line_bounds(text, 1), (0, 3));
        // Last paragraph runs to end-of-text.
        assert_eq!(line_bounds(text, 9), (8, 13));
    }

    #[test]
    fn grapheme_stepping_moves_one_char_at_a_time() {
        let text = "aあb"; // 1 + 3 + 1 bytes
        assert_eq!(next_grapheme(text, 0), 1);
        assert_eq!(next_grapheme(text, 1), 4);
        assert_eq!(next_grapheme(text, text.len()), text.len());
        assert_eq!(prev_grapheme(text, 4), 1);
        assert_eq!(prev_grapheme(text, 0), 0);
    }

    #[test]
    fn word_stepping_jumps_across_word_runs() {
        let text = "Hello world";
        // From the start, the next word boundary is the end of "Hello".
        assert_eq!(next_word(text, 0), 5);
        // From inside "Hello", still lands at its end.
        assert_eq!(next_word(text, 2), 5);
        // From the space, skips to the end of "world".
        assert_eq!(next_word(text, 5), 11);
        // Backwards from the end lands at the start of "world".
        assert_eq!(prev_word(text, 11), 6);
        // Backwards from inside "world" lands at its start.
        assert_eq!(prev_word(text, 8), 6);
    }

    #[test]
    fn caret_is_collapsed_at_a_single_point() {
        let sel = Selection::caret(point(1, 3));
        assert!(sel.is_caret());
        assert_eq!(sel.anchor, sel.focus);
        assert_eq!(sel.range_within(ElementId::from_u64(1)), Some((3, 3)));
    }

    #[test]
    fn range_within_normalizes_to_document_order() {
        // focus before anchor (drag leftwards) still yields start <= end.
        let sel = Selection {
            anchor: point(1, 7),
            focus: point(1, 2),
        };
        assert!(!sel.is_caret());
        assert_eq!(sel.range_within(ElementId::from_u64(1)), Some((2, 7)));
    }

    #[test]
    fn range_within_is_none_for_a_different_element() {
        let sel = Selection {
            anchor: point(1, 0),
            focus: point(1, 4),
        };
        assert_eq!(sel.range_within(ElementId::from_u64(2)), None);
    }
}
