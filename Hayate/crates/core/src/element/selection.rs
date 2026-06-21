//! 統一テキスト選択モデル（ADR-0097）。
//!
//! `Selection` は `ElementTree` が所有し、ドキュメント全体で常に高々1つだけが有効。
//! 両端点は `(ElementId, バイトオフセット)` の組。`anchor == focus` の退化した選択は
//! キャレットで、`EditState::cursor_byte_index` を包含する（ADR-0069）。

use crate::element::id::ElementId;

/// ポインタ/キー入力の `modifiers: u32` が運ぶ修飾キーのビットフラグ。
/// ワイヤ上の `MODIFIER_*` 契約（proto/spec）に一致: SHIFT=1, CTRL=2, ALT=4, META=8。
pub const MOD_SHIFT: u32 = 1;
pub const MOD_CTRL: u32 = 2;
pub const MOD_ALT: u32 = 4;
pub const MOD_META: u32 = 8;

/// 主コマンド修飾キー（Windows/Linux は Ctrl、macOS は Cmd=Meta）。
/// 全選択などのコードに使う。
pub const MOD_PRIMARY: u32 = MOD_CTRL | MOD_META;

/// 選択の一端: 特定要素のテキスト内のバイトオフセット。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelectionPoint {
    pub element: ElementId,
    pub offset: usize,
}

impl SelectionPoint {
    pub fn new(element: ElementId, offset: usize) -> Self {
        Self { element, offset }
    }
}

/// `anchor`（ドラッグ開始点）と `focus`（現在のポインタ位置）の間の連続テキスト選択。
/// フィールド順はドキュメント順を意味しない。呼び出し側は [`Selection::range_within`]
/// で正規化する。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub anchor: SelectionPoint,
    pub focus: SelectionPoint,
}

impl Selection {
    /// 1点に退化（collapse）した選択 — キャレット。
    pub fn caret(point: SelectionPoint) -> Self {
        Self {
            anchor: point,
            focus: point,
        }
    }

    /// 選択が collapse 状態（anchor == focus）= キャレットなら true。
    pub fn is_caret(&self) -> bool {
        self.anchor == self.focus
    }

    /// `element` 内の選択バイト範囲をドキュメント順（`start <= end`）に正規化して返す。
    /// 両端点が `element` 内にない場合は `None`（単一IFCのケース。要素跨ぎの選択は将来対応）。
    pub fn range_within(&self, element: ElementId) -> Option<(usize, usize)> {
        if self.anchor.element != element || self.focus.element != element {
            return None;
        }
        let a = self.anchor.offset;
        let b = self.focus.offset;
        Some((a.min(b), a.max(b)))
    }
}

/// 単語分割のための文字クラス。ダブルクリック（単語）選択は同一クラスの最大連続を
/// まとめる。デスクトップのテキストビューに倣い、英数字・空白・その他を別クラスとする。
#[derive(Clone, Copy, PartialEq, Eq)]
enum CharClass {
    Word,
    Space,
    Other,
}

fn classify(c: char) -> CharClass {
    if c.is_whitespace() {
        CharClass::Space
    } else if c.is_alphanumeric() || c == '_' {
        CharClass::Word
    } else {
        CharClass::Other
    }
}

/// `offset` を直前以前の最も近い `char` 境界に丸める（テキスト長でクランプ）。
/// 呼び出し側が生のバイトオフセットを安全に渡せるようにする。
fn floor_boundary(text: &str, offset: usize) -> usize {
    let mut o = offset.min(text.len());
    while o > 0 && !text.is_char_boundary(o) {
        o -= 1;
    }
    o
}

/// `offset` を含む「単語」のバイト範囲: `offset` の文字のクラス（単語/空白/その他）を
/// 共有する最大連続。テキスト末尾では直前の文字がクラスを決める（ADR-0097）。
pub fn word_bounds(text: &str, offset: usize) -> (usize, usize) {
    let len = text.len();
    if len == 0 {
        return (0, 0);
    }
    let offset = floor_boundary(text, offset);
    // 単語のクラスを決める文字: `offset` から始まる文字、末尾なら最後の文字。
    let pivot_start = if offset < len {
        offset
    } else {
        text.char_indices().next_back().map(|(i, _)| i).unwrap()
    };
    let class = classify(text[pivot_start..].chars().next().unwrap());

    let mut end = pivot_start;
    for (i, c) in text[pivot_start..].char_indices() {
        if classify(c) == class {
            end = pivot_start + i + c.len_utf8();
        } else {
            break;
        }
    }
    let mut start = pivot_start;
    for (i, c) in text[..pivot_start].char_indices().rev() {
        if classify(c) == class {
            start = i;
        } else {
            break;
        }
    }
    (start, end)
}

/// `offset` を含む段落（ハードライン）のバイト範囲。境界の `\n` 自体は含まない。
/// トリプルクリック選択。
pub fn line_bounds(text: &str, offset: usize) -> (usize, usize) {
    let offset = floor_boundary(text, offset);
    let start = text[..offset].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let end = text[offset..]
        .find('\n')
        .map(|i| offset + i)
        .unwrap_or(text.len());
    (start, end)
}

/// `offset` の次の `char` のバイトオフセット（末尾でクランプ）。
/// Shift+矢印が focus を1文字ずつ動かす。
pub fn next_grapheme(text: &str, offset: usize) -> usize {
    let offset = floor_boundary(text, offset);
    text[offset..]
        .chars()
        .next()
        .map(|c| offset + c.len_utf8())
        .unwrap_or(offset)
}

/// `offset` の前の `char` のバイトオフセット（0 でクランプ）。
pub fn prev_grapheme(text: &str, offset: usize) -> usize {
    let offset = floor_boundary(text, offset);
    text[..offset]
        .chars()
        .next_back()
        .map(|c| offset - c.len_utf8())
        .unwrap_or(0)
}

/// 左右矢印キーによる水平キャレット移動を解決する: 1グラフェム、または `by_word` なら
/// 1単語（macOS は Alt / Win/Linux は Ctrl）。矢印以外のキーでは `None` を返し、呼び出し
/// 側が他の処理にフォールスルーできる。読み取り専用 SelectionArea とテキスト入力の編集選択で
/// 共有（ADR-0097）。
pub fn arrow_step(text: &str, key: &str, offset: usize, by_word: bool) -> Option<usize> {
    Some(match (key, by_word) {
        ("ArrowRight", false) => next_grapheme(text, offset),
        ("ArrowLeft", false) => prev_grapheme(text, offset),
        ("ArrowRight", true) => next_word(text, offset),
        ("ArrowLeft", true) => prev_word(text, offset),
        _ => return None,
    })
}

/// `offset` の次の単語の末尾のバイトオフセット: 非単語の連続を飛ばし、続く単語の連続を
/// 消費する。単語単位の Shift+矢印。
pub fn next_word(text: &str, offset: usize) -> usize {
    let len = text.len();
    let mut o = floor_boundary(text, offset);
    while o < len {
        let c = text[o..].chars().next().unwrap();
        if classify(c) == CharClass::Word {
            break;
        }
        o += c.len_utf8();
    }
    while o < len {
        let c = text[o..].chars().next().unwrap();
        if classify(c) != CharClass::Word {
            break;
        }
        o += c.len_utf8();
    }
    o
}

/// `offset` の前の単語の先頭のバイトオフセット: 左方向に非単語の連続を飛ばし、続く単語の
/// 連続を消費する。
pub fn prev_word(text: &str, offset: usize) -> usize {
    let mut o = floor_boundary(text, offset);
    while o > 0 {
        let c = text[..o].chars().next_back().unwrap();
        if classify(c) == CharClass::Word {
            break;
        }
        o -= c.len_utf8();
    }
    while o > 0 {
        let c = text[..o].chars().next_back().unwrap();
        if classify(c) != CharClass::Word {
            break;
        }
        o -= c.len_utf8();
    }
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(id: u64, offset: usize) -> SelectionPoint {
        SelectionPoint::new(ElementId::from_u64(id), offset)
    }

    #[test]
    fn word_bounds_spans_the_word_under_an_interior_offset() {
        // "Hello world" — offset 2 は "Hello" の内側 → 0..5。
        assert_eq!(word_bounds("Hello world", 2), (0, 5));
        // "world" の内側 → 6..11。
        assert_eq!(word_bounds("Hello world", 8), (6, 11));
    }

    #[test]
    fn word_bounds_at_a_boundary_takes_the_following_word() {
        // offset 6 は "world" の先頭。
        assert_eq!(word_bounds("Hello world", 6), (6, 11));
        // 空白上の offset は空白の連続をまとめる。
        assert_eq!(word_bounds("Hello world", 5), (5, 6));
    }

    #[test]
    fn word_bounds_at_end_of_text_uses_the_preceding_word() {
        let text = "Hello world";
        assert_eq!(word_bounds(text, text.len()), (6, 11));
    }

    #[test]
    fn word_bounds_handles_multibyte_words() {
        // "あ い" — 最初の単語は3バイトの "あ"。
        let text = "あ い";
        assert_eq!(word_bounds(text, 0), (0, 3));
    }

    #[test]
    fn line_bounds_spans_the_paragraph_between_newlines() {
        let text = "one\ntwo\nthree";
        // "two" の内側: バイト 4..7（改行は除外）。
        assert_eq!(line_bounds(text, 5), (4, 7));
        // 最初の段落。
        assert_eq!(line_bounds(text, 1), (0, 3));
        // 最後の段落はテキスト末尾まで。
        assert_eq!(line_bounds(text, 9), (8, 13));
    }

    #[test]
    fn grapheme_stepping_moves_one_char_at_a_time() {
        let text = "aあb"; // 1 + 3 + 1 バイト
        assert_eq!(next_grapheme(text, 0), 1);
        assert_eq!(next_grapheme(text, 1), 4);
        assert_eq!(next_grapheme(text, text.len()), text.len());
        assert_eq!(prev_grapheme(text, 4), 1);
        assert_eq!(prev_grapheme(text, 0), 0);
    }

    #[test]
    fn word_stepping_jumps_across_word_runs() {
        let text = "Hello world";
        // 先頭からは次の単語境界が "Hello" の末尾。
        assert_eq!(next_word(text, 0), 5);
        // "Hello" の内側からも末尾に到達。
        assert_eq!(next_word(text, 2), 5);
        // 空白からは "world" の末尾へ飛ぶ。
        assert_eq!(next_word(text, 5), 11);
        // 末尾から後退すると "world" の先頭に到達。
        assert_eq!(prev_word(text, 11), 6);
        // "world" の内側から後退してもその先頭に到達。
        assert_eq!(prev_word(text, 8), 6);
    }

    #[test]
    fn caret_is_collapsed_at_a_single_point() {
        let sel = Selection::caret(point(1, 3));
        assert!(sel.is_caret());
        assert_eq!(sel.anchor, sel.focus);
        assert_eq!(sel.range_within(ElementId::from_u64(1)), Some((3, 3)));
    }

    #[test]
    fn range_within_normalizes_to_document_order() {
        // focus が anchor の前（左方向ドラッグ）でも start <= end になる。
        let sel = Selection {
            anchor: point(1, 7),
            focus: point(1, 2),
        };
        assert!(!sel.is_caret());
        assert_eq!(sel.range_within(ElementId::from_u64(1)), Some((2, 7)));
    }

    #[test]
    fn range_within_is_none_for_a_different_element() {
        let sel = Selection {
            anchor: point(1, 0),
            focus: point(1, 4),
        };
        assert_eq!(sel.range_within(ElementId::from_u64(2)), None);
    }
}
