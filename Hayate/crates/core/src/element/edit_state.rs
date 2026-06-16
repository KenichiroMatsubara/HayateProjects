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

    /// Collapse the selection to a caret at `offset`.
    fn collapse_to(&mut self, offset: usize) {
        let o = offset.min(self.text_content.len());
        self.cursor_byte_index = o;
        self.selection_anchor = o;
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

    pub fn apply_key_down(&mut self, key: &str) -> bool {
        match key {
            "Backspace" => self.backspace(),
            "Enter" => {
                self.append("\n");
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
    fn backspace_removes_last_scalar() {
        let mut edit = EditState::default();
        edit.append("hello");
        assert!(edit.backspace());
        assert_eq!(edit.text_content, "hell");
        assert_eq!(edit.cursor_byte_index, 4);
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
