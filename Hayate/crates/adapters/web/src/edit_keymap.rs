//! OS keystroke → [`EditIntent`] mapping owned by the Platform Adapter
//! (ADR-0103). `core` applies intents OS-independently (the *what*); this
//! adapter decides *which* key produces which intent (the OS keymap). The
//! Canvas path runs a raw key press through here and, when it yields an intent,
//! drives `ElementTree::apply_edit_intent` instead of the raw `on_key_down`
//! edit interpretation.
//!
//! This tracer covers the OS-independent horizontal arrows; later slices add the
//! OS-specific bindings (macOS Cmd/Option, Win/Linux Home/End/Ctrl) here without
//! touching `core`.

use hayate_core::{Direction, EditIntent, Granularity};

/// Modifier-key bitfield carried alongside a key press, matching the wire
/// `MODIFIER_*` contract (proto/spec): SHIFT=1, CTRL=2, ALT=4, META=8.
const MOD_SHIFT: u32 = 1;
const MOD_CTRL: u32 = 2;
const MOD_ALT: u32 = 4;
const MOD_META: u32 = 8;

/// Map a key press to an [`EditIntent`], or `None` for keys this adapter does
/// not interpret as editing (so the caller falls back to raw `on_key_down`).
/// Shift extends the selection, otherwise the caret moves. The OS keymap
/// (ADR-0103, #360):
///
/// - `ArrowLeft`/`ArrowRight`: a grapheme step, widened to a word by Alt
///   (macOS Option) or Ctrl (Win/Linux), or to the line boundary by Meta
///   (macOS Cmd).
/// - `Home`/`End`: the line boundary, or the document (field) boundary with Ctrl
///   (Win/Linux Ctrl+Home/End).
/// - `ArrowUp`/`ArrowDown` with Meta (macOS Cmd+↑/↓): the document boundary.
/// - `Backspace`/`Delete`: remove one char backward / forward, widened to a word
///   by Alt (macOS Option) or Ctrl (Win/Linux).
pub fn key_to_edit_intent(key: &str, modifiers: u32) -> Option<EditIntent> {
    // Clipboard / select-all on the primary modifier — Ctrl (Win/Linux) or
    // Meta/Cmd (macOS), the OS abstraction core calls `MOD_PRIMARY` (ADR-0103
    // §5③, #361). Checked first so Ctrl/Cmd+A/C/X/V never fall through to text
    // input; bare a/c/x/v stay printable.
    if modifiers & (MOD_CTRL | MOD_META) != 0 {
        if let Some(intent) = clipboard_intent(key) {
            return Some(intent);
        }
    }
    // Delete keys (ADR-0103): Backspace backward, Delete forward — widened from a
    // grapheme to a whole word by Alt (macOS Option) or Ctrl (Win/Linux), the
    // same "by word" modifiers as the arrows (#363).
    if let Some(direction) = match key {
        "Backspace" => Some(Direction::Backward),
        "Delete" => Some(Direction::Forward),
        _ => None,
    } {
        let granularity = if modifiers & (MOD_ALT | MOD_CTRL) != 0 {
            Granularity::Word
        } else {
            Granularity::Grapheme
        };
        return Some(EditIntent::Delete {
            granularity,
            direction,
        });
    }

    let ctrl = modifiers & MOD_CTRL != 0;
    let alt = modifiers & MOD_ALT != 0;
    let meta = modifiers & MOD_META != 0;

    let (granularity, direction) = match key {
        "ArrowLeft" | "ArrowRight" => {
            let direction = if key == "ArrowLeft" {
                Direction::Backward
            } else {
                Direction::Forward
            };
            let granularity = if meta {
                Granularity::LineBoundary // macOS Cmd+←/→
            } else if alt || ctrl {
                Granularity::Word // macOS Option / Win/Linux Ctrl
            } else {
                Granularity::Grapheme
            };
            (granularity, direction)
        }
        // Win/Linux line/document ends. Ctrl widens Home/End to the field ends.
        "Home" => (boundary_granularity(ctrl), Direction::Backward),
        "End" => (boundary_granularity(ctrl), Direction::Forward),
        // macOS Cmd+↑/↓ jump to the field ends.
        "ArrowUp" if meta => (Granularity::DocBoundary, Direction::Backward),
        "ArrowDown" if meta => (Granularity::DocBoundary, Direction::Forward),
        _ => return None,
    };

    Some(if modifiers & MOD_SHIFT != 0 {
        EditIntent::Extend {
            granularity,
            direction,
        }
    } else {
        EditIntent::Move {
            granularity,
            direction,
        }
    })
}

/// Map a letter (with the primary modifier already established by the caller) to
/// its clipboard / select-all [`EditIntent`] (ADR-0103 §5③). `None` for any other
/// key, so the press falls through to the raw input path.
fn clipboard_intent(key: &str) -> Option<EditIntent> {
    if key.eq_ignore_ascii_case("a") {
        Some(EditIntent::SelectAll)
    } else if key.eq_ignore_ascii_case("c") {
        Some(EditIntent::Copy)
    } else if key.eq_ignore_ascii_case("x") {
        Some(EditIntent::Cut)
    } else if key.eq_ignore_ascii_case("v") {
        Some(EditIntent::Paste)
    } else {
        None
    }
}

/// The boundary granularity for a Home/End press: the whole field with Ctrl,
/// otherwise the current line (equal in single-line semantics, #360).
fn boundary_granularity(ctrl: bool) -> Granularity {
    if ctrl {
        Granularity::DocBoundary
    } else {
        Granularity::LineBoundary
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_arrows_map_to_grapheme_moves() {
        assert_eq!(
            key_to_edit_intent("ArrowLeft", 0),
            Some(EditIntent::Move {
                granularity: Granularity::Grapheme,
                direction: Direction::Backward,
            }),
        );
        assert_eq!(
            key_to_edit_intent("ArrowRight", 0),
            Some(EditIntent::Move {
                granularity: Granularity::Grapheme,
                direction: Direction::Forward,
            }),
        );
    }

    #[test]
    fn shift_arrows_map_to_grapheme_extends() {
        assert_eq!(
            key_to_edit_intent("ArrowRight", MOD_SHIFT),
            Some(EditIntent::Extend {
                granularity: Granularity::Grapheme,
                direction: Direction::Forward,
            }),
        );
    }

    #[test]
    fn alt_or_ctrl_widens_the_step_to_a_word() {
        // Option+Arrow (macOS) and Ctrl+Arrow (Win/Linux) both mean "by word".
        assert_eq!(
            key_to_edit_intent("ArrowLeft", MOD_ALT),
            Some(EditIntent::Move {
                granularity: Granularity::Word,
                direction: Direction::Backward,
            }),
        );
        assert_eq!(
            key_to_edit_intent("ArrowLeft", MOD_CTRL | MOD_SHIFT),
            Some(EditIntent::Extend {
                granularity: Granularity::Word,
                direction: Direction::Backward,
            }),
        );
    }

    #[test]
    fn delete_keys_map_to_char_delete_intents() {
        // Bare Backspace removes the char before the caret, bare Delete the one
        // after (ADR-0103); the word-widening modifiers are covered separately.
        assert_eq!(
            key_to_edit_intent("Backspace", 0),
            Some(EditIntent::Delete {
                granularity: Granularity::Grapheme,
                direction: Direction::Backward,
            }),
        );
        assert_eq!(
            key_to_edit_intent("Delete", 0),
            Some(EditIntent::Delete {
                granularity: Granularity::Grapheme,
                direction: Direction::Forward,
            }),
        );
    }

    #[test]
    fn alt_or_ctrl_widens_delete_to_a_word() {
        // #363: Option+Backspace (macOS) and Ctrl+Backspace (Win/Linux) delete
        // the previous word; the same modifiers on Delete remove the next word.
        for word_mod in [MOD_ALT, MOD_CTRL] {
            assert_eq!(
                key_to_edit_intent("Backspace", word_mod),
                Some(EditIntent::Delete {
                    granularity: Granularity::Word,
                    direction: Direction::Backward,
                }),
            );
            assert_eq!(
                key_to_edit_intent("Delete", word_mod),
                Some(EditIntent::Delete {
                    granularity: Granularity::Word,
                    direction: Direction::Forward,
                }),
            );
        }
    }

    #[test]
    fn home_and_end_map_to_line_boundary_moves() {
        // Win/Linux Home/End → line boundary (= field end in single-line).
        assert_eq!(
            key_to_edit_intent("Home", 0),
            Some(EditIntent::Move {
                granularity: Granularity::LineBoundary,
                direction: Direction::Backward,
            }),
        );
        assert_eq!(
            key_to_edit_intent("End", 0),
            Some(EditIntent::Move {
                granularity: Granularity::LineBoundary,
                direction: Direction::Forward,
            }),
        );
    }

    #[test]
    fn ctrl_home_and_end_map_to_doc_boundary_moves() {
        // Win/Linux Ctrl+Home/End → document (field) boundary.
        assert_eq!(
            key_to_edit_intent("Home", MOD_CTRL),
            Some(EditIntent::Move {
                granularity: Granularity::DocBoundary,
                direction: Direction::Backward,
            }),
        );
        assert_eq!(
            key_to_edit_intent("End", MOD_CTRL),
            Some(EditIntent::Move {
                granularity: Granularity::DocBoundary,
                direction: Direction::Forward,
            }),
        );
    }

    #[test]
    fn shift_home_and_end_extend_to_the_boundary() {
        assert_eq!(
            key_to_edit_intent("End", MOD_SHIFT),
            Some(EditIntent::Extend {
                granularity: Granularity::LineBoundary,
                direction: Direction::Forward,
            }),
        );
        assert_eq!(
            key_to_edit_intent("Home", MOD_SHIFT | MOD_CTRL),
            Some(EditIntent::Extend {
                granularity: Granularity::DocBoundary,
                direction: Direction::Backward,
            }),
        );
    }

    #[test]
    fn macos_cmd_arrows_map_to_line_and_doc_boundaries() {
        // Cmd+←/→ = line ends, Cmd+↑/↓ = document ends (macOS), with Shift
        // extending. Distinct from Ctrl/Option, which mean "by word".
        assert_eq!(
            key_to_edit_intent("ArrowLeft", MOD_META),
            Some(EditIntent::Move {
                granularity: Granularity::LineBoundary,
                direction: Direction::Backward,
            }),
        );
        assert_eq!(
            key_to_edit_intent("ArrowRight", MOD_META | MOD_SHIFT),
            Some(EditIntent::Extend {
                granularity: Granularity::LineBoundary,
                direction: Direction::Forward,
            }),
        );
        assert_eq!(
            key_to_edit_intent("ArrowUp", MOD_META),
            Some(EditIntent::Move {
                granularity: Granularity::DocBoundary,
                direction: Direction::Backward,
            }),
        );
        assert_eq!(
            key_to_edit_intent("ArrowDown", MOD_META),
            Some(EditIntent::Move {
                granularity: Granularity::DocBoundary,
                direction: Direction::Forward,
            }),
        );
    }

    #[test]
    fn bare_vertical_arrows_are_not_edit_intents_yet() {
        // Single-line ↑/↓ without Cmd is vertical motion, deferred to #7; the
        // adapter leaves it to the raw key path.
        assert_eq!(key_to_edit_intent("ArrowUp", 0), None);
        assert_eq!(key_to_edit_intent("ArrowDown", 0), None);
    }

    #[test]
    fn non_editing_keys_are_not_edit_intents() {
        // Enter / printable keys fall through to raw on_key_down.
        assert_eq!(key_to_edit_intent("Enter", 0), None);
        assert_eq!(key_to_edit_intent("a", 0), None);
    }

    #[test]
    fn primary_modifier_letters_map_to_clipboard_and_select_all_intents() {
        // Ctrl (Win/Linux) or Meta/Cmd (macOS) — the primary modifier — turns
        // a/c/x/v into the clipboard / select-all members of the vocabulary
        // (ADR-0103 §5③, #361). Both modifiers and either letter case map.
        for primary in [MOD_CTRL, MOD_META] {
            assert_eq!(key_to_edit_intent("a", primary), Some(EditIntent::SelectAll));
            assert_eq!(key_to_edit_intent("A", primary), Some(EditIntent::SelectAll));
            assert_eq!(key_to_edit_intent("c", primary), Some(EditIntent::Copy));
            assert_eq!(key_to_edit_intent("x", primary), Some(EditIntent::Cut));
            assert_eq!(key_to_edit_intent("v", primary), Some(EditIntent::Paste));
        }
    }

    #[test]
    fn bare_letters_are_not_clipboard_intents() {
        // Without the primary modifier these are printable text, not commands —
        // they must fall through so typing "c"/"v" inserts characters.
        assert_eq!(key_to_edit_intent("c", 0), None);
        assert_eq!(key_to_edit_intent("v", 0), None);
        assert_eq!(key_to_edit_intent("a", MOD_ALT), None);
    }
}
