//! ソフトキーボード/IME の *絶対状態* → `EditState` 編集呼び出しの差分変換（ADR-0094）。
//!
//! 一部のプラットフォーム IME（Android GameActivity の GameTextInput 等）はテキスト
//! 入力を *絶対状態*（全バッファ＋任意の composing(preedit) 領域＋selection）として
//! 報告し、`hayate-core` が公開する離散的な構成デルタ（`on_composition_*`）ではない。
//! 本モジュールは連続する絶対状態を差分し、コアの「確定 `text_content` ＋キャレット
//! 位置の `preedit`」モデル（[`super::edit_state::EditState`]、ADR-0069）に合わせて最小の
//! コア編集呼び出し（[`ImeAction`]）へ変換する。selection を確定テキスト座標へ写像して
//! キャレットを更新するので、preedit/確定は末尾固定ではなくキャレット位置に入る。
//!
//! 変換はプラットフォーム非依存の純粋関数で、各アダプタはこのモジュールの型へ自身の
//! プラットフォーム IME 状態をマップするだけでよい（Android は android-activity の
//! `TextInputState`/`TextSpan`、将来の Web/iOS は各々の composition イベント）。
//! これにより差分・適用ロジックを一度だけ実装・テストし、全アダプタで共有する。

use super::tree::ElementTree;
use super::id::ElementId;

/// 報告テキストへのバイトオフセット範囲。オフセットは [`TextInputState::text`] への
/// UTF-8 バイトインデックス（android-activity の `TextSpan` 等に対応）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextSpan {
    pub start: usize,
    pub end: usize,
}

/// ソフトキーボード/IME の絶対テキスト状態。android-activity の `TextInputState` 等に
/// 対応。`selection` はキャレット/選択（`text` へのバイトオフセット）で、これを確定
/// テキスト座標へ写像してコアのキャレットを更新する — preedit/確定をキャレット位置に
/// 置くために必要（ADR-0069/0094: かつての「末尾キャレットのみ」前提を解消）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextInputState {
    pub text: String,
    /// composing/preedit 領域。構成中でなければ `None`。
    pub compose_region: Option<TextSpan>,
    /// キャレット/選択（`text` への UTF-8 バイトオフセット）。報告されない場合は `None`
    /// で、その場合は従来どおりキャレットを末尾に倒す。
    pub selection: Option<TextSpan>,
}

/// フォーカス中の TextInput に適用するコア編集呼び出し。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImeAction {
    /// 確定内容（composing 領域外のテキスト）を置換する。preedit も消す
    /// `ElementTree::element_set_text_content` に対応。
    SetText(String),
    /// キャレット/選択を確定テキスト座標（`anchor`/`focus` バイトオフセット）で設定する。
    /// `SetText` がキャレットを末尾へ倒すため、その後に正しい位置へ戻すのに使う。
    /// `ElementTree::element_set_selection` に対応。
    SetSelection { anchor: usize, focus: usize },
    /// アクティブな preedit（composing 領域）を置換する。空なら消す。
    /// `ElementTree::element_set_preedit` に対応。
    SetPreedit(String),
    /// 確定文字列をキャレット位置に挿入して確定する（増分コマンド経路、ADR-0117）。
    /// アクティブな preedit があれば確定文字列で置換してから確定する。末尾連結ではなく
    /// 現在のキャレット位置に入り、キャレットは挿入文字の直後へ進む
    /// （`EditState::finish_composition` に対応）。
    CommitText(String),
    /// キャレット直前の 1 グラフェムを削除する（増分コマンド経路の確定テキスト
    /// backspace）。`ElementTree::element_delete_backward` に対応。
    DeleteBackward,
}

/// 絶対状態を (確定テキスト, preedit, キャレット/選択) に分割する。確定テキストは
/// composing 領域外すべて、preedit は composing 部分文字列。`selection` は確定テキスト
/// 座標へ写像する（compose 領域分を差し引く）。compose 領域が無い、または不正
/// （範囲外 / 非 char 境界）なら「構成なし」として扱う。
fn decompose(state: &TextInputState) -> (String, String, Option<(usize, usize)>) {
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
            let sel = map_selection(state, |o| map_offset_excluding(o, span.start, span.end));
            return (committed, preedit, sel);
        }
    }
    let sel = map_selection(state, |o| o);
    (state.text.clone(), String::new(), sel)
}

/// 絶対 `text` 座標のオフセットを確定テキスト座標へ写像する。compose 領域 [cs, ce) は
/// 確定テキストから取り除かれるので、領域内は領域先頭 `cs` に丸め、領域より後ろは
/// その長さ分だけ前へ詰める。
fn map_offset_excluding(offset: usize, cs: usize, ce: usize) -> usize {
    if offset <= cs {
        offset
    } else if offset <= ce {
        cs
    } else {
        offset - (ce - cs)
    }
}

/// 報告された selection（あれば）を、確定テキスト座標へ写像した `(anchor, focus)` にする。
fn map_selection(
    state: &TextInputState,
    map: impl Fn(usize) -> usize,
) -> Option<(usize, usize)> {
    let span = state.selection?;
    let len = state.text.len();
    if span.start > len
        || span.end > len
        || !state.text.is_char_boundary(span.start)
        || !state.text.is_char_boundary(span.end)
    {
        return None;
    }
    Some((map(span.start), map(span.end)))
}

/// 2 つの絶対 IME 状態を差分し、最小のコア編集呼び出しにする。
pub fn translate_text_input(prev: &TextInputState, next: &TextInputState) -> Vec<ImeAction> {
    let (prev_committed, prev_preedit, prev_sel) = decompose(prev);
    let (committed, preedit, sel) = decompose(next);

    let mut actions = Vec::new();
    let set_text = committed != prev_committed;
    if set_text {
        actions.push(ImeAction::SetText(committed));
    }
    // `SetText` はキャレットを末尾へ倒すので、selection が分かっていれば常に戻す。
    // selection 単独の変化（キャレット移動）でも反映する。
    if let Some((anchor, focus)) = sel {
        if set_text || Some((anchor, focus)) != prev_sel {
            actions.push(ImeAction::SetSelection { anchor, focus });
        }
    }
    // `SetText` は preedit を消すので、確定テキストを設定しつつ構成が継続中の場合、
    // または preedit 自体が変わった場合は preedit を再設定する。preedit はコアの
    // キャレット位置（直前の `SetSelection` で確定済み）に表示される。
    if preedit != prev_preedit || (set_text && !preedit.is_empty()) {
        actions.push(ImeAction::SetPreedit(preedit));
    }
    actions
}

/// 変換済みアクション 1 つをフォーカス中の TextInput に適用する。TextInput 以外の
/// ターゲットでは no-op（コアのセッターが要素の編集状態でガードする）。
pub fn apply_ime_action(tree: &mut ElementTree, target: ElementId, action: &ImeAction) {
    match action {
        ImeAction::SetText(text) => tree.element_set_text_content(target, text),
        ImeAction::SetSelection { anchor, focus } => {
            tree.element_set_selection(target, *anchor, *focus)
        }
        ImeAction::SetPreedit(preedit) => tree.element_set_preedit(target, preedit),
        ImeAction::CommitText(text) => tree.element_finish_composition(target, text),
        ImeAction::DeleteBackward => tree.element_delete_backward(target),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::kind::ElementKind;

    fn composing(text: &str, start: usize, end: usize) -> TextInputState {
        TextInputState {
            text: text.to_string(),
            compose_region: Some(TextSpan { start, end }),
            selection: None,
        }
    }

    fn committed(text: &str) -> TextInputState {
        TextInputState {
            text: text.to_string(),
            compose_region: None,
            selection: None,
        }
    }

    /// 選択（キャレット）付きの絶対状態。`sel` は `text` への (start, end) バイト範囲。
    fn with_selection(mut state: TextInputState, start: usize, end: usize) -> TextInputState {
        state.selection = Some(TextSpan { start, end });
        state
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

    #[test]
    fn mid_text_caret_emits_set_selection_in_committed_coords() {
        // "helloworld" のキャレットが 5（hello|world）。selection を確定テキスト座標で
        // 反映する。確定テキストは不変なので SetSelection のみ。
        let prev = with_selection(committed("helloworld"), 10, 10);
        let next = with_selection(committed("helloworld"), 5, 5);
        let actions = translate_text_input(&prev, &next);
        assert_eq!(actions, vec![ImeAction::SetSelection { anchor: 5, focus: 5 }]);
    }

    #[test]
    fn composing_at_mid_caret_maps_preedit_position_off_the_tail() {
        // 確定 "helloworld"、キャレットは末尾(10)から中央(5)へ移って "X" を構成中。
        // 絶対 text は "helloXworld"、compose 領域 [5,6)、selection は compose 末尾(6)。
        // 確定テキストは "helloworld" のままで、selection(6) は確定座標 5 に写像され、
        // SetSelection でキャレットを 5 に戻してから preedit をそこに入れる。
        let prev = with_selection(committed("helloworld"), 10, 10);
        let next = with_selection(composing("helloXworld", 5, 6), 6, 6);
        let actions = translate_text_input(&prev, &next);
        assert_eq!(
            actions,
            vec![
                ImeAction::SetSelection { anchor: 5, focus: 5 },
                ImeAction::SetPreedit("X".into()),
            ],
            "preedit lands at the mid caret, not the tail",
        );
    }

    // コアに対するエンドツーエンド: 中央キャレットで構成→確定すると、文字がキャレット
    // 位置に入る（末尾に飛ばない）こと。ユーザー報告バグの回帰テスト。
    #[test]
    fn composition_at_mid_caret_commits_at_the_caret_not_the_tail() {
        let mut tree = ElementTree::new();
        let input = tree.element_create(1, ElementKind::TextInput);
        tree.set_root(input);
        tree.element_focus(input);
        let target = ElementId::from_u64(1);

        // 既存値を確定し、キャレットを中央(5)へ。
        let mut prev = TextInputState::default();
        for state in [
            with_selection(committed("helloworld"), 10, 10),
            with_selection(committed("helloworld"), 5, 5),
        ] {
            for action in translate_text_input(&prev, &state) {
                apply_ime_action(&mut tree, target, &action);
            }
            prev = state;
        }
        assert_eq!(tree.element_caret_byte_index(target), Some(5));

        // 中央で "X" を構成 → 確定。`element_get_text_content` は表示テキスト
        // （text_content + 有効 preedit）を返す。
        for state in [
            with_selection(composing("helloXworld", 5, 6), 6, 6),
            with_selection(committed("helloXworld"), 6, 6),
        ] {
            for action in translate_text_input(&prev, &state) {
                apply_ime_action(&mut tree, target, &action);
            }
            assert_eq!(
                tree.element_get_text_content(target),
                "helloXworld",
                "display reconstructs the buffer with the char at the caret",
            );
            prev = state;
        }
        assert_eq!(tree.element_caret_byte_index(target), Some(6));
    }

    // コアに対するエンドツーエンド: 日本語の構成＋確定で、TextInput の display_text が
    // 報告バッファと一致すること。
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
