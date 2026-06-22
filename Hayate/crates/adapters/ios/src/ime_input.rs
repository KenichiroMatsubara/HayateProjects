//! iOS UITextInput → `hayate-core` の IME 変換（ADR-0114）。
//!
//! ここが Android アダプタとの唯一の本質的な相違点。**Android GameTextInput** は入力を
//! *絶対状態*（全バッファ + 任意の composing 領域）として報告し、アダプタが連続状態を
//! 差分する。これに対し **iOS UITextInput** はアダプタが*実装するプロトコル*で、UIKit が
//! *増分コールバック*を push してくる（`insertText:` / `deleteBackward` /
//! `setMarkedText:selectedRange:` / `unmarkText`）。フレームごとに全文を読み出すモデルでは
//! なく、アダプタ側がバッファの真実を保持する。
//!
//! そこで、出力側（`ImeAction` / `apply_ime_action`、コアの「確定 text_content + 末尾
//! preedit」モデル ADR-0069 への 1:1 写像）は **Core 所有の `ime_reconcile` を再利用**する
//! （main の C4 で IME 変換は Core 所有になった）。入力側は増分コマンドを小さなローカル
//! バッファ（[`ImeBuffer`]）に畳んで最小のコア編集呼び出しに変換する iOS 固有の実装にする。
//! Core の `ime_reconcile` は絶対状態（Android GameTextInput）モデルのみで、UITextInput の
//! 増分経路を持たないため、この入力半分はこのアダプタに残す（将来 `apply_command` 自体も
//! Core へ寄せるのは「Core 所有」方針の続きで、別ブランチの再構築で扱う）。`touch_input` /
//! `surface_lifecycle` と同様 objc2/UIKit 型に依存しない純粋関数なので、変換とツリーへの
//! 適用を Mac/SDK 無しでホストテストできる。グルー（`app.rs`）が UITextInput コールバックを
//! [`ImeCommand`] に写す薄い層を担う。

// 出力半分は Core 所有の ime_reconcile を再利用する（`hayate-adapter-android` の app.rs と
// 同じ import 元）。iOS 固有の増分入力モデル（ImeCommand / ImeBuffer / apply_command）だけが
// このアダプタに残る。
#[cfg(any(target_os = "ios", test))]
pub use hayate_core::element::ime_reconcile::{apply_ime_action, ImeAction};

/// UITextInput が push する増分コマンド。`objc2`/UIKit 型に依存せず UIKit の
/// テキスト入力コールバックを写す。`selectedRange` は省略（コアは末尾キャレットのみ
/// 追跡する。Android が GameTextInput の selection を落とすのと同じ簡略化）。
#[cfg(any(target_os = "ios", test))]
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

/// アダプタが保持する UITextInput のローカルバッファ。確定テキストと、アクティブな
/// marked text（preedit）に分かれる。コアの `EditState`（ADR-0069）と同じ二分割。
#[cfg(any(target_os = "ios", test))]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImeBuffer {
    pub committed: String,
    pub preedit: String,
}

#[cfg(any(target_os = "ios", test))]
impl ImeBuffer {
    pub fn new() -> Self {
        Self::default()
    }
}

/// 増分コマンド 1 つをローカルバッファに適用し、必要な最小のコア編集呼び出しを返す。
///
/// `SetText` はコア側で preedit を消すため、確定テキストを設定しつつ marked が継続して
/// いる、または marked 自体が変わった場合は preedit を再設定する（Android の
/// `translate_text_input` が持つ「SetText は preedit を消すので再表明」不変条件と同型）。
#[cfg(any(target_os = "ios", test))]
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
#[cfg(any(target_os = "ios", test))]
fn pop_last_char(s: &mut String) {
    if let Some(ch) = s.chars().next_back() {
        let new_len = s.len() - ch.len_utf8();
        s.truncate(new_len);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hayate_core::{ElementId, ElementKind, ElementTree};

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
    // text_content が UITextInput のローカルバッファと一致すること。Android アダプタの
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
