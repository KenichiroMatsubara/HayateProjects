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
use hayate_core::{LayerRasterBounds, LayerScene, LayerTopology, SceneRead, SceneSnapshot};
use hayate_layer_compositor::{
    tunables, CompositeQuad, GpuBudget, LayerCompositor, LayerPresentation,
    LayerPresentationAdapter, LayerPresentationFrame, LayerRasterizer, LayerResourceId,
    LayerResourcePlane, PlacementPlan, RasterBand, RasterJob, RasterJobKind,
    RenderResourceBudgetPolicy, RenderResourceKey, RenderResourceResidency, ResidencyEvent,
    ResidencyStats, ResourceDomain, ScrollLayerExtent, ScrollLayerGeometry,
};
use skia_safe::{Color4f, Image, Matrix, Rect, Surface};

use crate::{new_raster_surface, SkiaResourceWorkCounts, SkiaSceneRenderer};

/// レイヤキャッシュ面は透明クリアで raster する（背景は合成の clear / root レイヤが持つ）。
const TRANSPARENT: [f32; 4] = [0.0, 0.0, 0.0, 0.0];

/// Layer cache 用 `SkSurface` の出自を差し替える seam。CPU raster と Ganesh は同じ
/// ScenePainter / raster planning を通り、この adapter だけが surface allocation を担う。
pub trait SkiaLayerSurfaceFactory {
    fn create_layer_surface(&mut self, width: i32, height: i32) -> Result<Surface, String>;
}

/// CPU raster fallback の layer-surface adapter。
pub struct SkiaRasterLayerSurfaceFactory;

impl SkiaLayerSurfaceFactory for SkiaRasterLayerSurfaceFactory {
    fn create_layer_surface(&mut self, width: i32, height: i32) -> Result<Surface, String> {
        new_raster_surface(width, height)
            .ok_or_else(|| format!("skia raster layer surface {width}x{height}"))
    }
}

/// Skia レイヤ rasterizer（[`LayerRasterizer`] の Skia 実装）。キャッシュ面は device
/// サイズの `SkImage`（`content_scale` を掛けて raster するので wgpu 経路の surface
/// サイズ texture と対応・tiny-skia の `Pixmap` キャッシュと同型）。
pub struct SkiaLayerRasterizer {
    width: i32,
    height: i32,
    content_scale: f32,
    plane: LayerResourcePlane,
    resources: RenderResourceResidency<SkiaLayerTexture>,
    renderer: SkiaSceneRenderer,
}

/// A layer-local Skia image plus the device-pixel scene origin represented by pixel (0, 0).
pub struct SkiaLayerTexture {
    image: Image,
    device_origin: (i32, i32),
}

impl SkiaLayerTexture {
    pub fn width(&self) -> i32 {
        self.image.width()
    }

    pub fn height(&self) -> i32 {
        self.image.height()
    }

    pub fn device_origin(&self) -> (i32, i32) {
        self.device_origin
    }
}

impl SkiaLayerRasterizer {
    pub fn new(width: u32, height: u32, content_scale: f32) -> Self {
        Self::new_for_plane(width, height, content_scale, LayerResourcePlane::Content)
    }

    pub fn new_for_plane(
        width: u32,
        height: u32,
        content_scale: f32,
        plane: LayerResourcePlane,
    ) -> Self {
        let policy = RenderResourceBudgetPolicy::for_device(
            hayate_layer_compositor::ResourceBudgetInputs::new(
                hayate_layer_compositor::DeviceMemoryClass::Balanced,
                width,
                height,
            ),
        );
        Self {
            width: (width.max(1)) as i32,
            height: (height.max(1)) as i32,
            content_scale,
            plane,
            resources: RenderResourceResidency::new(policy),
            renderer: SkiaSceneRenderer::new(),
        }
    }

    fn resource_key(&self, layer: ElementId) -> RenderResourceKey {
        RenderResourceKey::Layer(LayerResourceId::new(layer, self.plane))
    }

    /// サーフェスサイズ / DPR 変更。キャッシュ面は全部作り直しになる（呼び元は planner も invalidate）。
    pub fn resize(&mut self, width: u32, height: u32, content_scale: f32) {
        self.width = width.max(1) as i32;
        self.height = height.max(1) as i32;
        self.content_scale = content_scale;
        self.resources.clear_domain(ResourceDomain::Gpu);
    }

    fn band_device_height(&self, height: f32) -> i32 {
        (height * self.content_scale).ceil().max(1.0) as i32
    }

    fn device_region(&self, bounds: LayerRasterBounds) -> ((i32, i32), i32, i32) {
        let scale = self.content_scale;
        let left = (bounds.origin_x * scale).floor() as i32;
        let top = (bounds.origin_y * scale).floor() as i32;
        let right = ((bounds.origin_x + bounds.width) * scale).ceil() as i32;
        let bottom = ((bounds.origin_y + bounds.height) * scale).ceil() as i32;
        ((left, top), (right - left).max(1), (bottom - top).max(1))
    }

    pub fn scroll_band_bytes(&self, band: ScrollLayerExtent) -> u64 {
        u64::from(self.width as u32)
            * u64::from(self.band_device_height(band.height) as u32)
            * tunables::BYTES_PER_PIXEL
    }

    pub fn resource_work_counts(&self) -> SkiaResourceWorkCounts {
        self.renderer.resource_work_counts()
    }

    pub fn configure_resource_residency(&mut self, policy: RenderResourceBudgetPolicy) {
        self.renderer.configure_resource_residency(policy);
        self.resources.set_policy(policy);
    }

    pub fn handle_resource_lifecycle(&mut self, event: ResidencyEvent) {
        self.renderer.handle_resource_lifecycle(event);
        self.resources.handle_lifecycle(event);
    }

    pub fn residency_stats(&self) -> ResidencyStats {
        self.resources.stats()
    }

    pub fn rasterize_with_bounds(
        &mut self,
        layer: ElementId,
        scene: &(impl SceneRead + ?Sized),
        bounds: LayerRasterBounds,
        band: Option<RasterBand>,
    ) -> Result<(), String> {
        self.rasterize_with_bounds_and_layer_surface_factory(
            &mut SkiaRasterLayerSurfaceFactory,
            layer,
            scene,
            bounds,
            band,
        )
    }

    pub fn rasterize_with_bounds_and_layer_surface_factory(
        &mut self,
        factory: &mut dyn SkiaLayerSurfaceFactory,
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
        let mut surface = factory.create_layer_surface(width, height)?;
        self.renderer.render_scene_at(
            scene,
            surface.canvas(),
            TRANSPARENT,
            self.content_scale,
            device_origin,
        );
        let bytes = width as u64 * height as u64 * tunables::BYTES_PER_PIXEL;
        self.resources.insert_retained(
            ResourceDomain::Gpu,
            self.resource_key(layer),
            SkiaLayerTexture {
                image: surface.image_snapshot(),
                device_origin,
            },
            bytes,
            width as u64 * height as u64,
        );
        Ok(())
    }

    pub fn texture_bytes(&self, layer: ElementId) -> u64 {
        self.resources
            .peek(ResourceDomain::Gpu, self.resource_key(layer))
            .map_or(0, |texture| {
                texture.width() as u64
                    * texture.height() as u64
                    * hayate_layer_compositor::tunables::BYTES_PER_PIXEL
            })
    }

    pub fn rasterize_with_layer_surface_factory(
        &mut self,
        factory: &mut dyn SkiaLayerSurfaceFactory,
        layer: ElementId,
        scene: &(impl SceneRead + ?Sized),
        band: Option<RasterBand>,
    ) -> Result<(), String> {
        let (height, origin_y) = band
            .map(|band| (self.band_device_height(band.height), band.origin_y))
            .unwrap_or((self.height, 0.0));
        let mut surface = factory.create_layer_surface(self.width, height)?;
        self.renderer.render_scene_at(
            scene,
            surface.canvas(),
            TRANSPARENT,
            self.content_scale,
            (0, (origin_y * self.content_scale).floor() as i32),
        );
        let bytes = self.width as u64 * height as u64 * tunables::BYTES_PER_PIXEL;
        self.resources.insert_retained(
            ResourceDomain::Gpu,
            self.resource_key(layer),
            SkiaLayerTexture {
                image: surface.image_snapshot(),
                device_origin: (0, (origin_y * self.content_scale).floor() as i32),
            },
            bytes,
            self.width as u64 * height as u64,
        );
        Ok(())
    }
}

impl LayerRasterizer for SkiaLayerRasterizer {
    type Texture = SkiaLayerTexture;

    fn rasterize(
        &mut self,
        layer: ElementId,
        scene: &LayerScene,
        band: Option<RasterBand>,
    ) -> Result<(), String> {
        self.rasterize_with_layer_surface_factory(
            &mut SkiaRasterLayerSurfaceFactory,
            layer,
            scene,
            band,
        )
    }

    fn texture(&self, layer: ElementId) -> Option<&SkiaLayerTexture> {
        self.resources
            .peek(ResourceDomain::Gpu, self.resource_key(layer))
    }

    fn texture_bytes_per_layer(&self) -> u64 {
        u64::from(self.width as u32)
            * u64::from(self.height as u32)
            * hayate_layer_compositor::tunables::BYTES_PER_PIXEL
    }

    fn discard(&mut self, layer: ElementId) {
        self.resources
            .remove(ResourceDomain::Gpu, self.resource_key(layer));
    }

    fn discard_all(&mut self) {
        self.resources.clear_domain(ResourceDomain::Gpu);
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
    type Texture = SkiaLayerTexture;
    type Target = SkiaCompositeTarget;

    fn composite(
        &mut self,
        target: &mut SkiaCompositeTarget,
        quads: &[CompositeQuad<'_, SkiaLayerTexture>],
    ) -> Result<(), String> {
        let s = self.content_scale;
        let canvas = target.surface.canvas();
        canvas.save();
        let [r, g, b, a] = target.clear;
        canvas.clear(Color4f::new(r, g, b, a));
        for quad in quads {
            canvas.save();
            // placement clip は Core Layer Topology が target/device 空間へ写した矩形。
            // quad transform より先に固定しないと、composite-only scroll の平行移動で clip まで
            // 動いてしまい、viewport の途中から内容が欠ける。
            if let Some([x, y, w, h]) = quad.clip {
                canvas.clip_rect(
                    Rect::from_xywh(x * s, y * s, w * s, h * s),
                    None,
                    Some(true),
                );
            }
            canvas.concat(&self.device_matrix(quad.transform));
            let mut paint = skia_safe::Paint::default();
            paint.set_alpha_f(quad.opacity.clamp(0.0, 1.0));
            canvas.draw_image(
                &quad.texture.image,
                quad.texture.device_origin,
                Some(&paint),
            );
            canvas.restore();
        }
        canvas.restore();
        Ok(())
    }
}

/// skia-safe の per-layer raster/cache/composite を Native platform から共通利用する presenter。
/// 最終 `Surface` の出自だけを platform が決め、CPU raster と Ganesh/EGL は同じ shared
/// [`LayerPresentation`] transaction を通る。
pub struct SkiaLayerPresenter {
    presentation: LayerPresentation,
    rasterizer: SkiaLayerRasterizer,
    chrome_rasterizer: SkiaLayerRasterizer,
    compositor: SkiaLayerCompositor,
    width: u32,
    height: u32,
    content_scale: f32,
    last_raster_count: usize,
    last_raster_pixels: u64,
}

impl SkiaLayerPresenter {
    pub fn new(width: u32, height: u32, content_scale: f32) -> Self {
        let content_scale = content_scale.max(1.0);
        Self {
            presentation: LayerPresentation::new(),
            rasterizer: SkiaLayerRasterizer::new(width, height, content_scale),
            chrome_rasterizer: SkiaLayerRasterizer::new_for_plane(
                width,
                height,
                content_scale,
                LayerResourcePlane::ScrollChrome,
            ),
            compositor: SkiaLayerCompositor::new(content_scale),
            width: width.max(1),
            height: height.max(1),
            content_scale,
            last_raster_count: 0,
            last_raster_pixels: 0,
        }
    }

    /// 直近の [`Self::present`] で更新した content cache layer 数。固定 chrome の更新は
    /// composite-only scroll の主要 work-count に含めない。
    pub fn last_raster_count(&self) -> usize {
        self.last_raster_count
    }

    /// Device pixels rasterized into content caches by the most recent present.
    pub fn last_raster_pixels(&self) -> u64 {
        self.last_raster_pixels
    }

    /// Content cache bytes recorded in the shared budget ledger using actual texture dimensions.
    pub fn cached_texture_bytes(&self) -> u64 {
        self.presentation.cached_bytes()
    }

    pub fn configure_resource_residency(&mut self, policy: RenderResourceBudgetPolicy) {
        self.rasterizer.configure_resource_residency(policy);
        self.chrome_rasterizer.configure_resource_residency(policy);
    }

    pub fn handle_resource_lifecycle(&mut self, event: ResidencyEvent) {
        self.rasterizer.handle_resource_lifecycle(event);
        self.chrome_rasterizer.handle_resource_lifecycle(event);
        if matches!(
            event,
            ResidencyEvent::SurfaceLost | ResidencyEvent::ContextLost | ResidencyEvent::Shutdown
        ) {
            self.presentation.invalidate();
        }
    }

    pub fn resize(&mut self, width: u32, height: u32, content_scale: f32) {
        let content_scale = content_scale.max(1.0);
        if width == 0
            || height == 0
            || (width == self.width && height == self.height && content_scale == self.content_scale)
        {
            return;
        }
        self.width = width;
        self.height = height;
        self.content_scale = content_scale;
        self.rasterizer.resize(width, height, content_scale);
        self.chrome_rasterizer.resize(width, height, content_scale);
        self.compositor.set_content_scale(content_scale);
        self.presentation.invalidate();
    }

    #[allow(clippy::too_many_arguments)]
    pub fn present(
        &mut self,
        snapshot: &SceneSnapshot,
        topology: &LayerTopology,
        scroll_geometry: &HashMap<ElementId, ScrollLayerGeometry>,
        clear: [f32; 4],
        scene_origin: (f32, f32),
        budget: GpuBudget,
        surface: Surface,
    ) -> Result<Surface, String> {
        self.present_with_layer_surface_factory(
            snapshot,
            topology,
            scroll_geometry,
            clear,
            scene_origin,
            budget,
            &mut SkiaRasterLayerSurfaceFactory,
            surface,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn present_with_layer_surface_factory(
        &mut self,
        snapshot: &SceneSnapshot,
        topology: &LayerTopology,
        scroll_geometry: &HashMap<ElementId, ScrollLayerGeometry>,
        clear: [f32; 4],
        scene_origin: (f32, f32),
        budget: GpuBudget,
        factory: &mut dyn SkiaLayerSurfaceFactory,
        surface: Surface,
    ) -> Result<Surface, String> {
        self.last_raster_count = 0;
        self.last_raster_pixels = 0;
        if topology.paint_order().is_empty() {
            let mut surface = surface;
            SkiaSceneRenderer::new().render_scene_with_offset(
                snapshot,
                surface.canvas(),
                clear,
                self.content_scale,
                scene_origin.0,
                scene_origin.1,
            );
            return Ok(surface);
        }

        let mut target = Some(surface);
        let mut adapter = SkiaLayerPresentationAdapter {
            rasterizer: &mut self.rasterizer,
            chrome_rasterizer: &mut self.chrome_rasterizer,
            compositor: &mut self.compositor,
            factory,
            target: &mut target,
            clear,
            scene_origin,
            content_scale: self.content_scale,
            last_raster_count: &mut self.last_raster_count,
            last_raster_pixels: &mut self.last_raster_pixels,
        };
        self.presentation
            .present(
                LayerPresentationFrame {
                    snapshot,
                    topology,
                    scroll_geometry,
                },
                &mut adapter,
            )
            .map_err(|error| format!("layer presentation: {error:?}"))?;
        self.presentation.enforce_budget(budget, &mut adapter);
        target
            .ok_or_else(|| "Skia layer presentation did not return its target surface".to_string())
    }
}

/// Skia resource adapter for the shared layer-presentation transaction. It owns SkSurface
/// transfer, layer-surface allocation, raster caches and final composite; shared planning and
/// ledger mutation deliberately remain outside this module.
pub struct SkiaLayerPresentationAdapter<'a> {
    rasterizer: &'a mut SkiaLayerRasterizer,
    chrome_rasterizer: &'a mut SkiaLayerRasterizer,
    compositor: &'a mut SkiaLayerCompositor,
    factory: &'a mut dyn SkiaLayerSurfaceFactory,
    target: &'a mut Option<Surface>,
    clear: [f32; 4],
    scene_origin: (f32, f32),
    content_scale: f32,
    last_raster_count: &'a mut usize,
    last_raster_pixels: &'a mut u64,
}

impl LayerPresentationAdapter for SkiaLayerPresentationAdapter<'_> {
    type Error = String;

    fn rasterize(&mut self, job: &RasterJob<'_>) -> Result<u64, Self::Error> {
        if job.kind == RasterJobKind::ScrollChrome
            && !job.repaint
            && self.chrome_rasterizer.texture(job.layer).is_some()
        {
            return Ok(self.rasterizer.texture_bytes(job.layer)
                + self.chrome_rasterizer.texture_bytes(job.layer));
        }
        let rasterizer = match job.kind {
            RasterJobKind::Content => &mut *self.rasterizer,
            RasterJobKind::ScrollChrome => &mut *self.chrome_rasterizer,
        };
        match job.bounds {
            Some(bounds) => rasterizer.rasterize_with_bounds_and_layer_surface_factory(
                self.factory,
                job.layer,
                job.scene,
                bounds,
                job.band,
            )?,
            None => rasterizer.rasterize_with_layer_surface_factory(
                self.factory,
                job.layer,
                job.scene,
                job.band,
            )?,
        }
        let bytes = rasterizer.texture_bytes(job.layer);
        if job.kind == RasterJobKind::Content {
            *self.last_raster_count += 1;
            *self.last_raster_pixels += bytes / tunables::BYTES_PER_PIXEL;
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
        let surface = self
            .target
            .take()
            .ok_or_else(|| "Skia composite target was already taken".to_string())?;
        let mut target = SkiaCompositeTarget {
            surface,
            clear: self.clear,
        };
        let canvas = target.surface.canvas();
        canvas.save();
        canvas.translate((
            self.scene_origin.0 * self.content_scale,
            self.scene_origin.1 * self.content_scale,
        ));
        let result = self.compositor.composite(&mut target, &quads);
        target.surface.canvas().restore();
        *self.target = Some(target.surface);
        result
    }

    fn discard(&mut self, layers: &[ElementId]) {
        for &layer in layers {
            self.rasterizer.discard(layer);
            self.chrome_rasterizer.discard(layer);
        }
    }
}
