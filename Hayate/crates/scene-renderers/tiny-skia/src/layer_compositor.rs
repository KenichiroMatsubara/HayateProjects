//! CPU（tiny-skia）レイヤ rasterizer / compositor（#636・ADR-0125 backend 半分の web CPU 経路）。
//!
//! wgpu 経路（#633）と同じ [`LayerRasterizer`] / [`LayerCompositor`] を CPU で実装する。キャッシュ面は
//! `Pixmap`、合成は `draw_pixmap`（transform / opacity / 軸並行 clip 付き quad）。同一の planning
//! （[`hayate_layer_compositor::PresentPlanner`]）を trait 実装の差し替えだけで受けるので、
//! composite-only フレームでは全面 `render_scene` を起動せず、キャッシュ Pixmap を合成するだけになる
//! （Android Chrome の `?renderer=tiny-skia` でスクロール/transform フレームが全画面 CPU 再ラスタから
//! キャッシュ面合成に変わる）。分解の正しさは `tests/layer_compositor.rs` のピクセルパリティで固定する。
//!
//! 座標系: キャッシュ Pixmap は device px（`content_scale` を掛けて raster）。placement の transform
//! は logical scene 座標なので、合成時に `scale(s) ∘ placement ∘ scale(1/s)` へ写して device px の
//! draw 変換にする（texture 自身は device 解像度なので線形部は素通り・translate だけ ×s される）。

use std::collections::HashMap;

use hayate_core::element::id::ElementId;
use hayate_core::SceneGraph;
use hayate_layer_compositor::{CompositeQuad, LayerCompositor, LayerRasterizer, RasterBand};
use tiny_skia::{
    Color, FillRule, Mask, PathBuilder, Pixmap, PixmapPaint, Rect, Transform,
};

use crate::TinySkiaSceneRenderer;

/// レイヤキャッシュ面は透明クリアで raster する（背景は合成の clear / root レイヤが持つ）。
const TRANSPARENT: [f32; 4] = [0.0, 0.0, 0.0, 0.0];

fn to_premultiplied_color(c: [f32; 4]) -> Color {
    let [r, g, b, a] = c;
    Color::from_rgba(
        r.clamp(0.0, 1.0),
        g.clamp(0.0, 1.0),
        b.clamp(0.0, 1.0),
        a.clamp(0.0, 1.0),
    )
    .unwrap_or(Color::TRANSPARENT)
}

/// CPU レイヤ rasterizer（`LayerRasterizer` の tiny-skia 実装）。キャッシュ面は device サイズの
/// `Pixmap`（`content_scale` を掛けて raster するので wgpu 経路の surface サイズ texture と対応）。
pub struct TinySkiaLayerRasterizer {
    width: u32,
    height: u32,
    content_scale: f32,
    textures: HashMap<ElementId, Pixmap>,
}

impl TinySkiaLayerRasterizer {
    pub fn new(width: u32, height: u32, content_scale: f32) -> Self {
        Self {
            width: width.max(1),
            height: height.max(1),
            content_scale,
            textures: HashMap::new(),
        }
    }

    /// サーフェスサイズ / DPR 変更。キャッシュ面は全部作り直しになる（呼び元は planner も invalidate）。
    pub fn resize(&mut self, width: u32, height: u32, content_scale: f32) {
        self.width = width.max(1);
        self.height = height.max(1);
        self.content_scale = content_scale;
        self.textures.clear();
    }
}

impl LayerRasterizer for TinySkiaLayerRasterizer {
    type Texture = Pixmap;

    fn rasterize(
        &mut self,
        layer: ElementId,
        scene: &SceneGraph,
        // #707 (ADR-0127): scroll-band sizing is vello-only for now — tiny-skia's CPU `Pixmap`
        // is cheap to resize per-layer but that optimization is out of scope for this issue, so
        // the band is accepted (to satisfy the shared trait) and intentionally ignored; every
        // layer still gets a full-surface `Pixmap`, unchanged from before this parameter existed.
        _band: Option<RasterBand>,
    ) -> Result<(), String> {
        let mut pixmap = Pixmap::new(self.width, self.height)
            .ok_or_else(|| format!("tiny-skia layer pixmap {}x{}", self.width, self.height))?;
        // 透明クリアのキャッシュ面へ抽出済み sub-scene を raster（content_scale は render_scene が適用）。
        TinySkiaSceneRenderer::new().render_scene(scene, &mut pixmap, TRANSPARENT, self.content_scale);
        self.textures.insert(layer, pixmap);
        Ok(())
    }

    fn texture(&self, layer: ElementId) -> Option<&Pixmap> {
        self.textures.get(&layer)
    }

    fn texture_bytes_per_layer(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height) * hayate_layer_compositor::tunables::BYTES_PER_PIXEL
    }

    fn discard(&mut self, layer: ElementId) {
        self.textures.remove(&layer);
    }

    fn discard_all(&mut self) {
        self.textures.clear();
    }
}

/// CPU 合成先（1 フレーム分の Pixmap ＋ clear color）。composite は冒頭で clear→各 quad を
/// `draw_pixmap` する（root レイヤは不透明で全面を覆うので、それより前に clear がある前提で一致する）。
pub struct TinySkiaCompositeTarget {
    pub pixmap: Pixmap,
    pub clear: [f32; 4],
}

/// CPU quad compositor（`LayerCompositor` の tiny-skia 実装）。キャッシュ Pixmap を placement
/// （transform / opacity / 軸並行 clip）で `draw_pixmap` 合成する。合成に full `render_scene` は使わない。
pub struct TinySkiaLayerCompositor {
    content_scale: f32,
}

impl TinySkiaLayerCompositor {
    pub fn new(content_scale: f32) -> Self {
        Self { content_scale }
    }

    pub fn set_content_scale(&mut self, content_scale: f32) {
        self.content_scale = content_scale;
    }

    /// logical placement 変換 → device px の draw 変換 `scale(s) ∘ placement ∘ scale(1/s)`。
    fn device_transform(&self, t: [f64; 6]) -> Transform {
        let s = self.content_scale as f64;
        // 線形部（a,b,c,d）は scale が相殺、translate（e,f）だけ ×s。
        Transform::from_row(
            t[0] as f32,
            t[1] as f32,
            t[2] as f32,
            t[3] as f32,
            (t[4] * s) as f32,
            (t[5] * s) as f32,
        )
    }

    /// logical clip 矩形 → device px の Mask。
    fn device_clip_mask(&self, clip: [f32; 4], width: u32, height: u32) -> Option<Mask> {
        let s = self.content_scale;
        let [x, y, w, h] = clip;
        let rect = Rect::from_xywh(x * s, y * s, w * s, h * s)?;
        let mut mask = Mask::new(width, height)?;
        let path = PathBuilder::from_rect(rect);
        mask.fill_path(&path, FillRule::Winding, true, Transform::identity());
        Some(mask)
    }
}

impl LayerCompositor for TinySkiaLayerCompositor {
    type Texture = Pixmap;
    type Target = TinySkiaCompositeTarget;

    fn composite(
        &mut self,
        target: &mut TinySkiaCompositeTarget,
        quads: &[CompositeQuad<'_, Pixmap>],
    ) -> Result<(), String> {
        let (width, height) = (target.pixmap.width(), target.pixmap.height());
        target.pixmap.fill(to_premultiplied_color(target.clear));
        for quad in quads {
            let paint = PixmapPaint {
                opacity: quad.opacity.clamp(0.0, 1.0),
                ..PixmapPaint::default()
            };
            let transform = self.device_transform(quad.transform);
            let mask = quad
                .clip
                .and_then(|clip| self.device_clip_mask(clip, width, height));
            target.pixmap.draw_pixmap(
                0,
                0,
                quad.texture.as_ref(),
                &paint,
                transform,
                mask.as_ref(),
            );
        }
        Ok(())
    }
}
