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
//! 両入力モデルは共通の出力（[`ImeAction`]、コアの「確定 `text_content` ＋キャレット位置の
//! `preedit`」モデル ADR-0069 への 1:1 写像）へ合流する。各 leaf には native callback →
//! [`ImeCommand`]（iOS）/ native buffer → 絶対状態（Android）の glue だけが残り、編集
//! セマンティクスは持たない。実機 SDK や DOM/wasm を要さず全ターゲットでコンパイル/テスト
//! できる純粋関数。
//!
//! 確定/preedit は末尾固定ではなく**キャレット位置**へ入る（issue #563）。コアの `EditState`
//! がキャレットの正本（ポインタクリックで中央に置かれても保たれる）なので、本モジュールは
//! 確定済みテキストの平坦なミラーを持たず、キャレット相対の [`ImeAction::CommitText`] /
//! [`ImeAction::DeleteBackward`] / [`ImeAction::SetPreedit`] を出して `EditState` の
//! キャレット対応編集（web 経路と同じ `insert`/`finish_composition`）へ合流する。

use super::ime_reconcile::ImeAction;

/// UITextInput が push する増分コマンド。`objc2`/UIKit 型に依存せず UIKit の
/// テキスト入力コールバックを写す。`selectedRange` は省略（キャレット位置の正本はコアの
/// `EditState` が持ち、確定/preedit はそのキャレット位置へ入る。Android が GameTextInput の
/// selection を `TextInputState` 経由でコア座標へ写すのと同じく、こちらはコア側キャレットを
/// 使う）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImeCommand {
    /// `insertText:` — marked text があればそれを置換して確定する（IME の候補確定は
    /// marked 文字列を最終確定文字列で `insertText` する）。marked が無ければキャレット位置に
    /// 挿入する。
    Insert(String),
    /// `deleteBackward` — marked text があればその末尾 1 文字、無ければ確定テキストの
    /// キャレット直前 1 文字を削除する。
    DeleteBackward,
    /// `setMarkedText:selectedRange:` — preedit（marked text）を置換する。
    SetMarked(String),
    /// `unmarkText` — marked text を確定する。
    Unmark,
}

/// 増分入力モデルが保持する UITextInput のローカルバッファ。確定テキストの正本はコアの
/// [`EditState`](super::edit_state::EditState)（ADR-0069）が持つ — 増分経路は確定/preedit を
/// **キャレット位置**へ写すため（issue #563）、確定済みテキストの平坦なミラーは持たない。
/// バッファに残るのはアクティブな marked text（preedit）だけで、`deleteBackward`（変換中の
/// バックスペース）が marked 末尾を削り、差分で `SetPreedit` を出すために必要。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImeBuffer {
    pub preedit: String,
}

impl ImeBuffer {
    pub fn new() -> Self {
        Self::default()
    }
}

/// 増分コマンド 1 つをローカルバッファに適用し、必要な最小のコア編集呼び出しを返す。
///
/// 確定（`Insert` / `Unmark`）は末尾連結＋全文 `SetText` ではなく、キャレット位置へ確定する
/// [`ImeAction::CommitText`] を出す。コアの `EditState` がキャレットの正本なので（ポインタ
/// クリックで中央に置かれても保たれる）、確定/preedit は末尾固定にならずキャレット位置に
/// 入る（web 経路の `on_text_input`/`on_composition_end` と同じ振る舞いに合流、ADR-0117）。
pub fn apply_command(buf: &mut ImeBuffer, command: ImeCommand) -> Vec<ImeAction> {
    match command {
        ImeCommand::Insert(text) => {
            // `insertText:` は marked text を確定文字列で置換して確定する（候補確定の経路）。
            // marked が無ければキャレット位置へ挿入する。いずれもキャレット位置に入り
            // preedit は消える（`EditState::finish_composition` と同型）。
            let had_preedit = !buf.preedit.is_empty();
            buf.preedit.clear();
            if text.is_empty() {
                // 空文字 insert は確定対象が無い。marked があったならそれだけ消す。
                return if had_preedit {
                    vec![ImeAction::SetPreedit(String::new())]
                } else {
                    Vec::new()
                };
            }
            vec![ImeAction::CommitText(text)]
        }
        ImeCommand::DeleteBackward => {
            if buf.preedit.is_empty() {
                // marked が無ければ確定テキストのキャレット直前 1 文字を削る。
                vec![ImeAction::DeleteBackward]
            } else {
                // 変換中のバックスペース: marked 末尾 1 char を削り preedit を張り直す。
                pop_last_char(&mut buf.preedit);
                vec![ImeAction::SetPreedit(buf.preedit.clone())]
            }
        }
        ImeCommand::SetMarked(text) => {
            // preedit（marked text）を置換する。変化が無ければ何も出さない。
            if text == buf.preedit {
                return Vec::new();
            }
            buf.preedit = text.clone();
            vec![ImeAction::SetPreedit(text)]
        }
        ImeCommand::Unmark => {
            // marked text をその位置（キャレット）で確定する（insert を伴わない確定経路）。
            if buf.preedit.is_empty() {
                return Vec::new();
            }
            vec![ImeAction::CommitText(std::mem::take(&mut buf.preedit))]
        }
    }
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
    fn inserting_a_committed_character_commits_at_the_caret() {
        let mut buf = ImeBuffer::new();
        assert_eq!(
            apply_command(&mut buf, ImeCommand::Insert("a".into())),
            vec![ImeAction::CommitText("a".into())]
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

    // 変換確定: marked "かん" を "感" で insert すると、marked を "感" で置換してキャレット
    // 位置に確定する（"かん感" にはならない）。確定は `CommitText` でキャレット位置へ入り、
    // preedit はバッファからもクリアされる。
    #[test]
    fn committing_marked_text_via_insert_commits_text_and_clears_preedit() {
        let mut buf = ImeBuffer::new();
        apply_command(&mut buf, ImeCommand::SetMarked("かん".into()));
        // IME は通常、確定文字列をそのまま insert する。
        let actions = apply_command(&mut buf, ImeCommand::Insert("感".into()));
        assert_eq!(actions, vec![ImeAction::CommitText("感".into())]);
        assert!(buf.preedit.is_empty());
    }

    // unmarkText も marked text をその位置（キャレット）で確定する（insert を伴わない確定経路）。
    #[test]
    fn unmark_commits_marked_text() {
        let mut buf = ImeBuffer::new();
        apply_command(&mut buf, ImeCommand::SetMarked("かん".into()));
        let actions = apply_command(&mut buf, ImeCommand::Unmark);
        assert_eq!(actions, vec![ImeAction::CommitText("かん".into())]);
        assert!(buf.preedit.is_empty());
    }

    // 確定済みテキストの後ろで marked を始めても、確定テキストの正本（EditState）には触れず
    // preedit を出すだけ。バッファは marked のみ追跡する。
    #[test]
    fn marking_after_a_commit_only_emits_preedit() {
        let mut buf = ImeBuffer::new();
        apply_command(&mut buf, ImeCommand::Insert("abc".into()));
        assert_eq!(
            apply_command(&mut buf, ImeCommand::SetMarked("か".into())),
            vec![ImeAction::SetPreedit("か".into())]
        );
        assert_eq!(buf.preedit, "か");
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

    // marked が無ければ deleteBackward は確定テキストのキャレット直前 1 char を削る
    // （`DeleteBackward` アクション）。確定テキストの正本は EditState なので、削除は
    // コア側でキャレット対応に解決される（マルチバイトでも char 境界を割らない）。
    #[test]
    fn delete_backward_emits_committed_delete_when_unmarked() {
        let mut buf = ImeBuffer::new();
        apply_command(&mut buf, ImeCommand::Insert("あい".into()));
        assert_eq!(
            apply_command(&mut buf, ImeCommand::DeleteBackward),
            vec![ImeAction::DeleteBackward]
        );
    }

    #[test]
    fn no_op_command_emits_nothing() {
        let mut buf = ImeBuffer::new();
        // marked 無しで unmark は何も変えない。
        assert!(apply_command(&mut buf, ImeCommand::Unmark).is_empty());
        // 空文字 insert も何も変えない。
        assert!(apply_command(&mut buf, ImeCommand::Insert(String::new())).is_empty());
    }

    // ユーザー報告バグの回帰（issue #563）: 確定テキスト中央にキャレットがある状態で
    // IME 変換確定（`Insert`）を行うと、確定文字はキャレット位置に入り（末尾に飛ばない）、
    // キャレットは挿入文字の直後に来る。絶対状態経路の
    // `composition_at_mid_caret_commits_at_the_caret_not_the_tail` と同じ契約を、増分
    // コマンド経路で検証する。iOS も同じ `apply_command`/`apply_ime_action` 経路を共有
    // するため、本テストが通ればコア修正で iOS も同時に解消される（ADR-0117）。
    #[test]
    fn committing_at_mid_caret_lands_at_the_caret_not_the_tail() {
        let mut tree = ElementTree::new();
        let input = tree.element_create(1, ElementKind::TextInput);
        tree.set_root(input);
        tree.element_focus(input);
        let target = ElementId::from_u64(1);

        // 既存値 "helloworld" を確定し、キャレットを中央(5)へ（hello|world）。
        tree.element_append_text_content(input, "helloworld");
        tree.element_set_selection(input, 5, 5);

        let mut buf = ImeBuffer::new();
        for action in run(&mut buf, [ImeCommand::Insert("X".into())]) {
            apply_ime_action(&mut tree, target, &action);
        }

        assert_eq!(
            tree.element_get_text_content(target),
            "helloXworld",
            "commit lands at the caret, not the tail",
        );
        assert_eq!(
            tree.element_caret_byte_index(target),
            Some(6),
            "caret sits right after the inserted character",
        );
    }

    // criterion #2（issue #563）: 変換中（preedit）の表示位置もキャレット位置（中央）に
    // なる。確定テキスト中央のキャレットで marked を出すと、display_text は preedit を
    // キャレット位置に挿入して見せる（末尾ではない）。
    #[test]
    fn marking_at_mid_caret_shows_preedit_at_the_caret_not_the_tail() {
        let mut tree = ElementTree::new();
        let input = tree.element_create(1, ElementKind::TextInput);
        tree.set_root(input);
        tree.element_focus(input);
        let target = ElementId::from_u64(1);

        tree.element_append_text_content(input, "helloworld");
        tree.element_set_selection(input, 5, 5); // hello|world

        let mut buf = ImeBuffer::new();
        for action in run(&mut buf, [ImeCommand::SetMarked("X".into())]) {
            apply_ime_action(&mut tree, target, &action);
        }

        assert_eq!(
            tree.element_get_text_content(target),
            "helloXworld",
            "preedit shows at the caret, not the tail",
        );
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
