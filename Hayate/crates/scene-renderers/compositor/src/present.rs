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

use crate::{FramePlan, LayerCache};

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
        assert!(planner.plan(&[id(1)], &dirty(&[])).needs_raster, "cold cache は全面 raster");
    }

    #[test]
    fn clean_frame_after_full_raster_is_composite_only() {
        let mut planner = PresentPlanner::new();
        planner.note_full_raster(&[id(1), id(2)]);
        let plan = planner.plan(&[id(1), id(2)], &dirty(&[]));
        assert!(plan.is_composite_only(), "layer_dirty 空・キャッシュ有効 → raster を呼ばない");
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
}
