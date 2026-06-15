/// Text-input edit model (ADR-0069). Owned by TextInput elements only.
///
/// The caret is the degenerate form of the unified Selection model (ADR-0097):
/// `selection_anchor` and `cursor_byte_index` are the anchor/focus byte offsets
/// of a range within `text_content`. When they coincide the selection is
/// collapsed to a single caret, which is the common editing case.
#[derive(Clone, Debug, Default)]
pub struct EditState {
    pub text_content: String,
    pub preedit: Option<String>,
    /// The selection's focus (the moving end / caret position).
    pub cursor_byte_index: usize,
    /// The selection's anchor (the fixed end). Equal to `cursor_byte_index` for
    /// a collapsed caret.
    pub selection_anchor: usize,
}

impl EditState {
    pub fn display_text(&self) -> String {
        match &self.preedit {
            Some(p) => format!("{}{}", self.text_content, p),
            None => self.text_content.clone(),
        }
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
        self.preedit = if preedit.is_empty() {
            None
        } else {
            Some(preedit.to_string())
        };
    }

    pub fn commit_preedit(&mut self) {
        if let Some(preedit) = self.preedit.take() {
            self.text_content.push_str(&preedit);
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
}
