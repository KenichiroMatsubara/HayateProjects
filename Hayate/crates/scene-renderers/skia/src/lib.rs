//! skia-safe Scene Renderer for Hayate — native only (ADR-0146 / ADR-0147).
//!
//! Consumption contract: [`SkiaSceneRenderer::render_scene`] plus a minimal set of raster
//! surface helpers (ADR-0054 style: the walk and the [`hayate_core::ScenePainter`]
//! implementation are crate-internal, never exported). The painter itself
//! ([`painter::SkiaPainter`]) is surface-agnostic — it only ever touches the
//! [`skia_safe::Canvas`] it is handed, so the same code path draws into a CPU raster
//! surface today and a future GPU (EGL/Ganesh) surface without change (ADR-0146 §3).
//!
//! wasm32 is out of scope: skia-safe does not target `wasm32-unknown-unknown`, so this
//! crate is never a member of the web build graph (web keeps vello / tiny-skia unchanged).

mod layer_compositor;
mod painter;
mod resource_cache;

use hayate_core::{render_scene_graph, SceneGraph};
use skia_safe::{surfaces, Canvas, Color4f, ColorType, ISize, ImageInfo, Surface};

use painter::SkiaPainter;
use resource_cache::PaintResourceCache;

pub use resource_cache::{SkiaResourceWorkCounts, SKIA_PAINT_RESOURCE_CACHE_BUDGET_BYTES};

pub use layer_compositor::{
    SkiaCompositeTarget, SkiaLayerCompositor, SkiaLayerPresenter, SkiaLayerRasterizer,
    SkiaLayerSurfaceFactory, SkiaLayerTexture, SkiaRasterLayerSurfaceFactory,
};

pub struct SkiaSceneRenderer {
    resources: PaintResourceCache,
}

impl Default for SkiaSceneRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl SkiaSceneRenderer {
    pub fn new() -> Self {
        Self {
            resources: PaintResourceCache::new(),
        }
    }

    pub fn resource_work_counts(&self) -> SkiaResourceWorkCounts {
        self.resources.work_counts()
    }

    /// `canvas` へ `graph` を描く。`canvas` の出自（CPU raster surface / 将来の GPU
    /// surface）は問わない — 渡された Canvas へ描くだけで、内部で surface を作ったり
    /// 覗いたりしない（ADR-0146 §3 の surface 非依存 painter）。
    pub fn render_scene(
        &mut self,
        graph: &SceneGraph,
        canvas: &Canvas,
        clear_color: [f32; 4],
        content_scale: f32,
    ) {
        self.render_scene_with_offset(graph, canvas, clear_color, content_scale, 0.0, 0.0);
    }

    /// [`Self::render_scene`] と同じ painter を使い、logical px の平行移動を DPI scale より前に
    /// 適用して描く。Native の safe-area 原点と scroll overscan 帯の raster を同じ座標契約へ
    /// 揃えるための surface 非依存 seam。
    pub fn render_scene_with_offset(
        &mut self,
        graph: &SceneGraph,
        canvas: &Canvas,
        clear_color: [f32; 4],
        content_scale: f32,
        offset_x: f32,
        offset_y: f32,
    ) {
        canvas.save();
        let [r, g, b, a] = clear_color;
        canvas.clear(Color4f::new(r, g, b, a));
        if content_scale != 1.0 {
            canvas.scale((content_scale, content_scale));
        }
        if offset_x != 0.0 || offset_y != 0.0 {
            canvas.translate((offset_x, offset_y));
        }
        let mut painter = SkiaPainter::new(canvas, &mut self.resources);
        render_scene_graph(graph, &mut painter);
        canvas.restore();
    }

    /// `device_origin` が texture pixel (0, 0) に来るよう、内容を上・左へずらして描く。
    pub fn render_scene_at(
        &mut self,
        graph: &SceneGraph,
        canvas: &Canvas,
        clear_color: [f32; 4],
        content_scale: f32,
        device_origin: (i32, i32),
    ) {
        self.render_scene_with_offset(
            graph,
            canvas,
            clear_color,
            content_scale,
            -(device_origin.0 as f32) / content_scale,
            -(device_origin.1 as f32) / content_scale,
        );
    }
}

/// CPU raster surface（RGBA8888・premultiplied alpha）を確保する。導入時の結線は
/// desktop の CPU raster（ADR-0146 §3）— Render Host（issue #801）がこれで surface を
/// 作り、`render_scene` に `surface.canvas()` を渡す。
pub fn new_raster_surface(width: i32, height: i32) -> Option<Surface> {
    let info = ImageInfo::new(
        ISize::new(width.max(1), height.max(1)),
        ColorType::RGBA8888,
        skia_safe::AlphaType::Premul,
        None,
    );
    surfaces::raster(&info, None, None)
}

/// `surface` の現在のピクセルを RGBA8888（作成時と同じ premultiplied byte order）で
/// CPU へ読み戻す。Render Host のプレゼンテーションおよびテストの golden 比較で使う
/// surface 補助（ADR-0054 の「公開可: surface/present 補助」に相当）。
pub fn read_rgba(surface: &mut Surface) -> Vec<u8> {
    let info = surface.image_info();
    let height = info.height();
    let row_bytes = info.min_row_bytes();
    let mut buf = vec![0u8; row_bytes * height as usize];
    surface.read_pixels(&info, &mut buf, row_bytes, (0, 0));
    buf
}
