//! winit IME 入力 → Core 所有の *増分 command* 入力モデル（[`ImeCommand`] →
//! `apply_command` → [`ImeAction`] → `apply_ime_action`・ADR-0117）への配線（issue #508）。
//!
//! winit の [`Ime`] 抽象（`Enabled` / `Preedit` / `Commit` / `Disabled`）だけを glue し、
//! native IME（TSM/TSF/IBus）は直叩きしない。`core` が編集セマンティクス（marked text の
//! 置換・確定、preedit の管理）を持ち（*何を*）、本アダプタは winit イベントがどの
//! [`ImeCommand`] を生むか（*どの OS イベントが*）だけを決める。keyboard の
//! [`key_to_edit_intent`](crate::keyboard_input::key_to_edit_intent) と同型に、実窓・GPU 無しで
//! unit test できるよう写像は純粋関数に切り出す。
//!
//! 候補ウィンドウ配置（`set_ime_cursor_area`）と focus/blur に応じた IME enable/disable は、
//! core の [`drive_ime`](hayate_core::ElementTree::drive_ime) が一元的に決め
//! （[`ImePresentation`](hayate_core::ImePresentation)）、Platform Front の `ImeBridge`
//! 実装が winit `Window` へ反映する（`lib.rs`）。

use hayate_core::{
    apply_command, apply_ime_action, CharacterBounds, CompositionClause, CompositionUnderline,
    ElementTree, ImeAction, ImeBuffer, ImeCommand,
};
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::Ime;

/// winit の [`Ime`] イベントを Core の増分 [`ImeCommand`] へ写す純粋関数。バッファに
/// 影響しないイベント（IME の有効化／無効化）は `None`。
///
/// - [`Ime::Preedit`] → [`ImeCommand::SetMarked`]: 変換中の marked text（preedit）を置換する。
///   空文字の preedit も `SetMarked("")` として preedit を消すために流す。
/// - [`Ime::Commit`] → [`ImeCommand::Insert`]: marked text を確定文字列で置換して確定する
///   （IME の候補確定経路）。
/// - [`Ime::Enabled`] / [`Ime::Disabled`] → `None`: IME の可否制御であってバッファ command
///   ではない（可否は core `drive_ime` 経由で `set_ime_allowed` に反映する）。
pub fn ime_to_command(ime: &Ime) -> Option<ImeCommand> {
    match ime {
        Ime::Preedit(text, _cursor) => Some(ImeCommand::SetMarked(text.clone())),
        Ime::Commit(text) => Some(ImeCommand::Insert(text.clone())),
        _ => None,
    }
}

/// core が算出したキャレットの [`CharacterBounds`]（レイアウト＝論理 px）を winit
/// `set_ime_cursor_area` 引数（[`LogicalPosition`] / [`LogicalSize`]）へ写す純粋関数。
/// 変換候補ウィンドウを focus input のキャレット位置に出すための配線（criterion #4）。
/// 論理座標で渡し、物理への換算は winit が `scale_factor` で行う（ADR-0080 と同じく
/// `scale_factor` を潰さない）。
pub fn ime_cursor_area(bounds: CharacterBounds) -> (LogicalPosition<f64>, LogicalSize<f64>) {
    (
        LogicalPosition::new(bounds.x as f64, bounds.y as f64),
        LogicalSize::new(bounds.width as f64, bounds.height as f64),
    )
}

/// winit の preedit cursor 範囲を変換文節フォーマット（ADR-0102）へ写す純粋関数。
///
/// winit は文節ごとの下線太さを公開せず、preedit テキストと（platform IME が報告する）
/// cursor/選択のバイト範囲だけを `Ime::Preedit` に載せる。日本語 IME は変換中の active 文節を
/// この選択範囲として報告するので、その範囲を太線（[`CompositionUnderline::Thick`]）、前後を
/// 細線（[`CompositionUnderline::Thin`]）の文節にする。範囲が無い／キャレットのみ（空範囲）／
/// 不正（範囲外・非 char 境界）なら空 — core 側が preedit 全体を単一の細線で描く。
pub fn preedit_clauses(preedit: &str, cursor: Option<(usize, usize)>) -> Vec<CompositionClause> {
    let Some((start, end)) = cursor else {
        return Vec::new();
    };
    if start >= end
        || end > preedit.len()
        || !preedit.is_char_boundary(start)
        || !preedit.is_char_boundary(end)
    {
        return Vec::new();
    }
    let mut clauses = Vec::new();
    if start > 0 {
        clauses.push(CompositionClause {
            start: 0,
            end: start,
            underline: CompositionUnderline::Thin,
        });
    }
    clauses.push(CompositionClause {
        start,
        end,
        underline: CompositionUnderline::Thick,
    });
    if end < preedit.len() {
        clauses.push(CompositionClause {
            start: end,
            end: preedit.len(),
            underline: CompositionUnderline::Thin,
        });
    }
    clauses
}

/// winit `Ime` イベントを Core の増分 IME 入力モデルへ通す唯一の経路。[`ime_to_command`]
/// で [`ImeCommand`] に写し、`buf`（フレームをまたいで持ち回るローカルバッファ）へ
/// `apply_command` で畳み、得た [`ImeAction`](hayate_core::ImeAction) を focus 中 text-input へ
/// `apply_ime_action` で適用する（ADR-0117）。何か適用したら `true`。
///
/// バッファに影響しないイベント（`Enabled` / `Disabled`）や、focus 中要素が無いとき、
/// 変化が無いとき（例: 既に空の preedit を再度空へ）は `false`（keyboard seam と同型に
/// 何も起きない）。
pub fn apply_ime_input(tree: &mut ElementTree, ime: &Ime, buf: &mut ImeBuffer) -> bool {
    let Some(command) = ime_to_command(ime) else {
        return false;
    };
    let Some(target) = tree.focused_element() else {
        return false;
    };
    let actions = apply_command(buf, command);
    if actions.is_empty() {
        return false;
    }
    // 文節フォーマット（下線太さ）を運ぶのは `Ime::Preedit` だけ。preedit を張り直す際は
    // クラス付きセッターで active 文節を太線に描けるようにする（criterion #2・ADR-0102）。
    let clauses = match ime {
        Ime::Preedit(text, cursor) => Some(preedit_clauses(text, *cursor)),
        _ => None,
    };
    for action in &actions {
        match (action, &clauses) {
            (ImeAction::SetPreedit(preedit), Some(clauses)) if !preedit.is_empty() => {
                tree.element_set_preedit_with_clauses(target, preedit, clauses.clone());
            }
            _ => apply_ime_action(tree, target, action),
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use hayate_core::{CompositionUnderline, ElementId, ElementKind};

    /// `content` を確定済みに持つフォーカス済みテキスト入力。
    fn focused_input(content: &str) -> (ElementTree, ElementId) {
        let mut tree = ElementTree::new();
        let input = tree.element_create(1, ElementKind::TextInput);
        tree.set_root(input);
        tree.element_focus(input);
        if !content.is_empty() {
            tree.element_append_text_content(input, content);
        }
        (tree, input)
    }

    #[test]
    fn preedit_maps_to_set_marked() {
        // 変換中の marked text は SetMarked に写る（criterion #6 の中核写像）。
        assert_eq!(
            ime_to_command(&Ime::Preedit("ぎゅう".into(), None)),
            Some(ImeCommand::SetMarked("ぎゅう".into())),
        );
    }

    #[test]
    fn commit_maps_to_insert() {
        // 確定文字列の commit は Insert に写る（marked を置換して確定する確定経路）。
        assert_eq!(
            ime_to_command(&Ime::Commit("牛乳".into())),
            Some(ImeCommand::Insert("牛乳".into())),
        );
    }

    #[test]
    fn enable_and_disable_are_not_buffer_commands() {
        // IME の有効化／無効化は可否制御であってバッファ command ではない。可否は
        // core drive_ime → set_ime_allowed で扱う（criterion #5）。
        assert_eq!(ime_to_command(&Ime::Enabled), None);
        assert_eq!(ime_to_command(&Ime::Disabled), None);
    }

    // ── ime_cursor_area: キャレット bounds → winit set_ime_cursor_area 引数 ──────

    #[test]
    fn character_bounds_map_to_logical_cursor_area() {
        // 候補ウィンドウはキャレットの位置・寸法（論理 px）にそのまま向ける。
        let bounds = CharacterBounds {
            x: 12.0,
            y: 34.0,
            width: 2.0,
            height: 18.0,
        };
        let (pos, size) = ime_cursor_area(bounds);
        assert_eq!(pos, LogicalPosition::new(12.0, 34.0));
        assert_eq!(size, LogicalSize::new(2.0, 18.0));
    }

    // ── preedit_clauses: winit cursor 範囲 → 変換文節フォーマット ───────────────

    #[test]
    fn no_cursor_range_yields_no_clauses() {
        // 範囲なし（変換前）／キャレットのみは文節を作らない（core が単一細線で描く）。
        assert!(preedit_clauses("ぎゅう", None).is_empty());
        assert!(preedit_clauses("ぎゅう", Some((3, 3))).is_empty());
    }

    #[test]
    fn a_full_range_active_clause_has_no_thin_tails() {
        // active 文節が preedit 全体なら太線一本だけ（前後の細線テールは出さない）。
        let len = "ぎゅう".len();
        assert_eq!(
            preedit_clauses("ぎゅう", Some((0, len))),
            vec![CompositionClause {
                start: 0,
                end: len,
                underline: CompositionUnderline::Thick,
            }],
        );
    }

    #[test]
    fn an_out_of_bounds_or_unaligned_range_yields_no_clauses() {
        // 範囲外・非 char 境界は壊れた入力として無視し、単一細線へフォールバックする。
        assert!(preedit_clauses("ぎゅう", Some((0, 99))).is_empty());
        assert!(preedit_clauses("ぎゅう", Some((1, 3))).is_empty()); // 1 は「ぎ」の途中
    }

    // ── apply_ime_input: winit Ime → focus 中 text-input への適用 ──────────────

    #[test]
    fn composing_shows_underlined_preedit_in_the_focused_input() {
        // criterion #1: 「ぎゅうにゅう」を compose すると preedit が下線付きで表示される。
        // 文節分割前は preedit 全体が単一の細線下線（変換前の見た目）。
        let (mut tree, input) = focused_input("");
        let mut buf = ImeBuffer::new();
        assert!(apply_ime_input(
            &mut tree,
            &Ime::Preedit("ぎゅうにゅう".into(), None),
            &mut buf,
        ));
        assert_eq!(tree.element_get_text_content(input), "ぎゅうにゅう");
        assert_eq!(
            tree.element_composition_underlines(input),
            vec![(0, "ぎゅうにゅう".len(), CompositionUnderline::Thin)],
        );
    }

    #[test]
    fn converting_distinguishes_the_active_clause_from_the_settled_tail() {
        // criterion #2: 変換すると active 文節（太線）と確定テール（細線）が区別される。
        // winit は変換中の active 文節を preedit 内の cursor 選択範囲として報告するので、
        // その範囲を Thick、前後を Thin にする。例「ぎゅうにゅう」で先頭「ぎゅう」を
        // 変換中（active）、残り「にゅう」は未変換テール。
        let (mut tree, input) = focused_input("");
        let mut buf = ImeBuffer::new();
        let active_end = "ぎゅう".len();
        let total = "ぎゅうにゅう".len();
        assert!(apply_ime_input(
            &mut tree,
            &Ime::Preedit("ぎゅうにゅう".into(), Some((0, active_end))),
            &mut buf,
        ));
        assert_eq!(
            tree.element_composition_underlines(input),
            vec![
                (0, active_end, CompositionUnderline::Thick),
                (active_end, total, CompositionUnderline::Thin),
            ],
        );
    }

    #[test]
    fn committing_lands_the_text_and_clears_the_preedit() {
        // criterion #3: commit すると確定文字列が text-input に入り preedit が消える。
        let (mut tree, input) = focused_input("");
        let mut buf = ImeBuffer::new();
        apply_ime_input(
            &mut tree,
            &Ime::Preedit("ぎゅうにゅう".into(), None),
            &mut buf,
        );
        assert!(apply_ime_input(
            &mut tree,
            &Ime::Commit("牛乳".into()),
            &mut buf
        ));
        assert_eq!(tree.element_get_text_content(input), "牛乳");
        assert!(
            tree.element_composition_underlines(input).is_empty(),
            "確定後は変換下線が残ってはならない",
        );
    }

    #[test]
    fn committing_at_a_mid_caret_lands_at_the_caret_not_the_tail() {
        // issue #563 の回帰: 確定テキスト "helloworld" の中央(5, hello|world)にキャレットを
        // 置き、winit の `Ime::Preedit("X")` → `Ime::Commit("X")` で変換確定すると、確定文字は
        // キャレット位置に入り（"helloworldX" ではなく "helloXworld"）、キャレットは挿入文字の
        // 直後(6)へ進む。増分コマンド経路（ADR-0117）をデスクトップ seam 越しに駆動する
        // 決定的テスト。
        let (mut tree, input) = focused_input("helloworld");
        tree.element_set_selection(input, 5, 5);
        let mut buf = ImeBuffer::new();

        // 変換中: preedit はキャレット位置に表示される（末尾ではない）。
        assert!(apply_ime_input(
            &mut tree,
            &Ime::Preedit("X".into(), None),
            &mut buf,
        ));
        assert_eq!(tree.element_get_text_content(input), "helloXworld");

        // 確定: 文字がキャレット位置に入り、preedit は消える。
        assert!(apply_ime_input(&mut tree, &Ime::Commit("X".into()), &mut buf));
        assert_eq!(
            tree.element_get_text_content(input),
            "helloXworld",
            "commit lands at the caret, not the tail",
        );
        assert_eq!(
            tree.element_caret_byte_index(input),
            Some(6),
            "caret sits right after the inserted character",
        );
        assert!(
            tree.element_composition_underlines(input).is_empty(),
            "確定後は変換下線が残ってはならない",
        );
    }

    #[test]
    fn ime_without_focus_is_a_no_op() {
        // focus 中の編集要素が無ければ何も適用しない（パニックしない）。
        let mut tree = ElementTree::new();
        let root = tree.element_create(1, ElementKind::View);
        tree.set_root(root);
        let mut buf = ImeBuffer::new();
        assert!(!apply_ime_input(
            &mut tree,
            &Ime::Preedit("ぎゅう".into(), None),
            &mut buf,
        ));
    }
}
