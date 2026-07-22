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

use hayate_core::element::id::ElementId;
use hayate_core::{LayerRasterBounds, LayerScene, SceneRead};
use hayate_layer_compositor::{
    CompositeQuad, DeviceMemoryClass, LayerCompositor, LayerPresentationAdapter, LayerRasterizer,
    LayerResourceId, LayerResourcePlane, PlacementPlan, RasterBand, RasterJob, RasterJobKind,
    RenderResourceBudgetPolicy, RenderResourceKey, RenderResourceResidency, ResidencyEvent,
    ResidencyStats, ResourceBudgetInputs, ResourceDomain,
};
use tiny_skia::{Color, FillRule, Mask, PathBuilder, Pixmap, PixmapPaint, Point, Rect, Transform};

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

/// CPU レイヤ rasterizer（`LayerRasterizer` の tiny-skia 実装）。root は device surface サイズ、
/// 非 root は Core の [`LayerRasterBounds`] を device px へ外向き丸めした実寸 `Pixmap` を持つ。
pub struct TinySkiaLayerRasterizer {
    width: u32,
    height: u32,
    content_scale: f32,
    plane: LayerResourcePlane,
    resources: RenderResourceResidency<TinySkiaLayerTexture>,
}

/// A layer-local Pixmap plus the device-pixel scene origin represented by pixel (0, 0).
/// Rasterization subtracts this origin; compositing restores it before applying placement.
pub struct TinySkiaLayerTexture {
    pixmap: Pixmap,
    device_origin: (i32, i32),
}

impl TinySkiaLayerTexture {
    pub fn width(&self) -> u32 {
        self.pixmap.width()
    }

    pub fn height(&self) -> u32 {
        self.pixmap.height()
    }

    pub fn device_origin(&self) -> (i32, i32) {
        self.device_origin
    }

    pub fn pixmap(&self) -> &Pixmap {
        &self.pixmap
    }
}

impl TinySkiaLayerRasterizer {
    pub fn new(width: u32, height: u32, content_scale: f32) -> Self {
        Self::new_for_plane(width, height, content_scale, LayerResourcePlane::Content)
    }

    pub fn new_for_plane(
        width: u32,
        height: u32,
        content_scale: f32,
        plane: LayerResourcePlane,
    ) -> Self {
        Self {
            width: width.max(1),
            height: height.max(1),
            content_scale,
            plane,
            resources: RenderResourceResidency::new(RenderResourceBudgetPolicy::for_device(
                ResourceBudgetInputs::new(DeviceMemoryClass::Balanced, width, height),
            )),
        }
    }

    fn resource_key(&self, layer: ElementId) -> RenderResourceKey {
        RenderResourceKey::Layer(LayerResourceId::new(layer, self.plane))
    }

    pub fn configure_resource_residency(&mut self, policy: RenderResourceBudgetPolicy) {
        self.resources.set_policy(policy);
    }

    pub fn handle_resource_lifecycle(&mut self, event: ResidencyEvent) {
        self.resources.handle_lifecycle(event);
    }

    pub fn residency_stats(&self) -> ResidencyStats {
        self.resources.stats()
    }

    /// サーフェスサイズ / DPR 変更。キャッシュ面は全部作り直しになる（呼び元は planner も invalidate）。
    pub fn resize(&mut self, width: u32, height: u32, content_scale: f32) {
        self.width = width.max(1);
        self.height = height.max(1);
        self.content_scale = content_scale;
        self.resources.clear_domain(ResourceDomain::Cpu);
    }

    fn device_region(&self, bounds: LayerRasterBounds) -> ((i32, i32), u32, u32) {
        let scale = self.content_scale;
        let left = (bounds.origin_x * scale).floor() as i32;
        let top = (bounds.origin_y * scale).floor() as i32;
        let right = ((bounds.origin_x + bounds.width) * scale).ceil() as i32;
        let bottom = ((bounds.origin_y + bounds.height) * scale).ceil() as i32;
        (
            (left, top),
            (right - left).max(1) as u32,
            (bottom - top).max(1) as u32,
        )
    }

    /// Raster one extracted layer into Core's conservative logical bounds.
    pub fn rasterize_with_bounds(
        &mut self,
        layer: ElementId,
        scene: &(impl SceneRead + ?Sized),
        mut bounds: LayerRasterBounds,
        band: Option<RasterBand>,
    ) -> Result<(), String> {
        debug_assert_eq!(bounds.layer, layer);
        if let Some(band) = band {
            bounds.origin_y = band.origin_y;
            bounds.height = band.height;
        }
        let (device_origin, width, height) = self.device_region(bounds);
        let mut pixmap = Pixmap::new(width, height)
            .ok_or_else(|| format!("tiny-skia layer pixmap {width}x{height}"))?;
        TinySkiaSceneRenderer::new().render_scene_at(
            scene,
            &mut pixmap,
            TRANSPARENT,
            self.content_scale,
            device_origin,
        );
        let bytes = u64::from(width)
            * u64::from(height)
            * hayate_layer_compositor::tunables::BYTES_PER_PIXEL;
        self.resources.insert_retained(
            ResourceDomain::Cpu,
            self.resource_key(layer),
            TinySkiaLayerTexture {
                pixmap,
                device_origin,
            },
            bytes,
            u64::from(width) * u64::from(height),
        );
        Ok(())
    }

    pub fn texture_bytes(&self, layer: ElementId) -> u64 {
        self.resources
            .peek(ResourceDomain::Cpu, self.resource_key(layer))
            .map_or(0, |texture| {
                u64::from(texture.width())
                    * u64::from(texture.height())
                    * hayate_layer_compositor::tunables::BYTES_PER_PIXEL
            })
    }
}

impl LayerRasterizer for TinySkiaLayerRasterizer {
    type Texture = TinySkiaLayerTexture;

    fn rasterize(
        &mut self,
        layer: ElementId,
        scene: &LayerScene,
        // Compatibility entry point for root/unbounded callers. The production tiny-skia
        // present path uses `rasterize_with_bounds`, including scroll bands.
        _band: Option<RasterBand>,
    ) -> Result<(), String> {
        let mut pixmap = Pixmap::new(self.width, self.height)
            .ok_or_else(|| format!("tiny-skia layer pixmap {}x{}", self.width, self.height))?;
        // 透明クリアのキャッシュ面へ抽出済み sub-scene を raster（content_scale は render_scene が適用）。
        TinySkiaSceneRenderer::new().render_scene(
            scene,
            &mut pixmap,
            TRANSPARENT,
            self.content_scale,
        );
        let bytes = u64::from(self.width)
            * u64::from(self.height)
            * hayate_layer_compositor::tunables::BYTES_PER_PIXEL;
        self.resources.insert_retained(
            ResourceDomain::Cpu,
            self.resource_key(layer),
            TinySkiaLayerTexture {
                pixmap,
                device_origin: (0, 0),
            },
            bytes,
            u64::from(self.width) * u64::from(self.height),
        );
        Ok(())
    }

    fn texture(&self, layer: ElementId) -> Option<&TinySkiaLayerTexture> {
        self.resources
            .peek(ResourceDomain::Cpu, self.resource_key(layer))
    }

    fn texture_bytes_per_layer(&self) -> u64 {
        u64::from(self.width)
            * u64::from(self.height)
            * hayate_layer_compositor::tunables::BYTES_PER_PIXEL
    }

    fn discard(&mut self, layer: ElementId) {
        self.resources
            .remove(ResourceDomain::Cpu, self.resource_key(layer));
    }

    fn discard_all(&mut self) {
        self.resources.clear_domain(ResourceDomain::Cpu);
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

    /// A bounded texture wholly inside its clip needs no target-sized mask. This is the common
    /// layer-local case and avoids clearing/filling one full-surface mask per quad.
    fn clip_contains_texture(
        &self,
        clip: [f32; 4],
        texture: &TinySkiaLayerTexture,
        transform: Transform,
    ) -> bool {
        let (x, y) = texture.device_origin;
        let x1 = x as f32 + texture.width() as f32;
        let y1 = y as f32 + texture.height() as f32;
        let corners = [
            (x as f32, y as f32),
            (x1, y as f32),
            (x as f32, y1),
            (x1, y1),
        ];
        let [cx, cy, cw, ch] = clip;
        let (cx0, cy0, cx1, cy1) = (
            cx * self.content_scale,
            cy * self.content_scale,
            (cx + cw) * self.content_scale,
            (cy + ch) * self.content_scale,
        );
        corners.into_iter().all(|(px, py)| {
            let mut point = Point::from_xy(px, py);
            transform.map_point(&mut point);
            point.x >= cx0 && point.x <= cx1 && point.y >= cy0 && point.y <= cy1
        })
    }
}

impl LayerCompositor for TinySkiaLayerCompositor {
    type Texture = TinySkiaLayerTexture;
    type Target = TinySkiaCompositeTarget;

    fn composite(
        &mut self,
        target: &mut TinySkiaCompositeTarget,
        quads: &[CompositeQuad<'_, TinySkiaLayerTexture>],
    ) -> Result<(), String> {
        let (width, height) = (target.pixmap.width(), target.pixmap.height());
        target.pixmap.fill(to_premultiplied_color(target.clear));
        for quad in quads {
            let paint = PixmapPaint {
                opacity: quad.opacity.clamp(0.0, 1.0),
                ..PixmapPaint::default()
            };
            let transform = self.device_transform(quad.transform);
            let mask = quad.clip.and_then(|clip| {
                (!self.clip_contains_texture(clip, quad.texture, transform))
                    .then(|| self.device_clip_mask(clip, width, height))
                    .flatten()
            });
            target.pixmap.draw_pixmap(
                quad.texture.device_origin.0,
                quad.texture.device_origin.1,
                quad.texture.pixmap.as_ref(),
                &paint,
                transform,
                mask.as_ref(),
            );
        }
        Ok(())
    }
}

/// Adapter for the shared `LayerPresentation` transaction. It owns only tiny-skia resources;
/// planning, frame validation, cache ledger and commit ordering remain backend-independent.
pub struct TinySkiaLayerPresentationAdapter<'a> {
    pub rasterizer: &'a mut TinySkiaLayerRasterizer,
    pub chrome_rasterizer: &'a mut TinySkiaLayerRasterizer,
    pub compositor: &'a mut TinySkiaLayerCompositor,
    pub target: &'a mut Pixmap,
    pub clear: [f32; 4],
}

impl LayerPresentationAdapter for TinySkiaLayerPresentationAdapter<'_> {
    type Error = String;

    fn rasterize(&mut self, job: &RasterJob<'_>) -> Result<u64, Self::Error> {
        let rasterizer = match job.kind {
            RasterJobKind::Content => &mut *self.rasterizer,
            RasterJobKind::ScrollChrome => &mut *self.chrome_rasterizer,
        };
        if job.kind == RasterJobKind::ScrollChrome
            && !job.repaint
            && rasterizer.texture(job.layer).is_some()
        {
            return Ok(self.rasterizer.texture_bytes(job.layer)
                + self.chrome_rasterizer.texture_bytes(job.layer));
        }
        match job.bounds {
            Some(bounds) => {
                rasterizer.rasterize_with_bounds(job.layer, job.scene, bounds, job.band)?
            }
            None => rasterizer.rasterize(job.layer, job.scene, job.band)?,
        }
        Ok(self.rasterizer.texture_bytes(job.layer)
            + self.chrome_rasterizer.texture_bytes(job.layer))
    }

    fn composite(&mut self, plan: &PlacementPlan) -> Result<(), Self::Error> {
        let mut quads = Vec::with_capacity(plan.planes.len());
        for plane in &plan.planes {
            let texture = match plane.kind {
                RasterJobKind::Content => self.rasterizer.texture(plane.layer),
                RasterJobKind::ScrollChrome => self.chrome_rasterizer.texture(plane.layer),
            };
            if let Some(texture) = texture {
                quads.push(CompositeQuad {
                    layer: plane.layer,
                    transform: plane.transform,
                    opacity: 1.0,
                    clip: plane.clip,
                    texture,
                });
            }
        }
        let replacement = Pixmap::new(1, 1).expect("1x1 pixmap");
        let pixmap = std::mem::replace(self.target, replacement);
        let mut target = TinySkiaCompositeTarget {
            pixmap,
            clear: self.clear,
        };
        let result = self.compositor.composite(&mut target, &quads);
        *self.target = target.pixmap;
        result
    }

    fn discard(&mut self, layers: &[ElementId]) {
        for &layer in layers {
            self.rasterizer.discard(layer);
            self.chrome_rasterizer.discard(layer);
        }
    }
}
