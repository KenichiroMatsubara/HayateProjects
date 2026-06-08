/// Text-input edit model (ADR-0069). Owned by TextInput elements only.
#[derive(Clone, Debug, Default)]
pub struct EditState {
    pub text_content: String,
    pub preedit: Option<String>,
    pub cursor_byte_index: usize,
}

impl EditState {
    pub fn display_text(&self) -> String {
        match &self.preedit {
            Some(p) => format!("{}{}", self.text_content, p),
            None => self.text_content.clone(),
        }
    }

    pub fn set(&mut self, text: &str) {
        self.text_content = text.to_string();
        self.preedit = None;
        self.cursor_byte_index = self.text_content.len();
    }

    pub fn append(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.text_content.push_str(text);
        self.cursor_byte_index = self.text_content.len();
    }

    pub fn insert(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let byte = self.cursor_byte_index.min(self.text_content.len());
        self.text_content.insert_str(byte, text);
        self.cursor_byte_index = byte + text.len();
    }

    pub fn backspace(&mut self) -> bool {
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
        self.cursor_byte_index = self.text_content.len();
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
            self.cursor_byte_index = self.text_content.len();
        }
    }

    /// IME composition finalized: commit via the single preedit→content path.
    pub fn finish_composition(&mut self, committed: &str) {
        self.set_preedit(committed);
        self.commit_preedit();
    }

    pub fn paste(&mut self, text: &str) -> bool {
        if text.is_empty() {
            return false;
        }
        self.commit_preedit();
        self.append(text);
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
    fn finish_composition_uses_commit_preedit() {
        let mut edit = EditState::default();
        edit.append("abc");
        edit.set_preedit("DEF");
        edit.finish_composition("愛");
        assert_eq!(edit.text_content, "abc愛");
        assert!(edit.preedit.is_none());
    }
}
