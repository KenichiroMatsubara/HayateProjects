//! レイヤ texture キャッシュと専用 compositor の backend 非依存シーム（ADR-0125 backend 半分 /
//! ADR-0128 の `Send` seam / ADR-0130a パイプライン warmup）。
//!
//! `render_scene` を「全面描画」から「`layer_dirty` のレイヤだけ raster、残りはキャッシュ面を再利用」
//! へ変えるための **GPU 非依存の planning** をここに置く。実 raster（Vello = wgpu texture /
//! tiny-skia = `Pixmap`）と実 composite（専用 wgpu compositor）は platform backend が
//! [`LayerRasterizer`] / [`LayerCompositor`] trait 越しに差す。これにより以下をホストで固定する:
//!
//! - clean フレームでレイヤ再 raster がゼロ、変化フレームで dirty レイヤだけ raster（measurable
//!   work-count・ADR-0086 方式の拡張）。
//! - composite だけのフレームで Vello フルパイプラインを起動しない（[`FramePlan::is_composite_only`]）。
//! - init で全パイプライン variant（surface format × blend）を warmup（初回遅延生成なし・ADR-0130a）。
//! - cache/compositor が `Send` クリーンな seam の裏に隔離される（ADR-0128 の Raster スレッド分離の布石）。
//!
//! 同一 `layer_dirty`（ADR-0125 コア・#609）を入力にするため、tiny-skia(CPU) 経路も同じ planning で
//! 同じレイヤ化の恩恵を受ける（backend は trait 実装だけ差し替える）。

use std::collections::HashSet;

use hayate_core::element::id::ElementId;

/// 1 フレームのレイヤ raster 計画。`raster` は再 raster が要るレイヤ（cache miss / dirty）、`reuse` は
/// キャッシュ texture をそのまま合成に使うレイヤ（描画順を保つ）。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RasterPlan {
    /// 本フレームで Vello/tiny-skia により再 raster するレイヤ（描画順）。
    pub raster: Vec<ElementId>,
    /// キャッシュ面を再利用するレイヤ（描画順）。
    pub reuse: Vec<ElementId>,
}

/// レイヤ単位 retained texture キャッシュの **backend 非依存な台帳**。実 texture は backend が持つが、
/// 「どのレイヤがキャッシュ済みか」「このフレームでどれを raster するか」はここが決める。`Send` クリーン
/// （ADR-0128 で Raster スレッドへ移せるよう、GPU ハンドルを持たない）。
#[derive(Debug, Default)]
pub struct LayerCache {
    cached: HashSet<ElementId>,
}

impl LayerCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// このレイヤのキャッシュ面が有効か（再 raster 不要か）。
    pub fn is_cached(&self, layer: ElementId) -> bool {
        self.cached.contains(&layer)
    }

    /// 本フレームの raster 計画を立てる。レイヤは (a) 未キャッシュ（cache miss）、または
    /// (b) `layer_dirty` に含まれる（内容が変わった）なら raster、それ以外はキャッシュ再利用。
    /// `layers` は現在の全レイヤ（描画順 = ADR-0021 の子順序）。
    pub fn plan_raster(&self, layers: &[ElementId], layer_dirty: &HashSet<ElementId>) -> RasterPlan {
        let mut plan = RasterPlan::default();
        for &layer in layers {
            if !self.cached.contains(&layer) || layer_dirty.contains(&layer) {
                plan.raster.push(layer);
            } else {
                plan.reuse.push(layer);
            }
        }
        plan
    }

    /// raster 済みレイヤをキャッシュ済みに記録する（backend が texture を書いた後に呼ぶ）。
    pub fn mark_rasterized(&mut self, layer: ElementId) {
        self.cached.insert(layer);
    }

    /// レイヤが消えた/退避されたらキャッシュから外す（再要求時に再 raster される）。
    pub fn evict(&mut self, layer: ElementId) {
        self.cached.remove(&layer);
    }

    /// 現在キャッシュ済みのレイヤ数（測定/テスト用）。
    pub fn cached_len(&self) -> usize {
        self.cached.len()
    }
}

/// 1 フレームの描画計画。`needs_raster` が false なら Vello/tiny-skia を起動せず、専用 compositor だけ
/// でキャッシュ面を合成する（composite-only フレーム＝ADR-0125「合成は安い・毎フレーム」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FramePlan {
    /// 本フレームで raster パイプライン（Vello/tiny-skia）を起動する必要があるか。
    pub needs_raster: bool,
}

impl FramePlan {
    /// raster 計画から導く。raster 対象が 1 つでもあれば raster パイプライン起動、無ければ composite-only。
    pub fn from_raster(plan: &RasterPlan) -> Self {
        Self {
            needs_raster: !plan.raster.is_empty(),
        }
    }

    /// composite だけのフレームか（Vello フルパイプラインを起動しない）。
    pub fn is_composite_only(&self) -> bool {
        !self.needs_raster
    }
}

/// compositor パイプラインの surface format variant。warmup の正本（マジック値の散在を防ぐ）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SurfaceFormat {
    Bgra8Unorm,
    Rgba8Unorm,
}

impl SurfaceFormat {
    /// warmup で前倒し生成する全 surface format。
    pub const ALL: [SurfaceFormat; 2] = [SurfaceFormat::Bgra8Unorm, SurfaceFormat::Rgba8Unorm];
}

/// compositor の blend variant。キャッシュ texture quad を不透明/アルファ合成する。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlendMode {
    Opaque,
    Alpha,
}

impl BlendMode {
    /// warmup で前倒し生成する全 blend。
    pub const ALL: [BlendMode; 2] = [BlendMode::Opaque, BlendMode::Alpha];
}

/// 1 つの compositor パイプライン variant（surface format × blend）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PipelineVariant {
    pub format: SurfaceFormat,
    pub blend: BlendMode,
}

/// init 時に warmup する全 compositor パイプライン variant（surface format × blend の直積）。backend は
/// エンジン初期化時にこの全 variant を前倒し生成し、初回合成フレームで遅延生成（初回操作スパイク）が
/// 走らないようにする（ADR-0130a）。
pub fn warmup_variants() -> Vec<PipelineVariant> {
    let mut out = Vec::with_capacity(SurfaceFormat::ALL.len() * BlendMode::ALL.len());
    for format in SurfaceFormat::ALL {
        for blend in BlendMode::ALL {
            out.push(PipelineVariant { format, blend });
        }
    }
    out
}

/// レイヤ 1 枚を cache texture へ raster する backend 能力（Vello = wgpu texture / tiny-skia =
/// `Pixmap`）。ADR-0128 の Raster スレッド分離に備え `Send` を要求し、cache/compositor を `Send`
/// クリーンな seam の裏に保つ（実行は現スレッドでよい）。
pub trait LayerRasterizer: Send {
    /// backend ごとのキャッシュ面型（wgpu texture / `Pixmap`）。
    type Texture;
    /// `layer` のサブツリーをキャッシュ面へ raster して返す。
    fn rasterize(&mut self, layer: ElementId) -> Self::Texture;
}

/// キャッシュ texture quad を transform/opacity 付きで 1 render pass で合成する backend 能力（専用
/// wgpu compositor。合成に Vello は使わない・ADR-0125 Decision 4）。compositor は軸並行 clip のみ扱い、
/// 角丸 clip はレイヤ内容に焼き込む。`Send` クリーン（ADR-0128）。
pub trait LayerCompositor: Send {
    /// backend ごとのキャッシュ面型。
    type Texture;
    /// `quads`（レイヤ id・kurbo アフィン [a,b,c,d,e,f]・opacity・キャッシュ面）を描画順に 1 pass で合成。
    fn composite(&mut self, quads: &[CompositeQuad<Self::Texture>]);
}

/// compositor が 1 枚のキャッシュ texture を合成するための quad（transform/opacity 付き）。
#[derive(Debug, Clone, Copy)]
pub struct CompositeQuad<T> {
    pub layer: ElementId,
    /// kurbo アフィン係数 [a,b,c,d,e,f]（ADR-0020 の group transform）。
    pub transform: [f64; 6],
    pub opacity: f32,
    pub texture: T,
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
    fn cold_cache_rasters_every_layer() {
        // 起動直後はどのレイヤもキャッシュ未生成 → 全レイヤ raster（cache miss）。
        let cache = LayerCache::new();
        let layers = [id(1), id(2), id(3)];
        let plan = cache.plan_raster(&layers, &dirty(&[]));
        assert_eq!(plan.raster, vec![id(1), id(2), id(3)]);
        assert!(plan.reuse.is_empty());
    }

    #[test]
    fn clean_frame_rasters_zero_layers() {
        // 全レイヤをキャッシュ済みにし、layer_dirty が空（clean フレーム）→ 再 raster ゼロ・全 reuse。
        let mut cache = LayerCache::new();
        let layers = [id(1), id(2), id(3)];
        for &l in &layers {
            cache.mark_rasterized(l);
        }
        let plan = cache.plan_raster(&layers, &dirty(&[]));
        assert!(plan.raster.is_empty(), "clean フレームでレイヤ再 raster はゼロ");
        assert_eq!(plan.reuse, vec![id(1), id(2), id(3)]);
    }

    #[test]
    fn changed_frame_rasters_only_dirty_layers() {
        // キャッシュ済みで layer_dirty = {2} → レイヤ 2 だけ raster、他は reuse（damage 比例）。
        let mut cache = LayerCache::new();
        let layers = [id(1), id(2), id(3)];
        for &l in &layers {
            cache.mark_rasterized(l);
        }
        let plan = cache.plan_raster(&layers, &dirty(&[2]));
        assert_eq!(plan.raster, vec![id(2)]);
        assert_eq!(plan.reuse, vec![id(1), id(3)]);
    }

    #[test]
    fn evicted_layer_is_rerastered_next_frame() {
        let mut cache = LayerCache::new();
        let layers = [id(1), id(2)];
        cache.mark_rasterized(id(1));
        cache.mark_rasterized(id(2));
        cache.evict(id(1));
        let plan = cache.plan_raster(&layers, &dirty(&[]));
        // 退避された 1 は再 raster、2 は reuse のまま。
        assert_eq!(plan.raster, vec![id(1)]);
        assert_eq!(plan.reuse, vec![id(2)]);
    }

    #[test]
    fn composite_only_frame_does_not_need_raster() {
        // clean フレーム（raster 対象なし）→ composite-only。Vello フルパイプラインを起動しない。
        let mut cache = LayerCache::new();
        let layers = [id(1), id(2)];
        cache.mark_rasterized(id(1));
        cache.mark_rasterized(id(2));
        let plan = cache.plan_raster(&layers, &dirty(&[]));
        let frame = FramePlan::from_raster(&plan);
        assert!(frame.is_composite_only());
        assert!(!frame.needs_raster);
    }

    #[test]
    fn frame_with_a_dirty_layer_needs_raster() {
        let cache = LayerCache::new();
        let plan = cache.plan_raster(&[id(1)], &dirty(&[1]));
        let frame = FramePlan::from_raster(&plan);
        assert!(frame.needs_raster);
        assert!(!frame.is_composite_only());
    }

    #[test]
    fn warmup_enumerates_all_format_blend_variants_uniquely() {
        // 初回遅延生成を消すため、surface format × blend の全直積を前倒し生成する（ADR-0130a）。
        let variants = warmup_variants();
        assert_eq!(variants.len(), SurfaceFormat::ALL.len() * BlendMode::ALL.len());
        let unique: HashSet<_> = variants.iter().copied().collect();
        assert_eq!(unique.len(), variants.len(), "variant に重複が無い");
        assert!(variants.contains(&PipelineVariant {
            format: SurfaceFormat::Bgra8Unorm,
            blend: BlendMode::Alpha,
        }));
    }

    #[test]
    fn cache_is_send_clean() {
        // ADR-0128: cache は GPU ハンドルを持たず Send クリーン（Raster スレッドへ移せる）。
        fn assert_send<T: Send>() {}
        assert_send::<LayerCache>();
        assert_send::<RasterPlan>();
        assert_send::<FramePlan>();
        assert_send::<PipelineVariant>();
    }
}
