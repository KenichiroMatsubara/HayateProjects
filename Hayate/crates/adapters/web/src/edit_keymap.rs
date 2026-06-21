//! OS のキー入力を [`EditIntent`] へ写像する Platform Adapter（ADR-0103）。
//! `core` は OS 非依存に intent を適用し（*何を*）、本アダプタがどのキーが
//! どの intent を生むか（OS キーマップ）を決める。Canvas 経路は生のキー押下を
//! ここに通し、intent が得られたら生の `on_key_down` 解釈ではなく
//! `ElementTree::apply_edit_intent` を駆動する。

use hayate_core::{Direction, EditIntent, Granularity};

/// キー押下に付随する修飾キーのビットフィールド。ワイヤ契約 `MODIFIER_*`
/// に一致する: SHIFT=1, CTRL=2, ALT=4, META=8。
const MOD_SHIFT: u32 = 1;
const MOD_CTRL: u32 = 2;
const MOD_ALT: u32 = 4;
const MOD_META: u32 = 8;

/// キー押下を [`EditIntent`] へ写像する。本アダプタが編集として解釈しない
/// キーは `None`（呼び出し側が生の `on_key_down` にフォールバックする）。
/// Shift は選択を拡張し、それ以外はキャレットを移動する。OS キーマップ
/// （ADR-0103）:
///
/// - `ArrowLeft`/`ArrowRight`: 書記素単位。Alt（macOS Option）または
///   Ctrl（Win/Linux）で単語へ、Meta（macOS Cmd）で行境界へ拡大。
/// - `Home`/`End`: 行境界。Ctrl（Win/Linux Ctrl+Home/End）で文書（フィールド）境界。
/// - `ArrowUp`/`ArrowDown`: 表示行間の垂直移動。Meta（macOS Cmd+↑/↓）では
///   文書境界へジャンプする。
/// - `Backspace`/`Delete`: 後方／前方へ1文字削除。Alt（macOS Option）または
///   Ctrl（Win/Linux）で単語単位へ拡大。
pub fn key_to_edit_intent(key: &str, modifiers: u32) -> Option<EditIntent> {
    // プライマリ修飾（Ctrl=Win/Linux または Meta/Cmd=macOS）でのクリップボード
    // ／全選択（ADR-0103）。先に判定することで Ctrl/Cmd+A/C/X/V がテキスト入力へ
    // 漏れず、修飾なしの a/c/x/v は印字可能なまま残る。
    if modifiers & (MOD_CTRL | MOD_META) != 0 {
        if let Some(intent) = clipboard_intent(key) {
            return Some(intent);
        }
    }
    // 削除キー（ADR-0103）: Backspace は後方、Delete は前方。Alt（macOS Option）
    // または Ctrl（Win/Linux）で書記素から単語単位へ拡大（矢印と同じ「単語単位」修飾）。
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
                Granularity::Word // macOS Option ／ Win/Linux Ctrl
            } else {
                Granularity::Grapheme
            };
            (granularity, direction)
        }
        // Win/Linux の行／文書端。Ctrl で Home/End をフィールド端まで拡大。
        "Home" => (boundary_granularity(ctrl), Direction::Backward),
        "End" => (boundary_granularity(ctrl), Direction::Forward),
        // macOS Cmd+↑/↓ はフィールド端へジャンプ。
        "ArrowUp" if meta => (Granularity::DocBoundary, Direction::Backward),
        "ArrowDown" if meta => (Granularity::DocBoundary, Direction::Forward),
        // 修飾なし ↑/↓ は垂直移動: 複数行は表示行間、単一行はフィールド端へ。
        // 垂直移動に粒度は無関係。
        "ArrowUp" => (Granularity::Grapheme, Direction::Up),
        "ArrowDown" => (Granularity::Grapheme, Direction::Down),
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

/// 文字（プライマリ修飾は呼び出し側で確定済み）をクリップボード／全選択の
/// [`EditIntent`] へ写像する（ADR-0103）。他のキーは `None` で生の入力経路へ流れる。
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

/// Home/End 押下の境界粒度: Ctrl ありはフィールド全体、なしは現在の行
/// （単一行では両者は同義）。
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
        // Option+Arrow（macOS）と Ctrl+Arrow（Win/Linux）はどちらも「単語単位」。
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
        // 修飾なし Backspace はキャレット前、Delete は後ろの文字を削除（ADR-0103）。
        // 単語拡大の修飾は別テストで確認する。
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
        // Option+Backspace（macOS）と Ctrl+Backspace（Win/Linux）は前の単語を、
        // 同じ修飾の Delete は次の単語を削除する。
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
        // Win/Linux Home/End → 行境界（単一行ではフィールド端と同じ）。
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
        // Win/Linux Ctrl+Home/End → 文書（フィールド）境界。
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
        // Cmd+←/→ = 行端、Cmd+↑/↓ = 文書端（macOS）、Shift で拡張。
        // 「単語単位」の Ctrl/Option とは別。
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
    fn bare_vertical_arrows_map_to_vertical_motion() {
        // 修飾なし ↑/↓ は表示行間（複数行）またはフィールド端（単一行）へ移動、
        // Shift で拡張。Cmd+↑/↓（文書境界）とは別。
        assert_eq!(
            key_to_edit_intent("ArrowUp", 0),
            Some(EditIntent::Move {
                granularity: Granularity::Grapheme,
                direction: Direction::Up,
            }),
        );
        assert_eq!(
            key_to_edit_intent("ArrowDown", 0),
            Some(EditIntent::Move {
                granularity: Granularity::Grapheme,
                direction: Direction::Down,
            }),
        );
        assert_eq!(
            key_to_edit_intent("ArrowDown", MOD_SHIFT),
            Some(EditIntent::Extend {
                granularity: Granularity::Grapheme,
                direction: Direction::Down,
            }),
        );
    }

    #[test]
    fn non_editing_keys_are_not_edit_intents() {
        // Enter や印字キーは生の on_key_down へフォールスルーする。
        assert_eq!(key_to_edit_intent("Enter", 0), None);
        assert_eq!(key_to_edit_intent("a", 0), None);
    }

    #[test]
    fn primary_modifier_letters_map_to_clipboard_and_select_all_intents() {
        // プライマリ修飾（Ctrl=Win/Linux または Meta/Cmd=macOS）が a/c/x/v を
        // クリップボード／全選択 intent に変える（ADR-0103）。両修飾・大小文字とも対応。
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
        // プライマリ修飾なしでは印字テキストでありコマンドではない。
        // "c"/"v" の入力が文字を挿入できるよう、フォールスルーが必須。
        assert_eq!(key_to_edit_intent("c", 0), None);
        assert_eq!(key_to_edit_intent("v", 0), None);
        assert_eq!(key_to_edit_intent("a", MOD_ALT), None);
    }
}
