/// IME composition underline weight for one clause (ADR-0102). Chromium draws
/// the clause being converted with a thick underline and the surrounding,
/// already-determined clauses with a thin one; this mirrors EditContext
/// `textformatupdate`'s underline thickness styles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompositionUnderline {
    /// Pre-conversion text or a non-active clause — a thin underline.
    Thin,
    /// The clause currently being converted (the IME's active segment) — a thick
    /// underline.
    Thick,
}

/// One composition clause: a byte sub-range of the preedit text and its
/// underline weight. Offsets are relative to the preedit string (0 = its first
/// byte), matching how EditContext reports `textformatupdate` ranges once the
/// committed-content prefix is subtracted by the adapter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompositionClause {
    pub start: usize,
    pub end: usize,
    pub underline: CompositionUnderline,
}

impl CompositionClause {
    /// Decode the wire form carried across the EditContext `textformatupdate`
    /// boundary (ADR-0102): a flat `[start, end, weight, …]` triple stream where
    /// `weight == 0` is [`CompositionUnderline::Thin`] and any non-zero value is
    /// [`CompositionUnderline::Thick`]. A trailing partial triple is ignored.
    pub fn from_wire(formats: &[u32]) -> Vec<CompositionClause> {
        formats
            .chunks_exact(3)
            .filter_map(|c| {
                let (start, end) = (c[0] as usize, c[1] as usize);
                if start >= end {
                    return None;
                }
                let underline = if c[2] == 0 {
                    CompositionUnderline::Thin
                } else {
                    CompositionUnderline::Thick
                };
                Some(CompositionClause {
                    start,
                    end,
                    underline,
                })
            })
            .collect()
    }
}

/// In-progress IME composition (ADR-0102): the preedit text plus the clause
/// format ranges fed from EditContext `textformatupdate`. With no clauses, the
/// whole preedit renders as a single thin-underlined run — the pre-conversion
/// look before the IME has split the reading into segments.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Preedit {
    pub text: String,
    pub clauses: Vec<CompositionClause>,
}

/// Direction of an edit motion. `Backward`/`Forward` step horizontally along the
/// text run; `Up`/`Down` move vertically between display lines (ADR-0103). A
/// vertical motion needs Parley line geometry, so the `ElementTree` editing seam
/// resolves it for multi-line fields; a single-line field has no rows, so the
/// pure `EditState` seam treats `Up`/`Down` as jumps to the field start/end
/// (Chromium `<input>`: ↑ = 先頭, ↓ = 末尾).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Backward,
    Forward,
    Up,
    Down,
}

impl Direction {
    /// Whether this is a vertical (line-to-line) motion rather than a horizontal
    /// step. Vertical motions keep the sticky goal column; horizontal ones reset
    /// it.
    fn is_vertical(self) -> bool {
        matches!(self, Direction::Up | Direction::Down)
    }
}

/// Granularity of an edit motion (ADR-0103). The closed vocabulary grows as
/// later slices add line/document boundaries and vertical motion; this tracer
/// covers the horizontal grapheme and word steps reused from `selection.rs`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Granularity {
    Grapheme,
    Word,
    /// The boundary of the current display line — Home/End (or macOS Cmd+←/→).
    /// In single-line (`<input>`) semantics the line *is* the whole field, so
    /// this resolves to the field start/end; multi-line display-line ends are #7.
    LineBoundary,
    /// The boundary of the whole field — Ctrl+Home/End (or macOS Cmd+↑/↓).
    DocBoundary,
}

impl Granularity {
    /// Whether this granularity steps to an absolute field/line boundary rather
    /// than one grapheme/word relative to the caret. Boundary motions (Home/End)
    /// jump to the boundary even over a selection, unlike the arrow keys.
    fn is_boundary(self) -> bool {
        matches!(self, Granularity::LineBoundary | Granularity::DocBoundary)
    }
}

/// The closed edit-command vocabulary applied through the single editing seam
/// [`EditState::apply`] (ADR-0103, ADR-0071). `Move` repositions the caret;
/// `Extend` grows or shrinks the selection by moving the focus while the anchor
/// stays fixed; `Delete` removes one `granularity` step (or the selected range)
/// in `direction`. `SelectAll` selects the whole field. `Copy` / `Cut` / `Paste`
/// are clipboard members of the vocabulary, but the system clipboard is a
/// Platform Adapter responsibility (ADR-0097): `EditState` holds no clipboard, so
/// [`EditState::apply`] consumes only the pure-state members (`Move` / `Extend` /
/// `Delete` / `SelectAll`) and reports the clipboard members unconsumed, leaving
/// their read/write to the `ElementTree` editing seam that owns the `Clipboard`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditIntent {
    Move {
        granularity: Granularity,
        direction: Direction,
    },
    Extend {
        granularity: Granularity,
        direction: Direction,
    },
    Delete {
        granularity: Granularity,
        direction: Direction,
    },
    /// Select the whole field content (Ctrl/Cmd+A).
    SelectAll,
    /// Copy the selection to the system clipboard (Ctrl/Cmd+C). No state change.
    Copy,
    /// Cut the selection: copy it to the clipboard, then delete it (Ctrl/Cmd+X).
    Cut,
    /// Replace the selection with the clipboard text (Ctrl/Cmd+V).
    Paste,
}

/// Text-input edit model (ADR-0069). Owned by TextInput elements only.
///
/// The caret is the degenerate form of the unified Selection model (ADR-0097):
/// `selection_anchor` and `cursor_byte_index` are the anchor/focus byte offsets
/// of a range within `text_content`. When they coincide the selection is
/// collapsed to a single caret, which is the common editing case.
#[derive(Clone, Debug, Default)]
pub struct EditState {
    pub text_content: String,
    pub preedit: Option<Preedit>,
    /// The selection's focus (the moving end / caret position).
    pub cursor_byte_index: usize,
    /// The selection's anchor (the fixed end). Equal to `cursor_byte_index` for
    /// a collapsed caret.
    pub selection_anchor: usize,
    /// Sticky goal column for vertical (↑/↓) motion: the content-local x the
    /// caret aims for as it crosses display lines, so passing through a short
    /// line does not lose the original column (ADR-0103). `None` until a vertical
    /// motion establishes it; any horizontal motion clears it. Set in
    /// content-local pixels by the `ElementTree` seam, which owns Parley geometry.
    pub desired_x: Option<f32>,
}

impl EditState {
    pub fn display_text(&self) -> String {
        match &self.preedit {
            Some(p) => format!("{}{}", self.text_content, p.text),
            None => self.text_content.clone(),
        }
    }

    /// The active composition's underline ranges in **display-text byte offsets**
    /// (i.e. already shifted past the committed `text_content` prefix), each with
    /// its weight (ADR-0102). Empty when no composition is active. With no clause
    /// formats the whole preedit is one thin-underlined range — the look before
    /// the IME splits the reading into segments.
    pub fn composition_underlines(&self) -> Vec<(usize, usize, CompositionUnderline)> {
        let Some(preedit) = &self.preedit else {
            return Vec::new();
        };
        let base = self.text_content.len();
        if preedit.clauses.is_empty() {
            if preedit.text.is_empty() {
                return Vec::new();
            }
            return vec![(base, base + preedit.text.len(), CompositionUnderline::Thin)];
        }
        preedit
            .clauses
            .iter()
            .map(|c| (base + c.start, base + c.end, c.underline))
            .collect()
    }

    /// True when the selection is collapsed to a caret (anchor == focus).
    pub fn is_caret(&self) -> bool {
        self.selection_anchor == self.cursor_byte_index
    }

    /// The selected byte range `(start, end)` normalized to text order, or `None`
    /// when the selection is collapsed (nothing is selected).
    pub fn selection_range(&self) -> Option<(usize, usize)> {
        if self.is_caret() {
            None
        } else {
            let a = self.selection_anchor;
            let f = self.cursor_byte_index;
            Some((a.min(f), a.max(f)))
        }
    }

    /// Place a (possibly empty) selection with `anchor`/`focus` byte offsets,
    /// each clamped into the current text.
    pub fn set_selection(&mut self, anchor: usize, focus: usize) {
        let len = self.text_content.len();
        self.selection_anchor = anchor.min(len);
        self.cursor_byte_index = focus.min(len);
    }

    /// Move the focus (caret) to `offset`, keeping the anchor fixed — the
    /// Shift+Arrow / drag extension primitive.
    pub fn move_focus(&mut self, offset: usize) {
        self.cursor_byte_index = offset.min(self.text_content.len());
    }

    /// Collapse the selection to a caret at the current focus, discarding any
    /// selected range (used to enforce the single-active rule, ADR-0097).
    pub fn collapse(&mut self) {
        self.selection_anchor = self.cursor_byte_index;
    }

    /// Collapse the selection to a caret at `offset`. This is the caret-reposition
    /// choke point for edits (insert / delete / set / commit) and horizontal
    /// moves, so it also drops the sticky vertical goal column — only a vertical
    /// motion (which repositions via `move_focus` / `set_selection`) keeps it.
    fn collapse_to(&mut self, offset: usize) {
        let o = offset.min(self.text_content.len());
        self.cursor_byte_index = o;
        self.selection_anchor = o;
        self.desired_x = None;
    }

    /// Delete the selected range when non-empty, collapsing the caret to its
    /// start (replace-on-type primitive). Returns whether anything was removed.
    fn delete_selection(&mut self) -> bool {
        if let Some((start, end)) = self.selection_range() {
            self.text_content.replace_range(start..end, "");
            self.collapse_to(start);
            true
        } else {
            false
        }
    }

    pub fn set(&mut self, text: &str) {
        self.text_content = text.to_string();
        self.preedit = None;
        self.collapse_to(self.text_content.len());
    }

    pub fn append(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.text_content.push_str(text);
        self.collapse_to(self.text_content.len());
    }

    pub fn insert(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        // Typing over a range replaces it (replace-on-type, ADR-0097).
        self.delete_selection();
        let byte = self.cursor_byte_index.min(self.text_content.len());
        self.text_content.insert_str(byte, text);
        self.collapse_to(byte + text.len());
    }

    pub fn backspace(&mut self) -> bool {
        // A non-empty selection is deleted whole, instead of one trailing char.
        if self.delete_selection() {
            return true;
        }
        if self.text_content.is_empty() {
            return false;
        }
        let last_start = self
            .text_content
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.text_content.truncate(last_start);
        self.collapse_to(self.text_content.len());
        true
    }

    pub fn set_preedit(&mut self, preedit: &str) {
        self.set_preedit_with_clauses(preedit, Vec::new());
    }

    /// Set the preedit text together with its composition clause format ranges
    /// (ADR-0102). Clearing the text (empty `preedit`) drops the composition and
    /// any clauses with it.
    pub fn set_preedit_with_clauses(
        &mut self,
        preedit: &str,
        clauses: Vec<CompositionClause>,
    ) {
        self.preedit = if preedit.is_empty() {
            None
        } else {
            Some(Preedit {
                text: preedit.to_string(),
                clauses,
            })
        };
    }

    pub fn commit_preedit(&mut self) {
        if let Some(preedit) = self.preedit.take() {
            self.text_content.push_str(&preedit.text);
            self.collapse_to(self.text_content.len());
        }
    }

    /// IME composition finalized: commit via the single preedit→content path.
    pub fn finish_composition(&mut self, committed: &str) {
        self.set_preedit(committed);
        self.commit_preedit();
    }

    /// Cut: return the selected text and delete it (collapsing to a caret at the
    /// start of the removed range), or `None` when the selection is collapsed —
    /// the Cut toolbar action (ADR-0097, #272).
    pub fn cut(&mut self) -> Option<String> {
        let (start, end) = self.selection_range()?;
        let removed = self.text_content[start..end].to_string();
        self.delete_selection();
        Some(removed)
    }

    /// Replace the entire committed content with `value`, finalizing any active
    /// preedit first so an in-progress IME composition never lingers across the
    /// replacement (same preedit-confirmation integrity as `paste`). Returns
    /// whether the displayed text actually changed.
    pub fn set_value(&mut self, value: &str) -> bool {
        let changed = self.display_text() != value;
        self.commit_preedit();
        self.set(value);
        changed
    }

    pub fn paste(&mut self, text: &str) -> bool {
        if text.is_empty() {
            return false;
        }
        self.commit_preedit();
        // Pasting over a range replaces it (replace-on-type, ADR-0097).
        self.insert(text);
        true
    }

    /// The byte offset one `granularity` step from `offset` in `direction`,
    /// reusing the shared grapheme/word steppers (`selection.rs`).
    fn step(&self, granularity: Granularity, direction: Direction, offset: usize) -> usize {
        use crate::element::selection::{next_grapheme, next_word, prev_grapheme, prev_word};
        // Single-line vertical semantics (#368): a field with no rows treats ↑ as
        // a jump to the field start and ↓ to the field end (Chromium `<input>`).
        // Multi-line vertical motion needs Parley geometry and is resolved one
        // layer up, on the `ElementTree` editing seam.
        match direction {
            Direction::Up => return 0,
            Direction::Down => return self.text_content.len(),
            Direction::Backward | Direction::Forward => {}
        }
        match (granularity, direction) {
            (Granularity::Grapheme, Direction::Backward) => prev_grapheme(&self.text_content, offset),
            (Granularity::Grapheme, Direction::Forward) => next_grapheme(&self.text_content, offset),
            (Granularity::Word, Direction::Backward) => prev_word(&self.text_content, offset),
            (Granularity::Word, Direction::Forward) => next_word(&self.text_content, offset),
            // Single-line semantics (#360): the line and the document both span
            // the whole field, so either boundary collapses to the field ends.
            // Multi-line display-line boundaries are resolved on the tree seam.
            (Granularity::LineBoundary | Granularity::DocBoundary, Direction::Backward) => 0,
            (Granularity::LineBoundary | Granularity::DocBoundary, Direction::Forward) => {
                self.text_content.len()
            }
            // Vertical directions returned above.
            (_, Direction::Up | Direction::Down) => unreachable!("vertical handled above"),
        }
    }

    /// The single editing seam (ADR-0103): apply one closed-vocabulary
    /// [`EditIntent`] and report whether it was consumed.
    pub fn apply(&mut self, intent: EditIntent) -> bool {
        match intent {
            EditIntent::Move {
                granularity,
                direction,
            } => {
                // Both branches collapse via `collapse_to`, which drops the sticky
                // goal column — a single-line ↑/↓ has no rows to keep it for.
                // Chromium: a plain arrow over a selection collapses to the
                // directional edge without stepping; over a caret it steps one
                // unit and stays collapsed. A boundary motion (Home/End) or a
                // vertical jump ignores the selection and goes straight to the
                // target (single-line ↑ = field start, ↓ = field end).
                match self.selection_range() {
                    Some((start, end)) if !granularity.is_boundary() && !direction.is_vertical() => {
                        let edge = match direction {
                            Direction::Backward => start,
                            Direction::Forward => end,
                            Direction::Up | Direction::Down => unreachable!("vertical excluded"),
                        };
                        self.collapse_to(edge);
                    }
                    _ => {
                        let next = self.step(granularity, direction, self.cursor_byte_index);
                        self.collapse_to(next);
                    }
                }
                true
            }
            EditIntent::Extend {
                granularity,
                direction,
            } => {
                if !direction.is_vertical() {
                    self.desired_x = None;
                }
                let next = self.step(granularity, direction, self.cursor_byte_index);
                self.move_focus(next);
                true
            }
            EditIntent::Delete {
                granularity,
                direction,
            } => {
                // A non-empty selection is removed whole (replace-on-type
                // consistency), collapsing to its start; otherwise one
                // granularity step in `direction` from the caret is removed.
                if self.delete_selection() {
                    return true;
                }
                let from = self.cursor_byte_index;
                let to = self.step(granularity, direction, from);
                if from == to {
                    return false; // at the text boundary — nothing to delete
                }
                let (start, end) = (from.min(to), from.max(to));
                self.text_content.replace_range(start..end, "");
                self.collapse_to(start);
                true
            }
            EditIntent::SelectAll => {
                // Anchor at the start, focus at the end — the whole field becomes
                // the selected range (collapsed at 0 when the field is empty).
                self.set_selection(0, self.text_content.len());
                true
            }
            // Clipboard members cross the Platform Adapter boundary (ADR-0097):
            // EditState owns no clipboard, so it cannot read the selection out
            // (Copy), capture-then-delete it (Cut), or pull text in (Paste). The
            // `ElementTree` seam that holds the `Clipboard` resolves these; here
            // they are reported unconsumed so this layer never half-applies them.
            EditIntent::Copy | EditIntent::Cut | EditIntent::Paste => false,
        }
    }

    pub fn apply_key_down(&mut self, key: &str) -> bool {
        // Char editing (Backspace/Delete) and caret motion are interpreted as
        // EditIntents upstream (ADR-0103); only Enter remains a raw key here.
        match key {
            "Enter" => {
                // Insert at the caret (replacing any selection), not append — a
                // newline behaves like any other typed character (#362). Whether
                // Enter reaches here at all is gated upstream by the element's
                // `multiline` property; a single-line field treats it as submit.
                self.insert("\n");
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_inserts_a_newline_at_the_caret_not_at_the_end() {
        // The Enter key inserts `\n` at the caret position, like any other typed
        // character — not appended to the end (#362, fixing the old append bug).
        let mut edit = EditState::default();
        edit.set("ab"); // caret collapsed at end (2)
        edit.set_selection(1, 1); // caret between 'a' and 'b'
        assert!(edit.apply_key_down("Enter"));
        assert_eq!(edit.text_content, "a\nb", "newline lands at the caret");
        assert_eq!(edit.cursor_byte_index, 2, "caret sits after the inserted newline");
        assert!(edit.is_caret());
    }

    #[test]
    fn enter_replaces_the_selected_range() {
        // replace-on-type: pressing Enter over a selection drops the range and
        // inserts the newline in its place (#362).
        let mut edit = EditState::default();
        edit.set("hello");
        edit.set_selection(1, 4); // "ell" selected
        assert!(edit.apply_key_down("Enter"));
        assert_eq!(edit.text_content, "h\no");
        assert_eq!(edit.cursor_byte_index, 2, "caret after the newline");
        assert!(edit.is_caret());
    }

    #[test]
    fn backspace_removes_last_scalar() {
        let mut edit = EditState::default();
        edit.append("hello");
        assert!(edit.backspace());
        assert_eq!(edit.text_content, "hell");
        assert_eq!(edit.cursor_byte_index, 4);
    }

    fn move_grapheme(d: Direction) -> EditIntent {
        EditIntent::Move {
            granularity: Granularity::Grapheme,
            direction: d,
        }
    }

    #[test]
    fn select_all_spans_the_whole_content() {
        // SelectAll (Ctrl/Cmd+A) is a pure-state member of the editing seam: it
        // anchors at the field start and moves the focus to the end, so the whole
        // content becomes the selected range regardless of the prior caret.
        let mut edit = EditState::default();
        edit.set("héllo"); // caret collapsed at end
        edit.set_selection(2, 2);

        assert!(edit.apply(EditIntent::SelectAll));

        assert_eq!(
            edit.selection_range(),
            Some((0, "héllo".len())),
            "the entire content is selected",
        );
    }

    #[test]
    fn select_all_on_empty_content_stays_collapsed() {
        // Nothing to select: the range collapses at 0 (no spurious selection).
        let mut edit = EditState::default();
        assert!(edit.apply(EditIntent::SelectAll));
        assert!(edit.is_caret());
        assert_eq!(edit.cursor_byte_index, 0);
    }

    #[test]
    fn move_to_line_boundary_jumps_to_field_start_and_end() {
        // Single-line semantics (#360): line end = field end. Home (Backward)
        // collapses the caret to 0, End (Forward) to the content length.
        let mut edit = EditState::default();
        edit.set("hello"); // caret at end (5)
        edit.set_selection(2, 2); // caret in the middle

        assert!(edit.apply(EditIntent::Move {
            granularity: Granularity::LineBoundary,
            direction: Direction::Backward,
        }));
        assert_eq!(edit.cursor_byte_index, 0, "Home lands at the field start");
        assert!(edit.is_caret());

        assert!(edit.apply(EditIntent::Move {
            granularity: Granularity::LineBoundary,
            direction: Direction::Forward,
        }));
        assert_eq!(edit.cursor_byte_index, 5, "End lands at the field end");
        assert!(edit.is_caret());
    }

    fn extend_grapheme(d: Direction) -> EditIntent {
        EditIntent::Extend {
            granularity: Granularity::Grapheme,
            direction: d,
        }
    }

    fn delete_grapheme(d: Direction) -> EditIntent {
        EditIntent::Delete {
            granularity: Granularity::Grapheme,
            direction: d,
        }
    }

    #[test]
    fn delete_over_a_selection_removes_the_range_and_collapses_to_its_start() {
        // Both Backspace (Backward) and Delete (Forward) drop the whole selected
        // range, never just one adjacent char, and collapse to the range start.
        for direction in [Direction::Backward, Direction::Forward] {
            let mut edit = EditState::default();
            edit.set("hello");
            edit.set_selection(1, 4); // "ell" selected
            assert!(edit.apply(delete_grapheme(direction)));
            assert_eq!(edit.text_content, "ho", "{direction:?}: the range is gone");
            assert_eq!(edit.cursor_byte_index, 1, "{direction:?}: collapses to range start");
            assert!(edit.is_caret(), "{direction:?}: collapsed");
        }
    }

    #[test]
    fn delete_forward_grapheme_removes_the_char_after_the_caret() {
        let mut edit = EditState::default();
        edit.set("aあb"); // caret at end (5)
        edit.set_selection(0, 0); // caret at start
        assert!(edit.apply(delete_grapheme(Direction::Forward)));
        assert_eq!(edit.text_content, "あb", "removes the leading 'a'");
        assert_eq!(edit.cursor_byte_index, 0, "caret stays at the deletion point");
        assert!(edit.apply(delete_grapheme(Direction::Forward)));
        assert_eq!(edit.text_content, "b", "removes the 3-byte 'あ' whole");
        assert_eq!(edit.cursor_byte_index, 0);
        assert!(edit.is_caret());
    }

    #[test]
    fn delete_at_the_text_boundary_is_a_no_op() {
        let mut edit = EditState::default();
        edit.set("hi"); // caret at end (2)
        assert!(!edit.apply(delete_grapheme(Direction::Forward)), "nothing past the end");
        assert_eq!(edit.text_content, "hi");
        edit.set_selection(0, 0); // caret at start
        assert!(!edit.apply(delete_grapheme(Direction::Backward)), "nothing before the start");
        assert_eq!(edit.text_content, "hi");
    }

    #[test]
    fn delete_backward_grapheme_removes_the_char_before_the_caret() {
        let mut edit = EditState::default();
        edit.set("aあb"); // caret at end (5)
        assert!(edit.apply(delete_grapheme(Direction::Backward)));
        assert_eq!(edit.text_content, "aあ", "removes the trailing 'b'");
        assert_eq!(edit.cursor_byte_index, 4, "caret lands where 'b' began");
        assert!(edit.apply(delete_grapheme(Direction::Backward)));
        assert_eq!(edit.text_content, "a", "removes the 3-byte 'あ' whole");
        assert_eq!(edit.cursor_byte_index, 1);
        assert!(edit.is_caret());
    }

    #[test]
    fn move_forward_grapheme_steps_the_caret_one_char() {
        let mut edit = EditState::default();
        edit.set("aあb"); // caret at end (5)
        edit.set_selection(0, 0); // caret at start
        assert!(edit.apply(move_grapheme(Direction::Forward)));
        assert_eq!(edit.cursor_byte_index, 1, "advances past 'a'");
        assert!(edit.apply(move_grapheme(Direction::Forward)));
        assert_eq!(edit.cursor_byte_index, 4, "advances past the 3-byte 'あ'");
        assert!(edit.is_caret(), "a Move stays collapsed");
    }

    #[test]
    fn single_line_vertical_moves_to_field_start_and_end() {
        // Chromium `<input>` (#368): with no rows, ↑ jumps to the field start and
        // ↓ to the field end. EditState owns this pure single-line semantics; the
        // geometry-driven multi-line case is resolved on the ElementTree seam.
        let mut edit = EditState::default();
        edit.set("hello");
        edit.set_selection(2, 2); // caret in the middle

        assert!(edit.apply(EditIntent::Move {
            granularity: Granularity::Grapheme,
            direction: Direction::Up,
        }));
        assert_eq!(edit.cursor_byte_index, 0, "↑ → field start");
        assert!(edit.is_caret());

        assert!(edit.apply(EditIntent::Move {
            granularity: Granularity::Grapheme,
            direction: Direction::Down,
        }));
        assert_eq!(edit.cursor_byte_index, 5, "↓ → field end");
        assert!(edit.is_caret());
    }

    #[test]
    fn single_line_vertical_jumps_over_a_selection_to_the_field_end() {
        // Unlike a horizontal arrow (which collapses to the selection edge), ↑/↓
        // ignore the selection and jump straight to the field boundary.
        let mut edit = EditState::default();
        edit.set("hello");
        edit.set_selection(1, 4); // "ell" selected, focus at 4

        assert!(edit.apply(EditIntent::Move {
            granularity: Granularity::Grapheme,
            direction: Direction::Down,
        }));
        assert_eq!(edit.cursor_byte_index, 5, "↓ jumps past the selection to the end");
        assert!(edit.is_caret());
    }

    #[test]
    fn shift_vertical_extends_to_the_field_ends_in_a_single_line() {
        // Shift+↑/↓ in a single-line field extends the selection to the field
        // start/end, anchor fixed (the Extend counterpart of the Move jumps).
        let mut edit = EditState::default();
        edit.set("hello");
        edit.set_selection(2, 2);

        assert!(edit.apply(EditIntent::Extend {
            granularity: Granularity::Grapheme,
            direction: Direction::Up,
        }));
        assert_eq!(edit.selection_anchor, 2, "anchor stays put");
        assert_eq!(edit.cursor_byte_index, 0, "Shift+↑ extends to the field start");
        assert_eq!(edit.selection_range(), Some((0, 2)));
    }

    #[test]
    fn a_horizontal_move_clears_the_sticky_goal_column() {
        // The goal column is kept across vertical motion but reset the moment the
        // caret moves horizontally (ADR-0103) — otherwise a later ↑/↓ would snap
        // back to a stale column.
        let mut edit = EditState::default();
        edit.set("hello");
        edit.desired_x = Some(42.0);
        assert!(edit.apply(move_grapheme(Direction::Backward)));
        assert_eq!(edit.desired_x, None, "a horizontal step resets the goal column");
    }

    #[test]
    fn editing_clears_the_sticky_goal_column() {
        // Typing repositions the caret, so the next ↑/↓ must aim from the new
        // column, not a stale one — inserting clears the goal column.
        let mut edit = EditState::default();
        edit.set("hello");
        edit.set_selection(2, 2);
        edit.desired_x = Some(42.0);
        edit.insert("X");
        assert_eq!(edit.desired_x, None, "an edit resets the goal column");
    }

    #[test]
    fn move_to_doc_boundary_jumps_to_field_start_and_end() {
        // Ctrl+Home/End. In single-line semantics the document boundary equals
        // the line boundary, so both collapse to the field ends (#360).
        let mut edit = EditState::default();
        edit.set("hello world");
        edit.set_selection(4, 4);

        assert!(edit.apply(EditIntent::Move {
            granularity: Granularity::DocBoundary,
            direction: Direction::Backward,
        }));
        assert_eq!(edit.cursor_byte_index, 0, "Ctrl+Home lands at the start");

        assert!(edit.apply(EditIntent::Move {
            granularity: Granularity::DocBoundary,
            direction: Direction::Forward,
        }));
        assert_eq!(edit.cursor_byte_index, 11, "Ctrl+End lands at the end");
        assert!(edit.is_caret());
    }

    #[test]
    fn move_to_boundary_over_a_selection_jumps_to_the_boundary_not_the_edge() {
        // Unlike a plain arrow (which collapses to the selection edge), Home/End
        // over a selection jumps to the field boundary and collapses there.
        let mut edit = EditState::default();
        edit.set("hello");
        edit.set_selection(1, 4); // "ell" selected, focus at 4

        assert!(edit.apply(EditIntent::Move {
            granularity: Granularity::LineBoundary,
            direction: Direction::Forward,
        }));
        assert_eq!(
            edit.cursor_byte_index, 5,
            "End jumps past the selection's right edge (4) to the field end (5)",
        );
        assert!(edit.is_caret());
    }

    #[test]
    fn move_backward_grapheme_steps_the_caret_left() {
        let mut edit = EditState::default();
        edit.set("aあb"); // caret at end (5)
        assert!(edit.apply(move_grapheme(Direction::Backward)));
        assert_eq!(edit.cursor_byte_index, 4, "retreats past 'b'");
        assert!(edit.is_caret());
    }

    #[test]
    fn move_forward_over_a_selection_collapses_to_its_right_edge() {
        let mut edit = EditState::default();
        edit.set("hello");
        edit.set_selection(1, 4); // "ell" selected, focus at 4
        assert!(edit.apply(move_grapheme(Direction::Forward)));
        assert_eq!(
            edit.cursor_byte_index, 4,
            "collapses to the right edge, does not step to 5",
        );
        assert!(edit.is_caret());
    }

    #[test]
    fn move_backward_over_a_selection_collapses_to_its_left_edge() {
        let mut edit = EditState::default();
        edit.set("hello");
        edit.set_selection(4, 1); // "ell" selected, focus at 1 (drag leftwards)
        assert!(edit.apply(move_grapheme(Direction::Backward)));
        assert_eq!(
            edit.cursor_byte_index, 1,
            "collapses to the left edge regardless of which end the focus was",
        );
        assert!(edit.is_caret());
    }

    #[test]
    fn extend_grapheme_moves_the_focus_keeping_the_anchor() {
        let mut edit = EditState::default();
        edit.set("hello"); // caret at end (5)
        assert!(edit.apply(extend_grapheme(Direction::Backward)));
        assert!(edit.apply(extend_grapheme(Direction::Backward)));
        assert_eq!(edit.selection_anchor, 5, "anchor stays fixed at the start point");
        assert_eq!(edit.cursor_byte_index, 3, "focus retreats two chars");
        assert_eq!(edit.selection_range(), Some((3, 5)), "selects 'lo'");
        // Extending back forward contracts the range toward the anchor.
        assert!(edit.apply(extend_grapheme(Direction::Forward)));
        assert_eq!(edit.cursor_byte_index, 4, "focus advances, shrinking the range");
    }

    #[test]
    fn move_and_extend_by_word_use_the_shared_word_steppers() {
        let mut edit = EditState::default();
        edit.set("hello world"); // caret at end (11)
        edit.set_selection(0, 0); // caret at start
        assert!(edit.apply(EditIntent::Move {
            granularity: Granularity::Word,
            direction: Direction::Forward,
        }));
        assert_eq!(edit.cursor_byte_index, 5, "word move lands at end of 'hello'");
        assert!(edit.apply(EditIntent::Extend {
            granularity: Granularity::Word,
            direction: Direction::Forward,
        }));
        assert_eq!(edit.cursor_byte_index, 11, "word extend reaches end of 'world'");
        assert_eq!(edit.selection_range(), Some((5, 11)));
    }

    #[test]
    fn delete_by_word_removes_a_whole_word_in_each_direction() {
        // #363: Delete with Word granularity removes from the caret to the word
        // boundary (`prev_word` / `next_word`), the model behind Ctrl/Alt+
        // Backspace/Delete.
        let mut edit = EditState::default();
        edit.set("hello world"); // caret at end (11)
        assert!(edit.apply(EditIntent::Delete {
            granularity: Granularity::Word,
            direction: Direction::Backward,
        }));
        assert_eq!(edit.text_content, "hello ", "the word before the caret goes");
        assert_eq!(edit.cursor_byte_index, 6, "caret collapses to the word start");

        edit.set_selection(0, 0); // caret to the field start
        assert!(edit.apply(EditIntent::Delete {
            granularity: Granularity::Word,
            direction: Direction::Forward,
        }));
        assert_eq!(edit.text_content, " ", "the word after the caret goes");
        assert_eq!(edit.cursor_byte_index, 0, "caret stays at the deletion point");
    }

    #[test]
    fn extend_to_boundary_selects_to_the_field_end_keeping_the_anchor() {
        // Shift+End from a mid-field caret selects from the caret to the field
        // end, anchor fixed; Shift+Home then selects back to the start.
        let mut edit = EditState::default();
        edit.set("hello world");
        edit.set_selection(6, 6); // caret before "world"

        assert!(edit.apply(EditIntent::Extend {
            granularity: Granularity::LineBoundary,
            direction: Direction::Forward,
        }));
        assert_eq!(edit.selection_anchor, 6, "anchor stays put");
        assert_eq!(edit.cursor_byte_index, 11, "focus reaches the field end");
        assert_eq!(edit.selection_range(), Some((6, 11)), "selects 'world'");

        assert!(edit.apply(EditIntent::Extend {
            granularity: Granularity::DocBoundary,
            direction: Direction::Backward,
        }));
        assert_eq!(edit.selection_anchor, 6, "anchor still fixed");
        assert_eq!(edit.cursor_byte_index, 0, "focus crosses to the field start");
        assert_eq!(edit.selection_range(), Some((0, 6)), "now selects 'hello '");
    }

    #[test]
    fn paste_commits_preedit_first() {
        let mut edit = EditState::default();
        edit.append("ab");
        edit.set_preedit("CD");
        assert!(edit.paste("xy"));
        assert_eq!(edit.text_content, "abCDxy");
        assert!(edit.preedit.is_none());
    }

    #[test]
    fn typing_replaces_the_selected_range() {
        let mut edit = EditState::default();
        edit.set("hello"); // collapsed caret at end
        edit.set_selection(1, 4); // select "ell"
        assert!(!edit.is_caret());
        edit.insert("X");
        assert_eq!(edit.text_content, "hXo");
        assert_eq!(edit.cursor_byte_index, 2, "caret sits after the inserted text");
        assert!(edit.is_caret(), "the range collapses once it is replaced");
    }

    #[test]
    fn backspace_deletes_the_selected_range_not_one_char() {
        let mut edit = EditState::default();
        edit.set("hello");
        edit.set_selection(1, 4); // "ell"
        assert!(edit.backspace());
        assert_eq!(edit.text_content, "ho");
        assert_eq!(edit.cursor_byte_index, 1, "caret collapses to the range start");
        assert!(edit.is_caret());
    }

    #[test]
    fn paste_replaces_the_selected_range() {
        let mut edit = EditState::default();
        edit.set("hello");
        edit.set_selection(0, 5); // whole word
        assert!(edit.paste("bye"));
        assert_eq!(edit.text_content, "bye");
        assert_eq!(edit.cursor_byte_index, 3);
        assert!(edit.is_caret());
    }

    #[test]
    fn finish_composition_uses_commit_preedit() {
        let mut edit = EditState::default();
        edit.append("abc");
        edit.set_preedit("DEF");
        edit.finish_composition("愛");
        assert_eq!(edit.text_content, "abc愛");
        assert!(edit.preedit.is_none());
    }

    #[test]
    fn set_value_replaces_content_and_finalizes_active_preedit() {
        let mut edit = EditState::default();
        edit.append("abc");
        edit.set_preedit("DEF"); // in-progress IME composition
        assert!(edit.set_value("xyz"), "replacing the value is a change");
        assert_eq!(edit.text_content, "xyz", "value is fully replaced");
        assert!(edit.preedit.is_none(), "composition must not linger");
        assert_eq!(edit.display_text(), "xyz");
        assert_eq!(
            edit.cursor_byte_index, 3,
            "caret sits at the end of the value"
        );
        assert!(edit.is_caret());
    }

    #[test]
    fn preedit_retains_clause_format_ranges() {
        let mut edit = EditState::default();
        edit.append("ab");
        edit.set_preedit_with_clauses(
            "ぎゅう",
            vec![CompositionClause {
                start: 0,
                end: 9,
                underline: CompositionUnderline::Thick,
            }],
        );
        // The clause weight is preserved and the display text still concatenates.
        let preedit = edit.preedit.as_ref().expect("composition active");
        assert_eq!(preedit.text, "ぎゅう");
        assert_eq!(preedit.clauses[0].underline, CompositionUnderline::Thick);
        assert_eq!(edit.display_text(), "abぎゅう");
    }

    #[test]
    fn unformatted_preedit_underlines_the_whole_run_thin() {
        // Pre-conversion: no clause split yet ⇒ one thin underline over the
        // preedit, shifted past the committed prefix ("ab" = 2 bytes).
        let mut edit = EditState::default();
        edit.append("ab");
        edit.set_preedit("xyz");
        assert_eq!(
            edit.composition_underlines(),
            vec![(2, 5, CompositionUnderline::Thin)],
        );
    }

    #[test]
    fn clause_split_underlines_each_segment_in_display_offsets() {
        // During conversion the IME splits the reading into clauses; the active
        // one is thick. Offsets returned are display-text relative (past "ab").
        let mut edit = EditState::default();
        edit.append("ab");
        edit.set_preedit_with_clauses(
            "ぎゅうにゅう",
            vec![
                CompositionClause { start: 0, end: 9, underline: CompositionUnderline::Thick },
                CompositionClause { start: 9, end: 18, underline: CompositionUnderline::Thin },
            ],
        );
        assert_eq!(
            edit.composition_underlines(),
            vec![
                (2, 11, CompositionUnderline::Thick),
                (11, 20, CompositionUnderline::Thin),
            ],
        );
    }

    #[test]
    fn no_composition_has_no_underlines() {
        let mut edit = EditState::default();
        edit.set("hello");
        assert!(edit.composition_underlines().is_empty());
    }

    #[test]
    fn from_wire_decodes_format_triples() {
        // [start, end, weight] triples; weight 0 = thin, non-zero = thick.
        let clauses = CompositionClause::from_wire(&[0, 9, 1, 9, 18, 0]);
        assert_eq!(
            clauses,
            vec![
                CompositionClause { start: 0, end: 9, underline: CompositionUnderline::Thick },
                CompositionClause { start: 9, end: 18, underline: CompositionUnderline::Thin },
            ],
        );
        // Degenerate (empty/inverted) ranges and a trailing partial triple drop.
        assert!(CompositionClause::from_wire(&[5, 5, 0, 7]).is_empty());
    }

    #[test]
    fn commit_clears_composition_decoration() {
        let mut edit = EditState::default();
        edit.append("ab");
        edit.set_preedit_with_clauses(
            "ぎゅう",
            vec![CompositionClause { start: 0, end: 9, underline: CompositionUnderline::Thick }],
        );
        edit.commit_preedit();
        assert_eq!(edit.text_content, "abぎゅう");
        assert!(edit.preedit.is_none());
        assert!(
            edit.composition_underlines().is_empty(),
            "committing the composition clears its underlines",
        );
    }

    #[test]
    fn set_value_to_identical_committed_content_is_not_a_change() {
        let mut edit = EditState::default();
        edit.set("abc");
        assert!(
            !edit.set_value("abc"),
            "no-op replacement reports no change"
        );
    }
}
