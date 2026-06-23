//! IME の *増分 command* 入力モデル → `EditState` 編集呼び出しの畳み込み（ADR-0117 フェーズ1）。
//!
//! `hayate-core` は二つの platform IME 入力モデルを所有する。一方は *絶対状態* モデル
//! （Android GameTextInput が全バッファ＋任意の composing 領域を報告 → 差分、ADR-0094）で
//! 隣の [`super::ime_reconcile`] が持つ。本モジュールはもう一方の *増分 command* モデルを持つ:
//! iOS UITextInput はアダプタが*実装するプロトコル*で、UIKit が増分コールバック
//! （`insertText:` / `deleteBackward` / `setMarkedText:selectedRange:` / `unmarkText`）を
//! push してくる。フレームごとに全文を読むのではなく、増分コマンド（[`ImeCommand`]）を
//! 小さなローカルバッファ（[`ImeBuffer`]）に畳んで最小のコア編集へ変換する。
//!
//! 両入力モデルは共通の出力（[`ImeAction`]、コアの「確定 `text_content` ＋末尾 `preedit`」
//! モデル ADR-0069 への 1:1 写像）へ合流する。各 leaf には native callback → [`ImeCommand`]
//! （iOS）/ native buffer → 絶対状態（Android）の glue だけが残り、編集セマンティクスは
//! 持たない。実機 SDK や DOM/wasm を要さず全ターゲットでコンパイル/テストできる純粋関数。

use super::ime_reconcile::ImeAction;

/// UITextInput が push する増分コマンド。`objc2`/UIKit 型に依存せず UIKit の
/// テキスト入力コールバックを写す。`selectedRange` は省略（コアは末尾キャレットのみ
/// 追跡する。Android が GameTextInput の selection を落とすのと同じ簡略化）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImeCommand {
    /// `insertText:` — marked text があればそれを置換して確定する（IME の候補確定は
    /// marked 文字列を最終確定文字列で `insertText` する）。marked が無ければキャレット末尾に
    /// 追記する。
    Insert(String),
    /// `deleteBackward` — marked text があればその末尾 1 文字、無ければ確定テキストの
    /// 末尾 1 文字を削除する。
    DeleteBackward,
    /// `setMarkedText:selectedRange:` — preedit（marked text）を置換する。
    SetMarked(String),
    /// `unmarkText` — marked text を確定する。
    Unmark,
}

/// 増分入力モデルが保持する UITextInput のローカルバッファ。確定テキストと、アクティブな
/// marked text（preedit）に分かれる。コアの `EditState`（ADR-0069）と同じ二分割。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImeBuffer {
    pub committed: String,
    pub preedit: String,
}

impl ImeBuffer {
    pub fn new() -> Self {
        Self::default()
    }
}

/// 増分コマンド 1 つをローカルバッファに適用し、必要な最小のコア編集呼び出しを返す。
///
/// `SetText` はコア側で preedit を消すため、確定テキストを設定しつつ marked が継続して
/// いる、または marked 自体が変わった場合は preedit を再設定する（絶対状態モデルの
/// `translate_text_input` が持つ「SetText は preedit を消すので再表明」不変条件と同型）。
pub fn apply_command(buf: &mut ImeBuffer, command: ImeCommand) -> Vec<ImeAction> {
    let prev_committed = buf.committed.clone();
    let prev_preedit = buf.preedit.clone();

    match command {
        ImeCommand::Insert(text) => {
            // `insertText:` は marked text を置換して確定する（候補確定の経路）。marked が
            // 無ければ単にキャレット末尾へ追記する。いずれも preedit は消える。
            buf.preedit.clear();
            buf.committed.push_str(&text);
        }
        ImeCommand::DeleteBackward => {
            if !buf.preedit.is_empty() {
                pop_last_char(&mut buf.preedit);
            } else {
                pop_last_char(&mut buf.committed);
            }
        }
        ImeCommand::SetMarked(text) => {
            buf.preedit = text;
        }
        ImeCommand::Unmark => {
            buf.committed.push_str(&std::mem::take(&mut buf.preedit));
        }
    }

    let mut actions = Vec::new();
    let set_text = buf.committed != prev_committed;
    if set_text {
        actions.push(ImeAction::SetText(buf.committed.clone()));
    }
    if buf.preedit != prev_preedit || (set_text && !buf.preedit.is_empty()) {
        actions.push(ImeAction::SetPreedit(buf.preedit.clone()));
    }
    actions
}

/// 文字列の末尾 1 文字（UTF-8 char 単位）を取り除く。マルチバイト（日本語等）でも
/// char 境界を割らない。
fn pop_last_char(s: &mut String) {
    if let Some(ch) = s.chars().next_back() {
        let new_len = s.len() - ch.len_utf8();
        s.truncate(new_len);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::ime_reconcile::apply_ime_action;
    use crate::{ElementId, ElementKind, ElementTree};

    fn run(buf: &mut ImeBuffer, cmds: impl IntoIterator<Item = ImeCommand>) -> Vec<ImeAction> {
        cmds.into_iter()
            .flat_map(|c| apply_command(buf, c))
            .collect()
    }

    #[test]
    fn inserting_a_committed_character_sets_text() {
        let mut buf = ImeBuffer::new();
        assert_eq!(
            apply_command(&mut buf, ImeCommand::Insert("a".into())),
            vec![ImeAction::SetText("a".into())]
        );
    }

    #[test]
    fn starting_marked_text_only_sets_preedit() {
        let mut buf = ImeBuffer::new();
        assert_eq!(
            apply_command(&mut buf, ImeCommand::SetMarked("か".into())),
            vec![ImeAction::SetPreedit("か".into())]
        );
    }

    #[test]
    fn updating_marked_text_replaces_preedit() {
        let mut buf = ImeBuffer::new();
        apply_command(&mut buf, ImeCommand::SetMarked("か".into()));
        assert_eq!(
            apply_command(&mut buf, ImeCommand::SetMarked("かん".into())),
            vec![ImeAction::SetPreedit("かん".into())]
        );
    }

    // 変換確定: marked "かん" を "感" で insert すると、marked を "感" で置換して確定する
    // （"かん感" にはならない）。SetText が preedit を消すので空 preedit を再表明する。
    #[test]
    fn committing_marked_text_via_insert_sets_text_and_clears_preedit() {
        let mut buf = ImeBuffer::new();
        apply_command(&mut buf, ImeCommand::SetMarked("かん".into()));
        // IME は通常、確定文字列をそのまま insert する。
        let actions = apply_command(&mut buf, ImeCommand::Insert("感".into()));
        assert_eq!(
            actions,
            vec![
                ImeAction::SetText("感".into()),
                ImeAction::SetPreedit(String::new())
            ]
        );
        assert_eq!(buf.committed, "感");
        assert!(buf.preedit.is_empty());
    }

    // unmarkText も marked text を確定する（insert を伴わない確定経路）。
    #[test]
    fn unmark_commits_marked_text() {
        let mut buf = ImeBuffer::new();
        apply_command(&mut buf, ImeCommand::SetMarked("かん".into()));
        let actions = apply_command(&mut buf, ImeCommand::Unmark);
        assert_eq!(
            actions,
            vec![
                ImeAction::SetText("かん".into()),
                ImeAction::SetPreedit(String::new())
            ]
        );
    }

    // 確定プレフィックスを保ったまま marked を始める。
    #[test]
    fn marking_after_committed_prefix_preserves_committed() {
        let mut buf = ImeBuffer::new();
        apply_command(&mut buf, ImeCommand::Insert("abc".into()));
        assert_eq!(
            apply_command(&mut buf, ImeCommand::SetMarked("か".into())),
            vec![ImeAction::SetPreedit("か".into())]
        );
        assert_eq!(buf.committed, "abc");
    }

    // deleteBackward は marked があればその末尾を削る（変換中のバックスペース）。
    #[test]
    fn delete_backward_pops_marked_text_first() {
        let mut buf = ImeBuffer::new();
        apply_command(&mut buf, ImeCommand::SetMarked("かん".into()));
        assert_eq!(
            apply_command(&mut buf, ImeCommand::DeleteBackward),
            vec![ImeAction::SetPreedit("か".into())]
        );
    }

    // marked が無ければ deleteBackward は確定テキストの末尾 char を削る。マルチバイトでも
    // char 境界を割らない。
    #[test]
    fn delete_backward_pops_committed_char_when_unmarked() {
        let mut buf = ImeBuffer::new();
        apply_command(&mut buf, ImeCommand::Insert("あい".into()));
        assert_eq!(
            apply_command(&mut buf, ImeCommand::DeleteBackward),
            vec![ImeAction::SetText("あ".into())]
        );
        assert_eq!(buf.committed, "あ");
    }

    #[test]
    fn no_op_command_emits_nothing() {
        let mut buf = ImeBuffer::new();
        // marked 無しで unmark は何も変えない。
        assert!(apply_command(&mut buf, ImeCommand::Unmark).is_empty());
        // 空文字 insert も何も変えない。
        assert!(apply_command(&mut buf, ImeCommand::Insert(String::new())).is_empty());
    }

    // コアに対するエンドツーエンド: 日本語の変換（marked → 確定）で、TextInput の
    // text_content が UITextInput のローカルバッファと一致すること。絶対状態モデルの
    // `translation_drives_core_display_text_through_a_composition` と同じ契約を、異なる
    // フロントエンドモデル（増分コマンド）で検証する。
    #[test]
    fn commands_drive_core_display_text_through_a_composition() {
        let mut tree = ElementTree::new();
        let input = tree.element_create(1, ElementKind::TextInput);
        tree.set_root(input);
        tree.element_focus(input);
        let target = ElementId::from_u64(1);

        let mut buf = ImeBuffer::new();
        for (cmds, expected) in [
            (vec![ImeCommand::SetMarked("か".into())], "か"),
            (vec![ImeCommand::SetMarked("かん".into())], "かん"),
            (vec![ImeCommand::Insert("感".into())], "感"),
        ] {
            for action in run(&mut buf, cmds) {
                apply_ime_action(&mut tree, target, &action);
            }
            assert_eq!(tree.element_get_text_content(target), expected);
        }
    }
}
