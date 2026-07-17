//! present 経路の raster gating（#632 prefactor・ADR-0125 backend 半分の入口）。
//!
//! backend（Android / web）の present は毎フレーム、core の `frame_layers()` /
//! `frame_layer_dirty()`（root 暗黙レイヤ込み）をここへ渡し、[`LayerCache::plan_raster`] →
//! [`FramePlan`] の判定を**通してから** raster する。この slice ではレイヤを実質単一 root として
//! 扱う：`needs_raster` なら従来どおり全面 raster（`render_scene`）し、全レイヤをキャッシュ済みに
//! 記録する。`layer_dirty` 空でキャッシュ有効なら raster を呼ばず、直前の raster 済み target を
//! そのまま合成（blit / 既表示 canvas 維持）する＝描画出力は不変のまま raster 回数だけ減る。
//!
//! 後続（#633 wgpu compositor / #634 scroll レイヤ / #635 RasterThread / #636 web）は、この
//! gating の裏でレイヤ単位のキャッシュ面と合成に差し替わる（trait 実装の差し替えだけになる）。

use std::collections::HashSet;

use hayate_core::element::id::ElementId;

use crate::{FramePlan, GpuBudget, LayerCache, RasterPlan, ScrollLayerExtent};

/// backend 非依存の present 側 raster gating。「このフレームで raster パイプラインを起動するか」を
/// [`LayerCache`] の台帳から決める。GPU ハンドルは持たない（`Send` クリーン・ADR-0128 の布石）。
#[derive(Debug, Default)]
pub struct PresentPlanner {
    cache: LayerCache,
}

impl PresentPlanner {
    pub fn new() -> Self {
        Self::default()
    }

    /// 本フレームの描画計画。`layers` / `layer_dirty` は core が `render()` で捕捉した
    /// `frame_layers()` / `frame_layer_dirty()`（root 暗黙レイヤ込み）をそのまま渡す。
    /// 1 レイヤでも未キャッシュ / dirty なら raster、全レイヤのキャッシュが有効なら composite-only。
    pub fn plan(&self, layers: &[ElementId], layer_dirty: &HashSet<ElementId>) -> FramePlan {
        FramePlan::from_raster(&self.cache.plan_raster(layers, layer_dirty))
    }

    /// per-layer の raster 計画（#633）。dirty レイヤと未キャッシュレイヤだけを `raster` に、残りを
    /// `reuse` に置く。transform 係数だけが変わったレイヤ（`frame_layer_transform_dirty`）は
    /// ここに渡さない——内容キャッシュは有効なままで、合成時の quad transform 更新だけが要る。
    pub fn plan_layers(
        &self,
        layers: &[ElementId],
        content_dirty: &HashSet<ElementId>,
    ) -> RasterPlan {
        self.cache.plan_raster(layers, content_dirty)
    }

    /// レイヤ 1 枚の raster 完了をサイズ（バイト）付きで記録する（`mark_rasterized_sized`）。
    /// GPU 予算（ADR-0127）の計上に使う。
    pub fn note_layer_rasterized(&mut self, layer: ElementId, bytes: u64) {
        self.cache.mark_rasterized_sized(layer, bytes);
    }

    /// 全キャッシュ texture の合計バイト（予算判定・テスト用）。
    pub fn cached_bytes(&self) -> u64 {
        self.cache.total_bytes()
    }

    /// scroll レイヤの帯 raster をサイズ付きで記録する（#634）。`band` は今回 raster した縦帯
    /// （可視域＋overscan）、`bytes` は帯サイズの texture バイト（content 全高でなく帯サイズ）。
    pub fn note_scroll_rasterized(
        &mut self,
        layer: ElementId,
        band: ScrollLayerExtent,
        bytes: u64,
    ) {
        self.cache.mark_scroll_rasterized(layer, band, bytes);
    }

    /// scroll レイヤが本フレームで（差分）raster を要するか（#634）。キャッシュ帯が現在の可視域
    /// `[visible_top, visible_top + viewport_height]` を覆っていれば false（composite-only スクロール）。
    pub fn scroll_layer_needs_raster(
        &self,
        layer: ElementId,
        visible_top: f32,
        viewport_height: f32,
    ) -> bool {
        self.cache
            .scroll_needs_raster(layer, visible_top, viewport_height)
    }

    /// レイヤの現在キャッシュ済み scroll 帯（content-local、#707）。合成時の compensating
    /// translate に使う——`scroll_layer_needs_raster`/`note_scroll_rasterized` は「raster すべきか」
    /// を判定・記録するが、実際に texture へ入っている帯（composite-only フレームでは過去に
    /// raster したときのもの）を読むにはこちらを使う。
    pub fn cached_scroll_band(&self, layer: ElementId) -> Option<ScrollLayerExtent> {
        self.cache.cached_scroll_band(layer)
    }

    /// GPU 予算超過なら最も長く composite に使われていないレイヤ texture から LRU 退避し、合計を
    /// 予算内に収める（ADR-0127）。退避した id を返す（backend は対応する実 texture を解放する）。
    pub fn enforce_budget(&mut self, budget: GpuBudget) -> Vec<ElementId> {
        self.cache.enforce_budget(budget)
    }

    /// レイヤを composite に使ったと記録する（LRU 直近性の更新・ADR-0127）。
    pub fn note_composited(&mut self, layer: ElementId) {
        self.cache.note_composited(layer);
    }

    /// レイヤ 1 枚を台帳から外す（レイヤ消滅・退避）。次に必要になったら再 raster される。
    pub fn evict(&mut self, layer: ElementId) {
        self.cache.evict(layer);
    }

    /// 全面 raster（従来の `render_scene` 1 回）が完了したことを記録する。単一 root slice では
    /// 全面 raster が全レイヤの内容をターゲットへ焼き直すので、`layers` 全部をキャッシュ済みにする。
    pub fn note_full_raster(&mut self, layers: &[ElementId]) {
        for &layer in layers {
            self.cache.mark_rasterized(layer);
        }
    }

    /// キャッシュ面が失われた（surface 再作成 / resize でターゲット texture を作り直した）。
    /// 次フレームは clean でも全面 raster される。
    pub fn invalidate(&mut self) {
        self.cache = LayerCache::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(raw: u64) -> ElementId {
        ElementId::from_u64(raw)
    }

    fn dirty(ids: &[u64]) -> HashSet<ElementId> {
        ids.iter().map(|&r| id(r)).collect()
    }

    #[test]
    fn cold_planner_needs_raster() {
        let planner = PresentPlanner::new();
        assert!(
            planner.plan(&[id(1)], &dirty(&[])).needs_raster,
            "cold cache は全面 raster"
        );
    }

    #[test]
    fn clean_frame_after_full_raster_is_composite_only() {
        let mut planner = PresentPlanner::new();
        planner.note_full_raster(&[id(1), id(2)]);
        let plan = planner.plan(&[id(1), id(2)], &dirty(&[]));
        assert!(
            plan.is_composite_only(),
            "layer_dirty 空・キャッシュ有効 → raster を呼ばない"
        );
    }

    #[test]
    fn dirty_layer_needs_raster() {
        let mut planner = PresentPlanner::new();
        planner.note_full_raster(&[id(1), id(2)]);
        assert!(planner.plan(&[id(1), id(2)], &dirty(&[2])).needs_raster);
    }

    #[test]
    fn newly_appearing_layer_needs_raster() {
        // 新レイヤ（進行中 transition の開始等）はキャッシュ未生成なので raster が要る。
        let mut planner = PresentPlanner::new();
        planner.note_full_raster(&[id(1)]);
        assert!(planner.plan(&[id(1), id(3)], &dirty(&[])).needs_raster);
    }

    #[test]
    fn invalidate_discards_the_cache() {
        let mut planner = PresentPlanner::new();
        planner.note_full_raster(&[id(1)]);
        planner.invalidate();
        assert!(planner.plan(&[id(1)], &dirty(&[])).needs_raster);
    }

    #[test]
    fn planner_is_send_clean() {
        // ADR-0128: Raster スレッドへ移せるよう GPU ハンドルを持たない。
        fn assert_send<T: Send>() {}
        assert_send::<PresentPlanner>();
    }

    // ── #707: cached_scroll_band（合成時 compensating translate 用） ────────────

    #[test]
    fn cached_scroll_band_is_none_before_any_scroll_raster() {
        let planner = PresentPlanner::new();
        assert_eq!(planner.cached_scroll_band(id(1)), None);
    }

    #[test]
    fn cached_scroll_band_reflects_the_last_rastered_band_not_this_frames() {
        let mut planner = PresentPlanner::new();
        let first = ScrollLayerExtent {
            top: 0.0,
            height: 800.0,
        };
        planner.note_scroll_rasterized(id(1), first, 1000);
        assert_eq!(planner.cached_scroll_band(id(1)), Some(first));

        // composite-only フレーム（帯が可視域を覆っている間）は raster しない＝texture の帯は
        // 前回のまま——`cached_scroll_band` は常にそれを返す（このフレームの新規帯ではない）。
        assert_eq!(planner.cached_scroll_band(id(1)), Some(first));

        let second = ScrollLayerExtent {
            top: 200.0,
            height: 800.0,
        };
        planner.note_scroll_rasterized(id(1), second, 1000);
        assert_eq!(
            planner.cached_scroll_band(id(1)),
            Some(second),
            "再 raster 後は新しい帯を返す"
        );
    }

    #[test]
    fn cached_scroll_band_is_none_for_a_blanket_rastered_layer() {
        // 非 scroll レイヤ（`note_layer_rasterized`/`mark_rasterized_sized` 経由）は帯情報を持たない。
        let mut planner = PresentPlanner::new();
        planner.note_layer_rasterized(id(1), 1000);
        assert_eq!(planner.cached_scroll_band(id(1)), None);
    }

    #[test]
    fn cached_scroll_band_is_cleared_on_eviction() {
        let mut planner = PresentPlanner::new();
        let band = ScrollLayerExtent {
            top: 0.0,
            height: 800.0,
        };
        planner.note_scroll_rasterized(id(1), band, 1000);
        planner.evict(id(1));
        assert_eq!(planner.cached_scroll_band(id(1)), None);
    }
}
