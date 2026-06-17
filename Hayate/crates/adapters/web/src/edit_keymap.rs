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

/// Map a key press to an [`EditIntent`], or `None` for keys this adapter does
/// not interpret as editing (so the caller falls back to raw `on_key_down`).
/// Shift extends the selection, otherwise the caret moves; Alt (macOS) or Ctrl
/// (Win/Linux) widens a horizontal step from a grapheme to a word. Backspace and
/// Delete remove one char backward / forward.
pub fn key_to_edit_intent(key: &str, modifiers: u32) -> Option<EditIntent> {
    // Char delete keys (ADR-0103): Backspace backward, Delete forward. Word
    // granularity (Ctrl/Alt) is a later slice — char only here.
    match key {
        "Backspace" => {
            return Some(EditIntent::Delete {
                granularity: Granularity::Grapheme,
                direction: Direction::Backward,
            })
        }
        "Delete" => {
            return Some(EditIntent::Delete {
                granularity: Granularity::Grapheme,
                direction: Direction::Forward,
            })
        }
        _ => {}
    }
    let direction = match key {
        "ArrowLeft" => Direction::Backward,
        "ArrowRight" => Direction::Forward,
        _ => return None,
    };
    let granularity = if modifiers & (MOD_ALT | MOD_CTRL) != 0 {
        Granularity::Word
    } else {
        Granularity::Grapheme
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
        // Backspace removes the char before the caret, Delete the one after
        // (ADR-0103). Word granularity is a later slice — char only here.
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
    fn non_editing_keys_are_not_edit_intents() {
        // Enter / printable keys fall through to raw on_key_down.
        assert_eq!(key_to_edit_intent("Enter", 0), None);
        assert_eq!(key_to_edit_intent("a", 0), None);
    }
}
