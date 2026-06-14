//! Android GameTextInput → `hayate-core` IME translation (ADR-0087 stage C / ADR-0094).
//!
//! GameActivity's GameTextInput reports text input as *absolute state* — the
//! full buffer plus an optional composing (preedit) region — rather than the
//! discrete composition deltas `hayate-core` exposes (`on_composition_*`). This
//! module diffs successive states into the minimal core edit calls, mirroring
//! core's "committed `text_content` + trailing `preedit`" model (`EditState`,
//! ADR-0069). Like `touch_input`, it is kept free of `android-activity`/`ndk`
//! types so the translation and its application to the tree are host-testable
//! without the NDK; `app.rs` is the thin glue that reads the platform state.
//!
//! Scope (first stage C slice): committed text + preedit are kept faithful for
//! the common case where the composing region is a suffix at the caret.
//! Deferred: fine-grained `CompositionStart/Update/End` *events* for app
//! notification parity with the web adapter, and selection-aware editing.

#[cfg(any(target_os = "android", test))]
use hayate_core::{ElementId, ElementTree};

/// A byte-offset span into the reported text. Mirrors android-activity's
/// `TextSpan`; offsets are UTF-8 byte indices into [`TextInputState::text`].
#[cfg(any(target_os = "android", test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextSpan {
    pub start: usize,
    pub end: usize,
}

/// Absolute soft-keyboard text state. Mirrors android-activity's
/// `TextInputState` (selection omitted — core tracks only a trailing caret).
#[cfg(any(target_os = "android", test))]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextInputState {
    pub text: String,
    /// The composing/preedit region, or `None` when no composition is active.
    pub compose_region: Option<TextSpan>,
}

/// A core edit call to apply to the focused TextInput.
#[cfg(any(target_os = "android", test))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImeAction {
    /// Replace committed content (text outside the composing region). Maps to
    /// `ElementTree::element_set_text_content`, which also clears the preedit.
    SetText(String),
    /// Replace the active preedit (the composing region); empty clears it. Maps
    /// to `ElementTree::element_set_preedit`.
    SetPreedit(String),
}

/// Split an absolute state into (committed text, preedit). The committed text is
/// everything outside the composing region; the preedit is the composing
/// substring. An absent or malformed (out-of-bounds / non-char-boundary)
/// compose region is treated as "no composition".
#[cfg(any(target_os = "android", test))]
fn decompose(state: &TextInputState) -> (String, String) {
    if let Some(span) = state.compose_region {
        let len = state.text.len();
        if span.start <= span.end
            && span.end <= len
            && state.text.is_char_boundary(span.start)
            && state.text.is_char_boundary(span.end)
        {
            let preedit = state.text[span.start..span.end].to_string();
            let mut committed = String::with_capacity(len - (span.end - span.start));
            committed.push_str(&state.text[..span.start]);
            committed.push_str(&state.text[span.end..]);
            return (committed, preedit);
        }
    }
    (state.text.clone(), String::new())
}

/// Diff two GameTextInput states into the minimal core edit calls.
#[cfg(any(target_os = "android", test))]
pub fn translate_text_input(prev: &TextInputState, next: &TextInputState) -> Vec<ImeAction> {
    let (prev_committed, prev_preedit) = decompose(prev);
    let (committed, preedit) = decompose(next);

    let mut actions = Vec::new();
    let set_text = committed != prev_committed;
    if set_text {
        actions.push(ImeAction::SetText(committed));
    }
    // `SetText` clears any preedit, so re-assert the preedit whenever we set the
    // committed text and a composition is still active, or when the preedit
    // itself changed.
    if preedit != prev_preedit || (set_text && !preedit.is_empty()) {
        actions.push(ImeAction::SetPreedit(preedit));
    }
    actions
}

/// Apply one translated action to the focused TextInput. No-op for non-TextInput
/// targets (core's setters guard on the element's edit state).
#[cfg(any(target_os = "android", test))]
pub fn apply_ime_action(tree: &mut ElementTree, target: ElementId, action: &ImeAction) {
    match action {
        ImeAction::SetText(text) => tree.element_set_text_content(target, text),
        ImeAction::SetPreedit(preedit) => tree.element_set_preedit(target, preedit),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hayate_core::ElementKind;

    fn composing(text: &str, start: usize, end: usize) -> TextInputState {
        TextInputState {
            text: text.to_string(),
            compose_region: Some(TextSpan { start, end }),
        }
    }

    fn committed(text: &str) -> TextInputState {
        TextInputState {
            text: text.to_string(),
            compose_region: None,
        }
    }

    #[test]
    fn typing_a_committed_character_sets_text() {
        let actions = translate_text_input(&TextInputState::default(), &committed("a"));
        assert_eq!(actions, vec![ImeAction::SetText("a".into())]);
    }

    #[test]
    fn starting_composition_only_sets_preedit() {
        // "か" is 3 UTF-8 bytes; committed text is unchanged (empty).
        let actions = translate_text_input(&TextInputState::default(), &composing("か", 0, 3));
        assert_eq!(actions, vec![ImeAction::SetPreedit("か".into())]);
    }

    #[test]
    fn updating_composition_replaces_preedit() {
        let actions = translate_text_input(&composing("か", 0, 3), &composing("かん", 0, 6));
        assert_eq!(actions, vec![ImeAction::SetPreedit("かん".into())]);
    }

    #[test]
    fn committing_composition_sets_text_and_clears_preedit() {
        let actions = translate_text_input(&composing("かん", 0, 6), &committed("感"));
        assert_eq!(
            actions,
            vec![ImeAction::SetText("感".into()), ImeAction::SetPreedit(String::new())]
        );
    }

    #[test]
    fn suffix_composition_preserves_committed_prefix() {
        // "abc" already committed, now composing "か" at the caret (suffix).
        let actions = translate_text_input(&committed("abc"), &composing("abcか", 3, 6));
        assert_eq!(actions, vec![ImeAction::SetPreedit("か".into())]);
    }

    #[test]
    fn editing_committed_prefix_while_composing_reasserts_preedit() {
        // Committed prefix changes "a"->"b" while "か" stays in composition;
        // SetText would clear the preedit, so it must be re-asserted.
        let actions = translate_text_input(&composing("aか", 1, 4), &composing("bか", 1, 4));
        assert_eq!(
            actions,
            vec![ImeAction::SetText("b".into()), ImeAction::SetPreedit("か".into())]
        );
    }

    #[test]
    fn no_change_emits_nothing() {
        assert!(translate_text_input(&composing("か", 0, 3), &composing("か", 0, 3)).is_empty());
    }

    #[test]
    fn malformed_compose_region_falls_back_to_committed() {
        // Out-of-bounds end is treated as "no composition".
        let actions = translate_text_input(&TextInputState::default(), &composing("x", 0, 99));
        assert_eq!(actions, vec![ImeAction::SetText("x".into())]);
    }

    // End-to-end against core: a full Japanese composition + commit must leave
    // the TextInput's display_text equal to GameTextInput's reported buffer.
    #[test]
    fn translation_drives_core_display_text_through_a_composition() {
        let mut tree = ElementTree::new();
        let input = tree.element_create(1, ElementKind::TextInput);
        tree.set_root(input);
        tree.element_focus(input);
        let target = ElementId::from_u64(1);

        let mut prev = TextInputState::default();
        for (state, expected) in [
            (composing("か", 0, 3), "か"),
            (composing("かん", 0, 6), "かん"),
            (committed("感"), "感"),
        ] {
            for action in translate_text_input(&prev, &state) {
                apply_ime_action(&mut tree, target, &action);
            }
            assert_eq!(tree.element_get_text_content(target), expected);
            prev = state;
        }
    }
}
