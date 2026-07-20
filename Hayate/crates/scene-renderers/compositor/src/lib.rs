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
//!
//! ⚠️ **ADR-0135 により封印中** — この crate が支える per-layer 経路（web `layer-present`
//! feature）を有効化しないこと。#697 で実 Chromium 実行時に描画バグが確認され、実用段階に
//! ないと判定された。性能上の実害が具体的に発生するまで再開しない。

use std::collections::{HashMap, HashSet};

use hayate_core::element::id::ElementId;
use hayate_core::SceneGraph;

pub mod layer_scene;
pub use layer_scene::{
    collect_layer_placements, extract_layer_scene, extract_root_scene, extract_scroll_chrome_scene,
    extract_scroll_layer_scene, LayerPlacement,
};
pub mod pipeline_cache;
pub use pipeline_cache::PipelineCacheKey;
pub mod present;
pub use present::PresentPlanner;
pub mod layer_presentation;
pub use layer_presentation::{
    LayerPresentation, LayerPresentationAdapter, LayerPresentationError, LayerPresentationFrame,
    Placement, PlacementPlan, RasterJob, RasterJobKind,
};
pub mod raster_thread;
pub use raster_thread::{RasterCommand, RasterHandoff, RasterHandoffError, RasterThread};
pub mod scroll_geometry;
pub use scroll_geometry::{
    scroll_layer_geometry, scroll_layer_geometry_from_inputs, scroll_layer_geometry_table,
    RasterBand, ScrollLayerGeometry,
};

/// 名前付き tunable（ADR-0127）。オーバースキャン余白・GPU 予算・ピクセルバイトの単一正本。値は
/// プレースホルダで、マジックナンバーをロジックへ散らさないことが目的。予算（ビューポート N 枚分）は
/// platform が注入する既定値で、core のレイヤ判定はこれを知らない（policy=core, budget=platform）。
pub mod tunables {
    /// scroll 内容レイヤの可視域外オーバースキャン余白（上下それぞれ、論理 px）。
    pub const OVERSCAN_MARGIN_PX: f32 = 600.0;
    /// GPU 予算（ビューポート N 枚分）。モバイル既定は小さめ（ADR-0127）。
    pub const GPU_BUDGET_VIEWPORTS_MOBILE: f32 = 3.0;
    /// GPU 予算（ビューポート N 枚分）。デスクトップ/native ハイエンド既定は大きめ。
    pub const GPU_BUDGET_VIEWPORTS_DESKTOP: f32 = 8.0;
    /// 1 ピクセルあたりのバイト数（RGBA8 / BGRA8 とも 4）。
    pub const BYTES_PER_PIXEL: u64 = 4;
}

/// 1 フレームのレイヤ raster 計画。`raster` は再 raster が要るレイヤ（cache miss / dirty）、`reuse` は
/// キャッシュ texture をそのまま合成に使うレイヤ（描画順を保つ）。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RasterPlan {
    /// 本フレームで Vello/tiny-skia により再 raster するレイヤ（描画順）。
    pub raster: Vec<ElementId>,
    /// キャッシュ面を再利用するレイヤ（描画順）。
    pub reuse: Vec<ElementId>,
}

/// キャッシュ済みレイヤ 1 件の台帳エントリ。GPU ハンドルは持たず、サイズと composite 直近性だけを
/// 記録する（Send クリーン）。
#[derive(Debug, Clone, Copy)]
struct LayerEntry {
    /// キャッシュ texture のバイト数（予算計上用）。`mark_rasterized` では 0、サイズ付き raster で実値。
    bytes: u64,
    /// 最後に composite に使われた論理時刻（LRU 退避の基準。ADR-0127「最も長く composite に
    /// 使われていない」）。raster 時にも初期化する。
    last_composited: u64,
    /// scroll レイヤなら現在キャッシュ済みの縦帯（#634）。この帯が可視域を覆っている間は再 raster
    /// せず quad 平行移動で済ませる。非 scroll レイヤは `None`。
    scroll_band: Option<ScrollLayerExtent>,
}

/// レイヤ単位 retained texture キャッシュの **backend 非依存な台帳**。実 texture は backend が持つが、
/// 「どのレイヤがキャッシュ済みか」「このフレームでどれを raster するか」「予算超過で何を LRU 退避するか」
/// はここが決める。`Send` クリーン（ADR-0128 で Raster スレッドへ移せるよう、GPU ハンドルを持たない）。
#[derive(Debug, Default)]
pub struct LayerCache {
    cached: HashMap<ElementId, LayerEntry>,
    /// composite 使用の単調増加クロック（LRU の順序付け用）。
    tick: u64,
}

impl LayerCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// このレイヤのキャッシュ面が有効か（再 raster 不要か）。
    pub fn is_cached(&self, layer: ElementId) -> bool {
        self.cached.contains_key(&layer)
    }

    /// 本フレームの raster 計画を立てる。レイヤは (a) 未キャッシュ（cache miss / 退避済み）、または
    /// (b) `layer_dirty` に含まれる（内容が変わった）なら raster、それ以外はキャッシュ再利用。
    /// `layers` は現在の全レイヤ（描画順 = ADR-0021 の子順序）。
    pub fn plan_raster(
        &self,
        layers: &[ElementId],
        layer_dirty: &HashSet<ElementId>,
    ) -> RasterPlan {
        let mut plan = RasterPlan::default();
        for &layer in layers {
            if !self.cached.contains_key(&layer) || layer_dirty.contains(&layer) {
                plan.raster.push(layer);
            } else {
                plan.reuse.push(layer);
            }
        }
        plan
    }

    /// raster 済みレイヤをキャッシュ済みに記録する（サイズ未指定＝0 バイト計上。backend が texture を
    /// 書いた後に呼ぶ）。サイズを予算計上したい場合は [`mark_rasterized_sized`](Self::mark_rasterized_sized)。
    pub fn mark_rasterized(&mut self, layer: ElementId) {
        self.mark_rasterized_sized(layer, 0);
    }

    /// サイズ（バイト）付きで raster 済みレイヤを記録する。GPU 予算計上に使う（ADR-0127）。
    pub fn mark_rasterized_sized(&mut self, layer: ElementId, bytes: u64) {
        let tick = self.tick;
        self.cached.insert(
            layer,
            LayerEntry {
                bytes,
                last_composited: tick,
                scroll_band: None,
            },
        );
    }

    /// scroll レイヤの帯 raster を記録する（#634）。texture は帯サイズ（可視域＋overscan）なので
    /// `bytes` は帯サイズで渡す。以後この帯が可視域を覆う間は [`scroll_needs_raster`] が false を返し、
    /// present は quad 平行移動だけで済ませる（composite-only スクロール）。
    pub fn mark_scroll_rasterized(
        &mut self,
        layer: ElementId,
        band: ScrollLayerExtent,
        bytes: u64,
    ) {
        let tick = self.tick;
        self.cached.insert(
            layer,
            LayerEntry {
                bytes,
                last_composited: tick,
                scroll_band: Some(band),
            },
        );
    }

    /// scroll レイヤが本フレームで（差分）raster を要するか（#634）。未キャッシュ（cache miss /
    /// LRU 退避）か、キャッシュ帯が現在の可視域を覆っていなければ true。覆っていれば composite-only。
    pub fn scroll_needs_raster(
        &self,
        layer: ElementId,
        visible_top: f32,
        viewport_height: f32,
    ) -> bool {
        match self.cached.get(&layer).and_then(|e| e.scroll_band) {
            Some(band) => !band.covers(visible_top, viewport_height),
            None => true, // 未キャッシュ、または帯情報を持たない（非 scroll として記録された）。
        }
    }

    /// このレイヤの現在キャッシュ済み scroll 帯（content-local、#707）。合成時に、texture へ実際に
    /// 入っている帯を正しい画面位置へ戻す平行移動を組むために使う——**このフレームの**（新規）帯では
    /// なく、composite-only フレーム（`scroll_needs_raster` が false）では texture に入っている帯は
    /// 過去に raster したときのものなので、そちらを返す必要がある。scroll レイヤでない、または
    /// 未キャッシュなら `None`。
    pub fn cached_scroll_band(&self, layer: ElementId) -> Option<ScrollLayerExtent> {
        self.cached.get(&layer).and_then(|e| e.scroll_band)
    }

    /// レイヤを composite に使ったと記録し、LRU 直近性を更新する（ADR-0127）。退避は「最も長く
    /// composite に使われていない」面から行うため、合成のたびにこれを呼ぶ。
    pub fn note_composited(&mut self, layer: ElementId) {
        self.tick += 1;
        let tick = self.tick;
        if let Some(entry) = self.cached.get_mut(&layer) {
            entry.last_composited = tick;
        }
    }

    /// レイヤが消えた/退避されたらキャッシュから外す（再要求時に再 raster される）。
    pub fn evict(&mut self, layer: ElementId) {
        self.cached.remove(&layer);
    }

    /// 現在キャッシュ済みのレイヤ数（測定/テスト用）。
    pub fn cached_len(&self) -> usize {
        self.cached.len()
    }

    /// 全キャッシュ texture の合計バイト（予算判定用）。
    pub fn total_bytes(&self) -> u64 {
        self.cached.values().map(|e| e.bytes).sum()
    }

    /// Updates an existing entry's allocation while retaining its LRU and scroll-band state.
    pub fn update_bytes(&mut self, layer: ElementId, bytes: u64) {
        if let Some(entry) = self.cached.get_mut(&layer) {
            entry.bytes = bytes;
        }
    }

    /// GPU 予算超過なら、最も長く composite に使われていないレイヤ texture から LRU 退避し、合計を
    /// 予算内に収める（ADR-0127）。退避したレイヤ id を退避順に返す（再要求時に再 raster される）。
    /// 予算 0 や同点は決定的に（古い tick → 小さい ElementId 順で）退避する。
    pub fn enforce_budget(&mut self, budget: GpuBudget) -> Vec<ElementId> {
        let mut evicted = Vec::new();
        while self.total_bytes() > budget.max_bytes {
            let Some(victim) = self
                .cached
                .iter()
                .min_by_key(|(id, entry)| (entry.last_composited, id.to_u64()))
                .map(|(id, _)| *id)
            else {
                break;
            };
            self.cached.remove(&victim);
            evicted.push(victim);
        }
        evicted
    }
}

/// platform が注入する GPU texture 予算（ADR-0127）。core（#609）のレイヤ判定・`layer_dirty` は
/// これを知らない（policy=core, budget=platform）。単位「ビューポート N 枚分」をバイトに換算して持つ。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuBudget {
    pub max_bytes: u64,
}

impl GpuBudget {
    /// 明示バイトで予算を作る。
    pub fn from_bytes(max_bytes: u64) -> Self {
        Self { max_bytes }
    }

    /// ビューポート（幅×高 px）× N 枚 × 4byte で予算バイトを計算する。viewport と N は platform が
    /// 注入する（モバイルは [`tunables::GPU_BUDGET_VIEWPORTS_MOBILE`]、デスクトップは
    /// [`tunables::GPU_BUDGET_VIEWPORTS_DESKTOP`] 等）。
    pub fn from_viewports(viewport_w: u32, viewport_h: u32, viewports: f32) -> Self {
        let per = u64::from(viewport_w) * u64::from(viewport_h) * tunables::BYTES_PER_PIXEL;
        Self {
            max_bytes: (per as f64 * f64::from(viewports)) as u64,
        }
    }
}

/// scroll 内容レイヤの texture が覆う縦帯（論理 px・ADR-0127）。全高でなく「可視域＋上下オーバースキャン」
/// だけを raster してメモリを抑える。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollLayerExtent {
    /// 帯の上端（content 座標、px）。
    pub top: f32,
    /// 帯の高さ（px）。content 全高でクランプされ、全高は確保しない。
    pub height: f32,
}

impl ScrollLayerExtent {
    /// この帯が可視域 `[visible_top, visible_top + viewport_height]` を完全に覆うか。覆っていなければ
    /// 新規に現れた帯を差分 raster してキャッシュを更新する必要がある（ADR-0127）。
    pub fn covers(&self, visible_top: f32, viewport_height: f32) -> bool {
        self.top <= visible_top && (self.top + self.height) >= (visible_top + viewport_height)
    }

    /// この要求帯を高さ `capacity` の固定サイズ texture に収める。texture に余る高さは可視域の
    /// 上下へ均等に振り、content 端では要求帯内へスライドする。元の帯が可視域を覆い、かつ
    /// `capacity >= viewport_height` なら、返す帯も可視域を必ず覆う。
    pub fn fit_to_capacity(self, visible_top: f32, viewport_height: f32, capacity: f32) -> Self {
        let height = capacity.max(0.0).min(self.height.max(0.0));
        let max_top = (self.top + self.height - height).max(self.top);
        let overscan = (height - viewport_height).max(0.0) / 2.0;
        let top = (visible_top - overscan).clamp(self.top, max_top);
        Self { top, height }
    }
}

/// スクロール offset・可視域・content 全高・オーバースキャンから、raster すべき縦帯を計算する
/// （ADR-0127）。可視域の上下に `overscan` を足し、content の `[0, content_height]` でクランプする
/// ＝全高は確保しない（縮退版タイル化。本格タイル化への自然な前段）。
pub fn scroll_layer_extent(
    scroll_offset: f32,
    viewport_height: f32,
    content_height: f32,
    overscan: f32,
) -> ScrollLayerExtent {
    let top = (scroll_offset - overscan).max(0.0);
    let bottom = (scroll_offset + viewport_height + overscan).min(content_height);
    ScrollLayerExtent {
        top,
        height: (bottom - top).max(0.0),
    }
}

/// content-visible な帯の上端（content 座標・px・#639）。バウンス（overscroll）中は表示スクロール
/// offset が `[0, max]` の外へ出るが、**端を越えて露出するのは背景**であって新しい content ではない
/// （その見せ方＝rubber-band translate / Android stretch は合成時 affine が担う・ADR-0131）。したがって
/// content レイヤ帯が覆うべき範囲は、offset を `[0, max]` にクランプした「content-visible」位置で決まる。
///
/// これが #639 の芯：生 offset で帯カバレッジ（[`ScrollLayerExtent::covers`]）を判定すると、越境フレームは
/// content 帯が可視域を覆えず（帯は `[0, content_height]` にクランプされるため）、#634（帯内スクロールの
/// composite-only 化）が入ってもバウンス毎フレームがレイヤ再 raster に落ちる。クランプ後の top で帯と
/// カバレッジを組めば、バウンス中も端の帯を再利用でき composite-only を保てる。overshoot は帯にも
/// カバレッジにも一切影響しない（純粋に合成 affine の見せ方）。
pub fn scroll_content_visible_top(scroll_offset: f32, max_offset: f32) -> f32 {
    scroll_offset.clamp(0.0, max_offset.max(0.0))
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
/// surface は sRGB view で来ることがある（Android Vulkan 等）ため、sRGB variant も直積に含める。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SurfaceFormat {
    Bgra8Unorm,
    Bgra8UnormSrgb,
    Rgba8Unorm,
    Rgba8UnormSrgb,
}

impl SurfaceFormat {
    /// warmup で前倒し生成する全 surface format。
    pub const ALL: [SurfaceFormat; 4] = [
        SurfaceFormat::Bgra8Unorm,
        SurfaceFormat::Bgra8UnormSrgb,
        SurfaceFormat::Rgba8Unorm,
        SurfaceFormat::Rgba8UnormSrgb,
    ];
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

/// native では `Send`、wasm では無条件で満たされる境界（ADR-0128）。native の raster seam は
/// Raster スレッドへ移せるよう `Send` クリーンを要求するが、web は SceneGraph がスレッドを
/// 跨がない（エンジン丸ごと単一 Worker の近似形）ため wgpu の `!Send` 型をそのまま使える。
#[cfg(not(target_arch = "wasm32"))]
pub trait MaybeSend: Send {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Send> MaybeSend for T {}
#[cfg(target_arch = "wasm32")]
pub trait MaybeSend {}
#[cfg(target_arch = "wasm32")]
impl<T> MaybeSend for T {}

/// レイヤ 1 枚を cache texture へ raster する backend 能力（Vello = wgpu texture / tiny-skia =
/// `Pixmap`）。キャッシュ面は backend が所有・再利用し、planning（[`LayerCache`]）は台帳だけを持つ。
/// ADR-0128 の Raster スレッド分離に備え native では `Send` を要求し、cache/compositor を `Send`
/// クリーンな seam の裏に保つ（実行は現スレッドでよい。wasm は単一スレッドなので要求しない）。
pub trait LayerRasterizer: MaybeSend {
    /// backend ごとのキャッシュ面型（wgpu texture / `Pixmap`）。
    type Texture;
    /// `layer` の抽出済み sub-scene（[`layer_scene::extract_layer_scene`] /
    /// [`layer_scene::extract_root_scene`]）を透明クリアのキャッシュ面へ raster する。`band` が
    /// `Some` なら scroll 内容レイヤの overscan 帯サイジング（ADR-0127・#707）: 実装は
    /// キャッシュ面をフルサーフェスではなく `band.height` に合わせて確保し、`band.origin_y` が
    /// texture 行 0 に来るよう内容を平行移動して raster してよい（対応しない実装は無視して
    /// 従来どおりフルサーフェス raster してもよい——出力は変わらないがメモリ削減が効かないだけ）。
    fn rasterize(
        &mut self,
        layer: ElementId,
        scene: &SceneGraph,
        band: Option<RasterBand>,
    ) -> Result<(), String>;
    /// raster 済みキャッシュ面（合成入力）。未 raster / 破棄済みなら `None`。
    fn texture(&self, layer: ElementId) -> Option<&Self::Texture>;
    /// キャッシュ面 1 枚のバイト数（`mark_rasterized_sized` の計上値・ADR-0127）。
    fn texture_bytes_per_layer(&self) -> u64;
    /// レイヤ 1 枚のキャッシュ面を破棄する（LRU 退避・レイヤ消滅）。
    fn discard(&mut self, layer: ElementId);
    /// 全キャッシュ面を破棄する（resize / surface 再作成）。
    fn discard_all(&mut self);
}

/// キャッシュ texture quad を transform/opacity/軸並行 clip 付きで 1 render pass で合成する backend
/// 能力（専用 wgpu compositor。合成に Vello は使わない・ADR-0125 Decision 4）。角丸 clip はレイヤ
/// 内容に焼き込む。native では `Send` クリーン（ADR-0128。wasm は単一スレッドなので要求しない）。
pub trait LayerCompositor: MaybeSend {
    /// backend ごとのキャッシュ面型。
    type Texture;
    /// backend ごとの合成先（wgpu = surface view + format / tiny-skia = `Pixmap`）。
    type Target;
    /// `quads` を描画順に 1 pass で合成する。パイプライン variant は init 時に warmup 済みで、
    /// ここでの遅延生成は行わない（ADR-0130a。未生成 variant はエラー）。
    fn composite(
        &mut self,
        target: &mut Self::Target,
        quads: &[CompositeQuad<'_, Self::Texture>],
    ) -> Result<(), String>;
}

/// compositor が 1 枚のキャッシュ texture を合成するための quad（transform/opacity/clip 付き）。
/// `transform` / `clip` は [`layer_scene::collect_layer_placements`] の placement をそのまま渡す。
#[derive(Debug, Clone, Copy)]
pub struct CompositeQuad<'a, T> {
    pub layer: ElementId,
    /// kurbo アフィン係数 [a,b,c,d,e,f]（ADR-0020 の group transform、祖先合成済み）。
    pub transform: [f64; 6],
    pub opacity: f32,
    /// 祖先の軸並行 clip（デバイス空間 `[x, y, w, h]`）。scissor 相当。
    pub clip: Option<[f32; 4]>,
    pub texture: &'a T,
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
        assert!(
            plan.raster.is_empty(),
            "clean フレームでレイヤ再 raster はゼロ"
        );
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
        assert_eq!(
            variants.len(),
            SurfaceFormat::ALL.len() * BlendMode::ALL.len()
        );
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
        assert_send::<GpuBudget>();
    }

    // ── ADR-0127: scroll overscan サイジング ───────────────────────────────────

    #[test]
    fn tall_scroll_layer_is_sized_to_viewport_plus_overscan_not_full_height() {
        // content 10000px・viewport 800px・overscan 600px・offset 2000px。
        let extent = scroll_layer_extent(2000.0, 800.0, 10000.0, 600.0);
        // 全高 10000 ではなく、可視域 800 ＋ 上下 600 = 2000px だけ確保する。
        assert_eq!(extent.top, 1400.0);
        assert_eq!(extent.height, 2000.0);
        assert!(extent.height < 10000.0, "全高分の texture は確保しない");
        // 可視域は覆われている。
        assert!(extent.covers(2000.0, 800.0));
    }

    #[test]
    fn short_scroll_content_is_fully_covered() {
        // content が可視域＋overscan 未満なら全部を 1 帯に収める（クランプ）。
        let extent = scroll_layer_extent(0.0, 800.0, 500.0, 600.0);
        assert_eq!(extent.top, 0.0);
        assert_eq!(extent.height, 500.0);
    }

    #[test]
    fn scrolling_beyond_the_cached_band_requires_reraster() {
        // offset 0 で確保した帯は、overscan を超えてスクロールすると可視域を覆えなくなる。
        let band = scroll_layer_extent(0.0, 800.0, 10000.0, 600.0); // top 0, height 1400
        assert!(band.covers(0.0, 800.0));
        // overscan(600) を超えて 700px スクロール → 可視域下端 1500 > band 下端 1400 で未カバー。
        assert!(!band.covers(700.0, 800.0));
    }

    #[test]
    fn capacity_limited_band_slides_to_keep_the_visible_viewport_covered() {
        // 画面高に固定された compatible surface では、要求帯の先頭を単純に切ると、
        // 186px スクロール後の可視下端 842 が cache 下端 720 を越えて欠ける。
        let requested = ScrollLayerExtent {
            top: 0.0,
            height: 1442.0,
        };
        let fitted = requested.fit_to_capacity(186.0, 656.0, 720.0);
        assert_eq!(
            fitted,
            ScrollLayerExtent {
                top: 154.0,
                height: 720.0
            }
        );
        assert!(fitted.covers(186.0, 656.0));
    }

    #[test]
    fn capacity_limited_band_stays_inside_the_requested_content_range() {
        let requested = ScrollLayerExtent {
            top: 0.0,
            height: 1000.0,
        };
        assert_eq!(
            requested.fit_to_capacity(0.0, 656.0, 720.0),
            ScrollLayerExtent {
                top: 0.0,
                height: 720.0
            },
        );
        assert_eq!(
            requested.fit_to_capacity(344.0, 656.0, 720.0),
            ScrollLayerExtent {
                top: 280.0,
                height: 720.0
            },
        );
    }

    // ── #639: overscroll（バウンス）中の content-visible 帯 ─────────────────────

    #[test]
    fn in_bounds_offset_is_its_own_visible_top() {
        // 範囲内では content-visible top ＝ offset そのもの（クランプなし）。
        assert_eq!(scroll_content_visible_top(0.0, 4800.0), 0.0);
        assert_eq!(scroll_content_visible_top(1200.0, 4800.0), 1200.0);
        assert_eq!(scroll_content_visible_top(4800.0, 4800.0), 4800.0);
    }

    #[test]
    fn overscroll_offset_clamps_to_the_content_edge() {
        // バウンス中（offset が [0, max] の外）でも、露出するのは背景であって新しい content
        // ではない。覆うべき content 帯は端にクランプした位置で決まる（overshoot は合成 affine
        // が担う・ADR-0131）。下端越境は max、上端越境は 0 にクランプ。
        assert_eq!(
            scroll_content_visible_top(4920.0, 4800.0),
            4800.0,
            "下端越境は max へ"
        );
        assert_eq!(
            scroll_content_visible_top(-120.0, 4800.0),
            0.0,
            "上端越境は 0 へ"
        );
    }

    #[test]
    fn overscroll_band_from_visible_top_still_covers_the_viewport() {
        // 生 offset だと content 帯は可視域を覆えない（#634 が入ってもバウンスが再 raster に
        // 落ちる回帰）。content-visible top（クランプ）で組んだ帯なら覆う ＝ composite-only。
        let (max, vh, content_h) = (4800.0f32, 200.0f32, 5000.0f32);
        let raw_offset = max + 120.0; // 下端を 120px 越えたバウンスフレーム。
        let raw_band = scroll_layer_extent(raw_offset, vh, content_h, tunables::OVERSCAN_MARGIN_PX);
        assert!(
            !raw_band.covers(raw_offset, vh),
            "生 offset の帯は可視域を覆えない（回帰の芯）"
        );

        let top = scroll_content_visible_top(raw_offset, max);
        let band = scroll_layer_extent(top, vh, content_h, tunables::OVERSCAN_MARGIN_PX);
        assert!(
            band.covers(top, vh),
            "content-visible top の帯は可視域を覆う（composite-only）"
        );
    }

    #[test]
    fn non_scrollable_axis_visible_top_is_zero() {
        // スクロール不可（max <= 0）な軸は content-visible top 0 に張り付く（バウンスもしない）。
        assert_eq!(scroll_content_visible_top(0.0, 0.0), 0.0);
        assert_eq!(scroll_content_visible_top(50.0, 0.0), 0.0);
        assert_eq!(scroll_content_visible_top(-30.0, 0.0), 0.0);
    }

    // ── ADR-0127: GPU 予算＋LRU 退避 ───────────────────────────────────────────

    #[test]
    fn budget_from_viewports_is_viewport_bytes_times_n() {
        // 1000x800 ビューポート × 3 枚 × 4byte。
        let budget = GpuBudget::from_viewports(1000, 800, 3.0);
        assert_eq!(budget.max_bytes, 1000 * 800 * 4 * 3);
    }

    #[test]
    fn over_budget_evicts_lru_until_within_budget() {
        // 各 1000byte・予算 2500byte。3 枚入れると 3000 > 2500。
        let mut cache = LayerCache::new();
        cache.mark_rasterized_sized(id(1), 1000);
        cache.mark_rasterized_sized(id(2), 1000);
        cache.mark_rasterized_sized(id(3), 1000);
        // composite 順 = 3, 1, 2（→ 2 が最も新しく使われた = 1 が最も古い…順に応じて）。
        cache.note_composited(id(3));
        cache.note_composited(id(1));
        cache.note_composited(id(2));
        // → last_composited 昇順は 3(最古) < 1 < 2。予算超過分を 3 から退避。
        let evicted = cache.enforce_budget(GpuBudget::from_bytes(2500));
        assert_eq!(
            evicted,
            vec![id(3)],
            "最も長く composite に使われていない面から LRU 退避"
        );
        assert!(cache.total_bytes() <= 2500, "合計が予算内に収まる");
        // 退避された 3 は次フレームで再 raster 対象になる。
        let plan = cache.plan_raster(&[id(1), id(2), id(3)], &dirty(&[]));
        assert_eq!(plan.raster, vec![id(3)]);
        assert_eq!(plan.reuse, vec![id(1), id(2)]);
    }

    #[test]
    fn within_budget_evicts_nothing() {
        let mut cache = LayerCache::new();
        cache.mark_rasterized_sized(id(1), 1000);
        cache.mark_rasterized_sized(id(2), 1000);
        let evicted = cache.enforce_budget(GpuBudget::from_bytes(4000));
        assert!(evicted.is_empty());
        assert_eq!(cache.cached_len(), 2);
    }

    #[test]
    fn named_tunables_have_documented_placeholder_values() {
        // マジックナンバーを散らさないため、tunable は名前付き定数の単一正本に置く（ADR-0127）。
        assert!(tunables::OVERSCAN_MARGIN_PX > 0.0);
        assert!(tunables::GPU_BUDGET_VIEWPORTS_MOBILE < tunables::GPU_BUDGET_VIEWPORTS_DESKTOP);
        assert_eq!(tunables::BYTES_PER_PIXEL, 4);
    }
}
