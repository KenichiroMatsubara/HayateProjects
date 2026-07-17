//! ポインタジェスチャ分類器（ADR-0066）。Interaction 状態機械を単独所有する
//! runtime（ADR-0066）の内部継ぎ目で、pointer-down/move/up/long-press の列を
//! 分類済みの結果へ写す。
//!
//! 二つの関心を一つの純粋な状態型に集約する:
//!
//! - **タップ位相**: 同じ箇所付近の連続 pointer-down は caret → word →
//!   paragraph を巡回する（#267）。`classify_tap` がこの位相を返し、`reset_taps`
//!   が新規ジェスチャ（例: long-press の後）で周期を再開する。
//! - **ドラッグ種別**: 進行中の pointer-move を駆動するドラッグが「読み取り専用
//!   SelectionArea（ADR-0097）／text-input 編集選択（ADR-0097）／Mouse/Pen
//!   スクロールバーつまみ（ADR-0110）」のどれか。三者は排他なので単一の
//!   [`DragMode`] enum で表し、「どの `*_drag` ブール値がたまたま立っているか」で
//!   含意させない。
//!
//! DOM も wasm も要らない純粋な状態なので、ポインタ列を直接流してコア単体テストで
//! 分類結果を検証できる。

use crate::element::id::ElementId;
use crate::element::interaction::ScrollbarDrag;

/// 同じ箇所付近の押下を「近接」とみなす許容（px）。これを超える押下は連続
/// クリック周期を再開する（#267）。
pub(crate) const MULTI_CLICK_TOLERANCE: f32 = 4.0;

/// 同一箇所付近の連続 pointer-down が巡回するタップ位相（#267）。1, 2, 3 回目で
/// caret → word → paragraph、以降は繰り返す。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TapPhase {
    /// 1 回目: キャレットを落とす（ドラッグで拡張できる）。
    Caret,
    /// 2 回目: 単語を選択する。
    Word,
    /// 3 回目: 段落／行を選択する。
    Paragraph,
}

/// 進行中のポインタドラッグの種別（ADR-0066）。pointer-down が掴んだものに従って
/// 分類され、各 pointer-move を駆動する。三者は排他なので単一の enum で表す。
#[derive(Clone, Copy, Debug, Default)]
pub(crate) enum DragMode {
    /// ドラッグ中でない。
    #[default]
    None,
    /// 読み取り専用 SelectionArea のドラッグ選択（ADR-0097）。
    Selection,
    /// text-input の編集選択ドラッグ。掴んだフィールド（ADR-0097）。
    Edit(ElementId),
    /// Mouse/Pen スクロールバーつまみドラッグ（ADR-0110）。
    Scrollbar(ScrollbarDrag),
}

/// ポインタジェスチャ分類器（ADR-0066）。進行中の [`DragMode`] と連続クリック
/// 追跡を単独所有する。これにより interaction runtime は散らばった `*_drag`
/// ブール値とマルチクリックの生フィールドの代わりに、一つの名前付き状態を持つ。
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PointerGesture {
    drag: DragMode,
    /// 直近 pointer-down の位置。近接判定の基準。
    last_tap_pos: Option<(f32, f32)>,
    /// 直近 pointer-down 付近に着地した連続押下回数。
    tap_count: u32,
}

impl PointerGesture {
    /// 進行中のドラッグ種別。
    pub(crate) fn drag(&self) -> DragMode {
        self.drag
    }

    /// この種別でドラッグを開始（または更新）する。
    pub(crate) fn begin_drag(&mut self, mode: DragMode) {
        self.drag = mode;
    }

    /// 進行中のドラッグを終える（pointer-up / pointer-cancel）。どの種別でも畳む。
    pub(crate) fn end_drag(&mut self) {
        self.drag = DragMode::None;
    }

    /// 読み取り専用 SelectionArea のドラッグだけを止める。他種別（編集・
    /// スクロールバー）のドラッグは触らない。long-press が選択ドラッグ周期を
    /// 畳むときに使う。
    pub(crate) fn clear_selection_drag(&mut self) {
        if matches!(self.drag, DragMode::Selection) {
            self.drag = DragMode::None;
        }
    }

    /// pointer-down を分類する。直前の押下付近なら連続押下カウンタを増やし、
    /// さもなくば 1 から再開して、caret → word → paragraph を巡回するタップ位相を
    /// 返す。
    pub(crate) fn classify_tap(&mut self, x: f32, y: f32) -> TapPhase {
        let near = self.last_tap_pos.is_some_and(|(lx, ly)| {
            (x - lx).abs() <= MULTI_CLICK_TOLERANCE && (y - ly).abs() <= MULTI_CLICK_TOLERANCE
        });
        self.tap_count = if near { self.tap_count + 1 } else { 1 };
        self.last_tap_pos = Some((x, y));
        match (self.tap_count - 1) % 3 {
            0 => TapPhase::Caret,
            1 => TapPhase::Word,
            _ => TapPhase::Paragraph,
        }
    }

    /// この箇所に単一タップを記録する（巡回させない）。Shift+クリックは範囲を
    /// 拡張するだけでマルチクリック周期を進めないので、次の素の押下が新規地点と
    /// 比較できるよう位置を覚えつつ回数を 1 に固定する。
    pub(crate) fn note_single_tap(&mut self, x: f32, y: f32) {
        self.last_tap_pos = Some((x, y));
        self.tap_count = 1;
    }

    /// 連続クリック追跡を再開する（新規ジェスチャ。例: long-press の後）。続く
    /// 素のタップはキャレットから始まる。
    pub(crate) fn reset_taps(&mut self) {
        self.last_tap_pos = None;
        self.tap_count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_taps_at_one_spot_cycle_caret_word_paragraph() {
        let mut g = PointerGesture::default();
        assert_eq!(g.classify_tap(10.0, 10.0), TapPhase::Caret);
        assert_eq!(g.classify_tap(10.0, 10.0), TapPhase::Word);
        assert_eq!(g.classify_tap(10.0, 10.0), TapPhase::Paragraph);
        // 3 を越えるとキャレットから巡回し直す。
        assert_eq!(g.classify_tap(10.0, 10.0), TapPhase::Caret);
    }

    #[test]
    fn taps_within_tolerance_keep_cycling() {
        let mut g = PointerGesture::default();
        assert_eq!(g.classify_tap(10.0, 10.0), TapPhase::Caret);
        // 許容内（4px）のずれは「同じ箇所」とみなす。
        assert_eq!(
            g.classify_tap(10.0 + MULTI_CLICK_TOLERANCE, 10.0),
            TapPhase::Word
        );
    }

    #[test]
    fn tap_beyond_tolerance_restarts_at_caret() {
        let mut g = PointerGesture::default();
        assert_eq!(g.classify_tap(10.0, 10.0), TapPhase::Caret);
        assert_eq!(g.classify_tap(10.0, 10.0), TapPhase::Word);
        // 許容を越えた押下は新規箇所なのでキャレットへ戻る。
        assert_eq!(
            g.classify_tap(10.0 + MULTI_CLICK_TOLERANCE + 1.0, 10.0),
            TapPhase::Caret
        );
    }

    #[test]
    fn reset_taps_makes_next_tap_a_caret() {
        let mut g = PointerGesture::default();
        g.classify_tap(10.0, 10.0);
        g.classify_tap(10.0, 10.0); // Word
                                    // long-press 後など、周期を再開すると同じ箇所でもキャレットから始まる。
        g.reset_taps();
        assert_eq!(g.classify_tap(10.0, 10.0), TapPhase::Caret);
    }

    #[test]
    fn note_single_tap_does_not_advance_cycle() {
        let mut g = PointerGesture::default();
        // Shift+クリックは単一タップとして記録するので、続く同箇所の素の押下は
        // キャレットでなく単語へ進む（回数 1 → 2）。
        g.note_single_tap(10.0, 10.0);
        assert_eq!(g.classify_tap(10.0, 10.0), TapPhase::Word);
    }

    #[test]
    fn drag_mode_defaults_to_none() {
        let g = PointerGesture::default();
        assert!(matches!(g.drag(), DragMode::None));
    }

    #[test]
    fn begin_then_end_drag_round_trips_to_none() {
        let mut g = PointerGesture::default();
        g.begin_drag(DragMode::Selection);
        assert!(matches!(g.drag(), DragMode::Selection));
        g.end_drag();
        assert!(matches!(g.drag(), DragMode::None));
    }

    #[test]
    fn clear_selection_drag_only_clears_selection() {
        let mut g = PointerGesture::default();
        // 選択ドラッグは畳まれる。
        g.begin_drag(DragMode::Selection);
        g.clear_selection_drag();
        assert!(matches!(g.drag(), DragMode::None));

        // 編集ドラッグは long-press の選択クリアでは触らない。
        let field = ElementId::from_u64(7);
        g.begin_drag(DragMode::Edit(field));
        g.clear_selection_drag();
        assert!(matches!(g.drag(), DragMode::Edit(id) if id == field));
    }
}
