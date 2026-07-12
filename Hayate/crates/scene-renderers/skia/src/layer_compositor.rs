//! Skia レイヤ rasterizer / compositor（ADR-0125 backend 半分・ADR-0146 §6）。
//!
//! 既存の [`LayerRasterizer`] / [`LayerCompositor`] trait（backend 非依存、tiny-skia が雛形）を
//! Skia で実装する。キャッシュ面は `SkImage`（`SkSurface::imageSnapshot()`）、合成は
//! `Canvas::draw_image`。`SkSurface` 自体は Skia の参照カウントが非アトミックなため
//! `Send` ではない（rust-skia は明示的に `unsafe_send_sync!` した型だけ `Send` にする）。
//! `LayerRasterizer`/`LayerCompositor` は ADR-0128 の布石で実装型に `Send` を要求するので、
//! キャッシュには `SkSurface` ではなく `SkImage`（rust-skia が `Send`/`Sync` と宣言済み）を
//! 保持する — raster 直後に `image_snapshot()` でスナップショットを取ることで、tiny-skia の
//! `Pixmap` 保持と同じ「backend が texture を所有・再利用する」契約を保ったまま `Send` を満たす。

use std::collections::HashMap;

use hayate_core::element::id::ElementId;
use hayate_core::SceneGraph;
use hayate_layer_compositor::{CompositeQuad, LayerCompositor, LayerRasterizer, RasterBand};
use skia_safe::{Color4f, Image, Matrix, Rect, Surface};

use crate::{SkiaSceneRenderer, new_raster_surface};

/// レイヤキャッシュ面は透明クリアで raster する（背景は合成の clear / root レイヤが持つ）。
const TRANSPARENT: [f32; 4] = [0.0, 0.0, 0.0, 0.0];

/// Skia レイヤ rasterizer（[`LayerRasterizer`] の Skia 実装）。キャッシュ面は device
/// サイズの `SkImage`（`content_scale` を掛けて raster するので wgpu 経路の surface
/// サイズ texture と対応・tiny-skia の `Pixmap` キャッシュと同型）。
pub struct SkiaLayerRasterizer {
    width: i32,
    height: i32,
    content_scale: f32,
    textures: HashMap<ElementId, Image>,
}

impl SkiaLayerRasterizer {
    pub fn new(width: u32, height: u32, content_scale: f32) -> Self {
        Self {
            width: (width.max(1)) as i32,
            height: (height.max(1)) as i32,
            content_scale,
            textures: HashMap::new(),
        }
    }

    /// サーフェスサイズ / DPR 変更。キャッシュ面は全部作り直しになる（呼び元は planner も invalidate）。
    pub fn resize(&mut self, width: u32, height: u32, content_scale: f32) {
        self.width = width.max(1) as i32;
        self.height = height.max(1) as i32;
        self.content_scale = content_scale;
        self.textures.clear();
    }
}

impl LayerRasterizer for SkiaLayerRasterizer {
    type Texture = Image;

    fn rasterize(
        &mut self,
        layer: ElementId,
        scene: &SceneGraph,
        // #707 (ADR-0127) 同様、scroll-band サイジングは未対応（tiny-skia と同じ v1 スコープ
        // 外の選択）。band は trait 契約を満たすため受け取るが無視し、常にフルサーフェスで
        // raster する。
        _band: Option<RasterBand>,
    ) -> Result<(), String> {
        let mut surface = new_raster_surface(self.width, self.height)
            .ok_or_else(|| format!("skia layer surface {}x{}", self.width, self.height))?;
        SkiaSceneRenderer::new().render_scene(scene, surface.canvas(), TRANSPARENT, self.content_scale);
        self.textures.insert(layer, surface.image_snapshot());
        Ok(())
    }

    fn texture(&self, layer: ElementId) -> Option<&Image> {
        self.textures.get(&layer)
    }

    fn texture_bytes_per_layer(&self) -> u64 {
        u64::from(self.width as u32)
            * u64::from(self.height as u32)
            * hayate_layer_compositor::tunables::BYTES_PER_PIXEL
    }

    fn discard(&mut self, layer: ElementId) {
        self.textures.remove(&layer);
    }

    fn discard_all(&mut self) {
        self.textures.clear();
    }
}

/// Skia 合成先（1 フレーム分の `SkSurface` ＋ clear color）。composite は冒頭で
/// clear→各 quad を `draw_image` する。
pub struct SkiaCompositeTarget {
    pub surface: Surface,
    pub clear: [f32; 4],
}

/// Skia quad compositor（[`LayerCompositor`] の Skia 実装）。キャッシュ `SkImage` を
/// placement（transform / opacity / 軸並行 clip）で合成する。合成に `render_scene` の
/// フル walk は使わない — tiny-skia / wgpu 専用 compositor と同じ「合成は安い」契約
/// （ADR-0125 Decision 4）。
pub struct SkiaLayerCompositor {
    content_scale: f32,
}

impl SkiaLayerCompositor {
    pub fn new(content_scale: f32) -> Self {
        Self { content_scale }
    }

    pub fn set_content_scale(&mut self, content_scale: f32) {
        self.content_scale = content_scale;
    }

    /// logical placement 変換 → device px の draw 変換 `scale(s) ∘ placement ∘ scale(1/s)`
    /// （tiny-skia `device_transform` と同じ導出。線形部は scale が相殺、translate だけ ×s）。
    fn device_matrix(&self, t: [f64; 6]) -> Matrix {
        let s = self.content_scale as f64;
        Matrix::new_all(
            t[0] as f32,
            t[2] as f32,
            (t[4] * s) as f32,
            t[1] as f32,
            t[3] as f32,
            (t[5] * s) as f32,
            0.0,
            0.0,
            1.0,
        )
    }
}

impl LayerCompositor for SkiaLayerCompositor {
    type Texture = Image;
    type Target = SkiaCompositeTarget;

    fn composite(
        &mut self,
        target: &mut SkiaCompositeTarget,
        quads: &[CompositeQuad<'_, Image>],
    ) -> Result<(), String> {
        let s = self.content_scale;
        let canvas = target.surface.canvas();
        canvas.save();
        let [r, g, b, a] = target.clear;
        canvas.clear(Color4f::new(r, g, b, a));
        for quad in quads {
            canvas.save();
            canvas.concat(&self.device_matrix(quad.transform));
            if let Some([x, y, w, h]) = quad.clip {
                canvas.clip_rect(Rect::from_xywh(x * s, y * s, w * s, h * s), None, Some(true));
            }
            let mut paint = skia_safe::Paint::default();
            paint.set_alpha_f(quad.opacity.clamp(0.0, 1.0));
            canvas.draw_image(quad.texture, (0, 0), Some(&paint));
            canvas.restore();
        }
        canvas.restore();
        Ok(())
    }
}
