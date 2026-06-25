//! winit キー入力 → [`EditIntent`] への純粋写像と、focus 中 text-input への適用
//! （ADR-0103 / issue #507）。
//!
//! `core` は OS 非依存に intent を適用し（*何を*）、本アダプタがどのキーがどの intent を
//! 生むか（OS キーマップ）を決める。web の `edit_keymap.rs`（`key_to_edit_intent`）を雛形に
//! した desktop 版 keymap で、keymap の 2 実装目になる（将来 Core/common へ昇格させる prior
//! art。今回は昇格させず leaf に置く）。違いは入力表現だけ: web は DOM のキー文字列 +
//! ワイヤ修飾ビットを受け、desktop は winit の [`Key`] + [`ModifiersState`] を直に受ける。
//!
//! 編集として解釈しないキーは `None`。印字可能文字（`Key::Character`）は [`apply_key_input`]
//! が `on_text_input` へそのまま流し、text-input に入る。実窓・GPU 無しで unit test できるよう
//! 写像は純粋関数に切り出す（`pointer_input` と同型）。

use hayate_core::{Direction, EditIntent, ElementTree, Granularity};
use winit::keyboard::{Key, ModifiersState, NamedKey};

/// winit のキー押下を [`EditIntent`] へ写像する。編集として解釈しないキーは `None`
/// （呼び出し側が印字テキストとして扱う）。Shift は選択を拡張し、それ以外はキャレットを
/// 移動する。OS キーマップ（ADR-0103、web `edit_keymap` と同じ規約）:
///
/// - `ArrowLeft`/`ArrowRight`: 書記素単位。Alt（macOS Option）または Ctrl（Win/Linux）で
///   単語へ、Super（macOS Cmd）で行境界へ拡大。
/// - `Home`/`End`: 行境界。Ctrl（Win/Linux Ctrl+Home/End）で文書（フィールド）境界。
/// - `ArrowUp`/`ArrowDown`: 表示行間の垂直移動。Super（macOS Cmd+↑/↓）で文書境界へジャンプ。
/// - `Backspace`/`Delete`: 後方／前方へ1文字削除。Alt（macOS Option）または Ctrl（Win/Linux）
///   で単語単位へ拡大。
/// - `a`/`c`/`x`/`v` + 主修飾（Ctrl=Win/Linux または Super/Cmd=macOS）: 全選択／クリップボード。
pub fn key_to_edit_intent(key: &Key, modifiers: ModifiersState) -> Option<EditIntent> {
    let shift = modifiers.shift_key();
    let ctrl = modifiers.control_key();
    let alt = modifiers.alt_key();
    let meta = modifiers.super_key();

    // 主修飾（Ctrl=Win/Linux または Super/Cmd=macOS）でのクリップボード／全選択（ADR-0103）。
    // 先に判定することで Ctrl/Cmd+A/C/X/V がテキスト入力へ漏れず、修飾なしの a/c/x/v は
    // 印字可能なまま残る。
    if ctrl || meta {
        if let Key::Character(c) = key {
            if let Some(intent) = clipboard_intent(c) {
                return Some(intent);
            }
        }
    }

    // 削除キー（ADR-0103）: Backspace は後方、Delete は前方。Alt（macOS Option）または
    // Ctrl（Win/Linux）で書記素から単語単位へ拡大（矢印と同じ「単語単位」修飾）。
    if let Key::Named(named) = key {
        if let Some(direction) = match named {
            NamedKey::Backspace => Some(Direction::Backward),
            NamedKey::Delete => Some(Direction::Forward),
            _ => None,
        } {
            let granularity = if alt || ctrl {
                Granularity::Word
            } else {
                Granularity::Grapheme
            };
            return Some(EditIntent::Delete {
                granularity,
                direction,
            });
        }
    }

    let (granularity, direction) = match key {
        Key::Named(named @ (NamedKey::ArrowLeft | NamedKey::ArrowRight)) => {
            let direction = if *named == NamedKey::ArrowLeft {
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
        Key::Named(NamedKey::Home) => (boundary_granularity(ctrl), Direction::Backward),
        Key::Named(NamedKey::End) => (boundary_granularity(ctrl), Direction::Forward),
        // macOS Cmd+↑/↓ はフィールド端へジャンプ。
        Key::Named(NamedKey::ArrowUp) if meta => (Granularity::DocBoundary, Direction::Backward),
        Key::Named(NamedKey::ArrowDown) if meta => (Granularity::DocBoundary, Direction::Forward),
        // 修飾なし ↑/↓ は垂直移動: 複数行は表示行間、単一行はフィールド端へ。
        // 垂直移動に粒度は無関係。
        Key::Named(NamedKey::ArrowUp) => (Granularity::Grapheme, Direction::Up),
        Key::Named(NamedKey::ArrowDown) => (Granularity::Grapheme, Direction::Down),
        _ => return None,
    };

    Some(if shift {
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

/// 文字（主修飾は呼び出し側で確定済み）をクリップボード／全選択の [`EditIntent`] へ
/// 写像する（ADR-0103）。他のキーは `None` で印字テキスト経路へ流れる。
fn clipboard_intent(c: &str) -> Option<EditIntent> {
    if c.eq_ignore_ascii_case("a") {
        Some(EditIntent::SelectAll)
    } else if c.eq_ignore_ascii_case("c") {
        Some(EditIntent::Copy)
    } else if c.eq_ignore_ascii_case("x") {
        Some(EditIntent::Cut)
    } else if c.eq_ignore_ascii_case("v") {
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

/// winit `KeyboardInput`（press）を Core の編集シームへ写す唯一の経路。編集コマンド
/// （[`key_to_edit_intent`]）は focus 中 text-input に `apply_edit_intent` で適用し、
/// それ以外の印字可能文字（`Key::Character`）は `on_text_input` へそのまま入れる
/// （ADR-0103・ADR-0069）。`text` は winit `KeyEvent::text`（レイアウト/Shift を反映した
/// 挿入テキスト）。何か適用したら `true`。
///
/// 主修飾（Ctrl/Super）下の文字キーはコマンドであって印字テキストではないので挿入しない
/// （例: Ctrl+B はキャレットに 'b' を入れない）。Backspace/Enter 等の `Key::Named` は
/// 制御文字を運びうるが、`Key::Character` 限定の分岐に入らないので text には漏れない。
pub fn apply_key_input(
    tree: &mut ElementTree,
    key: &Key,
    text: Option<&str>,
    modifiers: ModifiersState,
) -> bool {
    if let Some(intent) = key_to_edit_intent(key, modifiers) {
        return match tree.focused_element() {
            Some(focused) => tree.apply_edit_intent(focused, intent),
            None => false,
        };
    }

    // 印字可能文字。主修飾なしの文字キーだけが focus 中 text-input にそのまま入る。
    if !modifiers.control_key() && !modifiers.super_key() {
        if let Key::Character(c) = key {
            if let Some(focused) = tree.focused_element() {
                tree.on_text_input(focused, text.unwrap_or(c.as_str()));
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use hayate_core::ElementKind;

    fn ch(s: &str) -> Key {
        Key::Character(s.into())
    }

    const SHIFT: ModifiersState = ModifiersState::SHIFT;
    const CTRL: ModifiersState = ModifiersState::CONTROL;
    const ALT: ModifiersState = ModifiersState::ALT;
    const META: ModifiersState = ModifiersState::SUPER;
    const NONE: ModifiersState = ModifiersState::empty();

    #[test]
    fn bare_arrows_map_to_grapheme_moves() {
        assert_eq!(
            key_to_edit_intent(&Key::Named(NamedKey::ArrowLeft), NONE),
            Some(EditIntent::Move {
                granularity: Granularity::Grapheme,
                direction: Direction::Backward,
            }),
        );
        assert_eq!(
            key_to_edit_intent(&Key::Named(NamedKey::ArrowRight), NONE),
            Some(EditIntent::Move {
                granularity: Granularity::Grapheme,
                direction: Direction::Forward,
            }),
        );
    }

    #[test]
    fn shift_arrows_map_to_grapheme_extends() {
        assert_eq!(
            key_to_edit_intent(&Key::Named(NamedKey::ArrowRight), SHIFT),
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
            key_to_edit_intent(&Key::Named(NamedKey::ArrowLeft), ALT),
            Some(EditIntent::Move {
                granularity: Granularity::Word,
                direction: Direction::Backward,
            }),
        );
        assert_eq!(
            key_to_edit_intent(&Key::Named(NamedKey::ArrowLeft), CTRL | SHIFT),
            Some(EditIntent::Extend {
                granularity: Granularity::Word,
                direction: Direction::Backward,
            }),
        );
    }

    #[test]
    fn delete_keys_map_to_char_delete_intents() {
        // 修飾なし Backspace はキャレット前、Delete は後ろの文字を削除（ADR-0103）。
        assert_eq!(
            key_to_edit_intent(&Key::Named(NamedKey::Backspace), NONE),
            Some(EditIntent::Delete {
                granularity: Granularity::Grapheme,
                direction: Direction::Backward,
            }),
        );
        assert_eq!(
            key_to_edit_intent(&Key::Named(NamedKey::Delete), NONE),
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
        for word_mod in [ALT, CTRL] {
            assert_eq!(
                key_to_edit_intent(&Key::Named(NamedKey::Backspace), word_mod),
                Some(EditIntent::Delete {
                    granularity: Granularity::Word,
                    direction: Direction::Backward,
                }),
            );
            assert_eq!(
                key_to_edit_intent(&Key::Named(NamedKey::Delete), word_mod),
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
            key_to_edit_intent(&Key::Named(NamedKey::Home), NONE),
            Some(EditIntent::Move {
                granularity: Granularity::LineBoundary,
                direction: Direction::Backward,
            }),
        );
        assert_eq!(
            key_to_edit_intent(&Key::Named(NamedKey::End), NONE),
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
            key_to_edit_intent(&Key::Named(NamedKey::Home), CTRL),
            Some(EditIntent::Move {
                granularity: Granularity::DocBoundary,
                direction: Direction::Backward,
            }),
        );
        assert_eq!(
            key_to_edit_intent(&Key::Named(NamedKey::End), CTRL),
            Some(EditIntent::Move {
                granularity: Granularity::DocBoundary,
                direction: Direction::Forward,
            }),
        );
    }

    #[test]
    fn shift_home_and_end_extend_to_the_boundary() {
        assert_eq!(
            key_to_edit_intent(&Key::Named(NamedKey::End), SHIFT),
            Some(EditIntent::Extend {
                granularity: Granularity::LineBoundary,
                direction: Direction::Forward,
            }),
        );
        assert_eq!(
            key_to_edit_intent(&Key::Named(NamedKey::Home), SHIFT | CTRL),
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
            key_to_edit_intent(&Key::Named(NamedKey::ArrowLeft), META),
            Some(EditIntent::Move {
                granularity: Granularity::LineBoundary,
                direction: Direction::Backward,
            }),
        );
        assert_eq!(
            key_to_edit_intent(&Key::Named(NamedKey::ArrowRight), META | SHIFT),
            Some(EditIntent::Extend {
                granularity: Granularity::LineBoundary,
                direction: Direction::Forward,
            }),
        );
        assert_eq!(
            key_to_edit_intent(&Key::Named(NamedKey::ArrowUp), META),
            Some(EditIntent::Move {
                granularity: Granularity::DocBoundary,
                direction: Direction::Backward,
            }),
        );
        assert_eq!(
            key_to_edit_intent(&Key::Named(NamedKey::ArrowDown), META),
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
            key_to_edit_intent(&Key::Named(NamedKey::ArrowUp), NONE),
            Some(EditIntent::Move {
                granularity: Granularity::Grapheme,
                direction: Direction::Up,
            }),
        );
        assert_eq!(
            key_to_edit_intent(&Key::Named(NamedKey::ArrowDown), NONE),
            Some(EditIntent::Move {
                granularity: Granularity::Grapheme,
                direction: Direction::Down,
            }),
        );
        assert_eq!(
            key_to_edit_intent(&Key::Named(NamedKey::ArrowDown), SHIFT),
            Some(EditIntent::Extend {
                granularity: Granularity::Grapheme,
                direction: Direction::Down,
            }),
        );
    }

    #[test]
    fn non_editing_keys_are_not_edit_intents() {
        // Enter や印字キーは編集 intent ではない（印字テキスト経路へフォールスルー）。
        assert_eq!(key_to_edit_intent(&Key::Named(NamedKey::Enter), NONE), None);
        assert_eq!(key_to_edit_intent(&ch("a"), NONE), None);
    }

    #[test]
    fn primary_modifier_letters_map_to_clipboard_and_select_all_intents() {
        // 主修飾（Ctrl=Win/Linux または Super/Cmd=macOS）が a/c/x/v をクリップボード／
        // 全選択 intent に変える（ADR-0103）。両修飾・大小文字とも対応。
        for primary in [CTRL, META] {
            assert_eq!(key_to_edit_intent(&ch("a"), primary), Some(EditIntent::SelectAll));
            assert_eq!(key_to_edit_intent(&ch("A"), primary), Some(EditIntent::SelectAll));
            assert_eq!(key_to_edit_intent(&ch("c"), primary), Some(EditIntent::Copy));
            assert_eq!(key_to_edit_intent(&ch("x"), primary), Some(EditIntent::Cut));
            assert_eq!(key_to_edit_intent(&ch("v"), primary), Some(EditIntent::Paste));
        }
    }

    #[test]
    fn bare_letters_are_not_clipboard_intents() {
        // 主修飾なしでは印字テキストでありコマンドではない。"c"/"v" の入力が文字を
        // 挿入できるよう、フォールスルーが必須。
        assert_eq!(key_to_edit_intent(&ch("c"), NONE), None);
        assert_eq!(key_to_edit_intent(&ch("v"), NONE), None);
        assert_eq!(key_to_edit_intent(&ch("a"), ALT), None);
    }

    // ── apply_key_input: winit イベント → focus 中 text-input への適用 ──────────

    /// `content` を保持しキャレットが末尾にあるフォーカス済みテキスト入力。
    fn focused_input(content: &str) -> (ElementTree, hayate_core::ElementId) {
        let mut tree = ElementTree::new();
        let input = tree.element_create(1, ElementKind::TextInput);
        tree.set_root(input);
        tree.element_focus(input);
        tree.element_append_text_content(input, content);
        (tree, input)
    }

    #[test]
    fn typing_an_ascii_character_inserts_it_into_the_focused_input() {
        // criterion #1: focus 中 text-input への ASCII タイプが入力される。winit の
        // KeyEvent.text（ここでは "X"）がそのまま挿入される。
        let (mut tree, input) = focused_input("ab"); // キャレットは末尾 (2)
        assert!(apply_key_input(&mut tree, &ch("X"), Some("X"), NONE));
        assert_eq!(tree.element_get_text_content(input), "abX");
    }

    #[test]
    fn a_bare_arrow_moves_the_caret_through_the_seam() {
        // criterion #2: 矢印が EditIntent seam 経由でキャレットを動かす。
        let (mut tree, input) = focused_input("hi"); // キャレットは末尾 (2)
        assert!(apply_key_input(&mut tree, &Key::Named(NamedKey::ArrowLeft), None, NONE));
        assert_eq!(tree.element_caret_byte_index(input), Some(1));
    }

    #[test]
    fn shift_arrow_extends_a_selection_through_the_seam() {
        // criterion #5: Shift で選択拡張。
        let (mut tree, input) = focused_input("hello"); // キャレットは末尾 (5)
        assert!(apply_key_input(&mut tree, &Key::Named(NamedKey::ArrowLeft), None, SHIFT));
        assert!(apply_key_input(&mut tree, &Key::Named(NamedKey::ArrowLeft), None, SHIFT));
        assert_eq!(tree.element_text_selection(input), Some((3, 5)));
    }

    #[test]
    fn primary_a_selects_all_through_the_seam() {
        // criterion #5: Cmd(macOS)/Ctrl(Win/Linux)+A が全選択。両主修飾を確認する。
        for primary in [CTRL, META] {
            let (mut tree, input) = focused_input("hello");
            assert!(apply_key_input(&mut tree, &ch("a"), Some("a"), primary));
            assert_eq!(tree.element_text_selection(input), Some((0, 5)));
        }
    }

    #[test]
    fn backspace_deletes_the_grapheme_before_the_caret_through_the_seam() {
        // criterion #4: Backspace で後方削除。
        let (mut tree, input) = focused_input("hello");
        assert!(apply_key_input(&mut tree, &Key::Named(NamedKey::Backspace), None, NONE));
        assert_eq!(tree.element_get_text_content(input), "hell");
    }

    #[test]
    fn a_primary_modified_letter_does_not_insert_text() {
        // 主修飾下の文字キーはコマンドであり印字テキストではない。Ctrl+B は edit intent
        // でないが、'b' をフィールドに入れてはならない（text 経路をゲートする）。
        let (mut tree, input) = focused_input("ab");
        assert!(!apply_key_input(&mut tree, &ch("b"), Some("b"), CTRL));
        assert_eq!(tree.element_get_text_content(input), "ab", "Ctrl+B は何も挿入しない");
    }

    #[test]
    fn typing_without_focus_is_a_no_op() {
        // focus 中の要素が無ければ挿入も intent 適用もしない（パニックしない）。
        let mut tree = ElementTree::new();
        let root = tree.element_create(1, ElementKind::View);
        tree.set_root(root);
        assert!(!apply_key_input(&mut tree, &ch("X"), Some("X"), NONE));
        assert!(!apply_key_input(&mut tree, &Key::Named(NamedKey::ArrowLeft), None, NONE));
    }
}
