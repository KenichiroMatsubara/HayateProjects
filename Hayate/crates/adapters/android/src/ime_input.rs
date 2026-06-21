//! Android GameTextInput → `hayate-core` の IME 変換（ADR-0094）。
//!
//! GameActivity の GameTextInput はテキスト入力を *絶対状態*（全バッファ＋任意の
//! composing(preedit) 領域）として報告する。`hayate-core` が公開する離散的な
//! 構成デルタ（`on_composition_*`）ではない。本モジュールは連続する状態を差分し、
//! コアの「確定 `text_content` ＋末尾 `preedit`」モデル（`EditState`、ADR-0069）に
//! 合わせて最小のコア編集呼び出しに変換する。`touch_input` と同様に
//! `android-activity`/`ndk` 型に依存しないため、変換とツリーへの適用を NDK 無しで
//! ホストテストできる。`app.rs` がプラットフォーム状態を読む薄いグルー。

#[cfg(any(target_os = "android", test))]
use hayate_core::{ElementId, ElementTree};

/// 報告テキストへのバイトオフセット範囲。android-activity の `TextSpan` に対応し、
/// オフセットは [`TextInputState::text`] への UTF-8 バイトインデックス。
#[cfg(any(target_os = "android", test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextSpan {
    pub start: usize,
    pub end: usize,
}

/// ソフトキーボードの絶対テキスト状態。android-activity の `TextInputState` に
/// 対応（selection は省略。コアは末尾キャレットのみ追跡する）。
#[cfg(any(target_os = "android", test))]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextInputState {
    pub text: String,
    /// composing/preedit 領域。構成中でなければ `None`。
    pub compose_region: Option<TextSpan>,
}

/// フォーカス中の TextInput に適用するコア編集呼び出し。
#[cfg(any(target_os = "android", test))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImeAction {
    /// 確定内容（composing 領域外のテキスト）を置換する。preedit も消す
    /// `ElementTree::element_set_text_content` に対応。
    SetText(String),
    /// アクティブな preedit（composing 領域）を置換する。空なら消す。
    /// `ElementTree::element_set_preedit` に対応。
    SetPreedit(String),
}

/// 絶対状態を (確定テキスト, preedit) に分割する。確定テキストは composing 領域外
/// すべて、preedit は composing 部分文字列。compose 領域が無い、または不正
/// （範囲外 / 非 char 境界）なら「構成なし」として扱う。
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

/// 2 つの GameTextInput 状態を差分し、最小のコア編集呼び出しにする。
#[cfg(any(target_os = "android", test))]
pub fn translate_text_input(prev: &TextInputState, next: &TextInputState) -> Vec<ImeAction> {
    let (prev_committed, prev_preedit) = decompose(prev);
    let (committed, preedit) = decompose(next);

    let mut actions = Vec::new();
    let set_text = committed != prev_committed;
    if set_text {
        actions.push(ImeAction::SetText(committed));
    }
    // `SetText` は preedit を消すので、確定テキストを設定しつつ構成が継続中の場合、
    // または preedit 自体が変わった場合は preedit を再設定する。
    if preedit != prev_preedit || (set_text && !preedit.is_empty()) {
        actions.push(ImeAction::SetPreedit(preedit));
    }
    actions
}

/// 変換済みアクション 1 つをフォーカス中の TextInput に適用する。TextInput 以外の
/// ターゲットでは no-op（コアのセッターが要素の編集状態でガードする）。
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
        // "か" は UTF-8 で 3 バイト。確定テキストは不変（空）。
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
        // "abc" は確定済み、キャレット（末尾）で "か" を構成中。
        let actions = translate_text_input(&committed("abc"), &composing("abcか", 3, 6));
        assert_eq!(actions, vec![ImeAction::SetPreedit("か".into())]);
    }

    #[test]
    fn editing_committed_prefix_while_composing_reasserts_preedit() {
        // "か" を構成中のまま確定プレフィックスが "a"->"b" に変わる。SetText が
        // preedit を消すため再設定が必要。
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
        // 範囲外の end は「構成なし」として扱う。
        let actions = translate_text_input(&TextInputState::default(), &composing("x", 0, 99));
        assert_eq!(actions, vec![ImeAction::SetText("x".into())]);
    }

    // コアに対するエンドツーエンド: 日本語の構成＋確定で、TextInput の display_text が
    // GameTextInput の報告バッファと一致すること。
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
