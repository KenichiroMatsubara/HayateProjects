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
/// - `Backspace`/`Delete`: remove one char backward / forward.
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
}
