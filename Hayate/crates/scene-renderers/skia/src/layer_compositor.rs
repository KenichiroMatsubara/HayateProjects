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

use std::collections::{HashMap, HashSet};

use hayate_core::element::id::ElementId;
use hayate_core::{LayerRasterBounds, SceneGraph};
use hayate_layer_compositor::layer_scene::{
    collect_layer_placements, compose, extract_layer_scene, extract_root_scene,
    extract_scroll_chrome_scene, extract_scroll_layer_scene,
};
use hayate_layer_compositor::{
    tunables, CompositeQuad, GpuBudget, LayerCompositor, LayerRasterizer, PresentPlanner,
    RasterBand, ScrollLayerExtent, ScrollLayerGeometry,
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
    textures: HashMap<ElementId, SkiaLayerTexture>,
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
        Self {
            width: (width.max(1)) as i32,
            height: (height.max(1)) as i32,
            content_scale,
            textures: HashMap::new(),
            renderer: SkiaSceneRenderer::new(),
        }
    }

    /// サーフェスサイズ / DPR 変更。キャッシュ面は全部作り直しになる（呼び元は planner も invalidate）。
    pub fn resize(&mut self, width: u32, height: u32, content_scale: f32) {
        self.width = width.max(1) as i32;
        self.height = height.max(1) as i32;
        self.content_scale = content_scale;
        self.textures.clear();
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

    pub fn rasterize_with_bounds(
        &mut self,
        layer: ElementId,
        scene: &SceneGraph,
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
        scene: &SceneGraph,
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
        self.textures.insert(
            layer,
            SkiaLayerTexture {
                image: surface.image_snapshot(),
                device_origin,
            },
        );
        Ok(())
    }

    pub fn texture_bytes(&self, layer: ElementId) -> u64 {
        self.textures.get(&layer).map_or(0, |texture| {
            texture.width() as u64
                * texture.height() as u64
                * hayate_layer_compositor::tunables::BYTES_PER_PIXEL
        })
    }

    pub fn rasterize_with_layer_surface_factory(
        &mut self,
        factory: &mut dyn SkiaLayerSurfaceFactory,
        layer: ElementId,
        scene: &SceneGraph,
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
        self.textures.insert(
            layer,
            SkiaLayerTexture {
                image: surface.image_snapshot(),
                device_origin: (0, (origin_y * self.content_scale).floor() as i32),
            },
        );
        Ok(())
    }
}

impl LayerRasterizer for SkiaLayerRasterizer {
    type Texture = SkiaLayerTexture;

    fn rasterize(
        &mut self,
        layer: ElementId,
        scene: &SceneGraph,
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
            // placement clip は collect_layer_placements が target/device 空間へ写した矩形。
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
/// 最終 `Surface` の出自だけを platform が決め、CPU raster と Ganesh/EGL は同じ planning を通る。
pub struct SkiaLayerPresenter {
    planner: PresentPlanner,
    rasterizer: SkiaLayerRasterizer,
    chrome_rasterizer: SkiaLayerRasterizer,
    compositor: SkiaLayerCompositor,
    previous_layers: HashSet<ElementId>,
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
            planner: PresentPlanner::new(),
            rasterizer: SkiaLayerRasterizer::new(width, height, content_scale),
            chrome_rasterizer: SkiaLayerRasterizer::new(width, height, content_scale),
            compositor: SkiaLayerCompositor::new(content_scale),
            previous_layers: HashSet::new(),
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
        self.planner.cached_bytes()
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
        self.planner.invalidate();
        self.previous_layers.clear();
    }

    #[allow(clippy::too_many_arguments)]
    pub fn present(
        &mut self,
        scene: &SceneGraph,
        layers: &[ElementId],
        layer_raster_bounds: &[LayerRasterBounds],
        layer_dirty: &HashSet<ElementId>,
        scroll_geometry: &HashMap<ElementId, ScrollLayerGeometry>,
        clear: [f32; 4],
        scene_origin: (f32, f32),
        budget: GpuBudget,
        surface: Surface,
    ) -> Result<Surface, String> {
        self.present_with_layer_surface_factory(
            scene,
            layers,
            layer_raster_bounds,
            layer_dirty,
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
        scene: &SceneGraph,
        layers: &[ElementId],
        layer_raster_bounds: &[LayerRasterBounds],
        layer_dirty: &HashSet<ElementId>,
        scroll_geometry: &HashMap<ElementId, ScrollLayerGeometry>,
        clear: [f32; 4],
        scene_origin: (f32, f32),
        budget: GpuBudget,
        factory: &mut dyn SkiaLayerSurfaceFactory,
        surface: Surface,
    ) -> Result<Surface, String> {
        self.last_raster_count = 0;
        self.last_raster_pixels = 0;
        let Some(&root) = layers.first() else {
            let mut surface = surface;
            SkiaSceneRenderer::new().render_scene_with_offset(
                scene,
                surface.canvas(),
                clear,
                self.content_scale,
                scene_origin.0,
                scene_origin.1,
            );
            return Ok(surface);
        };
        let boundaries: HashSet<ElementId> = layers.iter().copied().collect();
        let raster_bounds: HashMap<ElementId, LayerRasterBounds> = layer_raster_bounds
            .iter()
            .map(|bounds| (bounds.layer, *bounds))
            .collect();
        for stale in self
            .previous_layers
            .difference(&boundaries)
            .copied()
            .collect::<Vec<_>>()
        {
            self.rasterizer.discard(stale);
            self.chrome_rasterizer.discard(stale);
            self.planner.evict(stale);
        }
        self.previous_layers = boundaries.clone();

        let non_scroll_layers: Vec<ElementId> = layers
            .iter()
            .copied()
            .filter(|layer| !scroll_geometry.contains_key(layer))
            .collect();
        let plan = self.planner.plan_layers(&non_scroll_layers, layer_dirty);
        for &layer in &plan.raster {
            let extracted = if layer == root {
                extract_root_scene(scene, root, &boundaries)
            } else {
                extract_layer_scene(scene, layer, &boundaries)
                    .ok_or_else(|| format!("skia layer {} is missing", layer.to_u64()))?
            };
            if layer == root {
                self.rasterizer
                    .rasterize_with_layer_surface_factory(factory, layer, &extracted, None)?;
            } else {
                let bounds = raster_bounds.get(&layer).copied().ok_or_else(|| {
                    format!(
                        "missing Core raster bounds for skia layer {}",
                        layer.to_u64()
                    )
                })?;
                self.rasterizer
                    .rasterize_with_bounds_and_layer_surface_factory(
                        factory, layer, &extracted, bounds, None,
                    )?;
            }
            self.last_raster_count += 1;
            self.last_raster_pixels += self.rasterizer.texture_bytes(layer)
                / hayate_layer_compositor::tunables::BYTES_PER_PIXEL;
            self.planner
                .note_layer_rasterized(layer, self.rasterizer.texture_bytes(layer));
        }

        for &layer in layers {
            let Some(geometry) = scroll_geometry.get(&layer) else {
                continue;
            };
            if layer_dirty.contains(&layer) || self.chrome_rasterizer.texture(layer).is_none() {
                let chrome = extract_scroll_chrome_scene(scene, layer, &boundaries)
                    .ok_or_else(|| format!("skia scroll chrome {} is missing", layer.to_u64()))?;
                let bounds = raster_bounds.get(&layer).copied().ok_or_else(|| {
                    format!(
                        "missing Core raster bounds for skia scroll chrome {}",
                        layer.to_u64()
                    )
                })?;
                self.chrome_rasterizer
                    .rasterize_with_bounds_and_layer_surface_factory(
                        factory,
                        layer,
                        &chrome,
                        LayerRasterBounds {
                            origin_y: geometry.absolute_top,
                            height: geometry.viewport_height,
                            ..bounds
                        },
                        None,
                    )?;
            }
            if !geometry.content_dirty
                && !self.planner.scroll_layer_needs_raster(
                    layer,
                    geometry.visible_top,
                    geometry.viewport_height,
                )
            {
                continue;
            }
            let extracted = if layer == root {
                extract_root_scene(scene, root, &boundaries)
            } else {
                extract_scroll_layer_scene(scene, layer, &boundaries, geometry.scroll_affine)
                    .ok_or_else(|| format!("skia scroll layer {} is missing", layer.to_u64()))?
            };
            let bounds = raster_bounds.get(&layer).copied().ok_or_else(|| {
                format!(
                    "missing Core raster bounds for skia scroll layer {}",
                    layer.to_u64()
                )
            })?;
            self.rasterizer
                .rasterize_with_bounds_and_layer_surface_factory(
                    factory,
                    layer,
                    &extracted,
                    bounds,
                    Some(geometry.raster_band()),
                )?;
            self.last_raster_count += 1;
            self.last_raster_pixels += self.rasterizer.texture_bytes(layer)
                / hayate_layer_compositor::tunables::BYTES_PER_PIXEL;
            self.planner.note_scroll_rasterized(
                layer,
                geometry.band,
                self.rasterizer.texture_bytes(layer) + self.chrome_rasterizer.texture_bytes(layer),
            );
        }

        let placements = collect_layer_placements(scene, root, &boundaries);
        let mut quads: Vec<CompositeQuad<'_, SkiaLayerTexture>> = Vec::new();
        for placement in &placements {
            if let Some(texture) = self.rasterizer.texture(placement.layer) {
                let transform =
                    scroll_geometry
                        .get(&placement.layer)
                        .map_or(placement.transform, |geometry| {
                            // Bounded textures retain their canonical device origin. Reapply only
                            // the live scroll affine; adding the cached band's origin here would
                            // place that origin twice.
                            compose(placement.transform, geometry.scroll_affine)
                        });
                quads.push(CompositeQuad {
                    layer: placement.layer,
                    transform,
                    opacity: 1.0,
                    clip: placement.clip,
                    texture,
                });
            }
            if scroll_geometry.contains_key(&placement.layer) {
                if let Some(texture) = self.chrome_rasterizer.texture(placement.layer) {
                    quads.push(CompositeQuad {
                        layer: placement.layer,
                        // Chrome's bounded texture already carries its absolute scene origin.
                        transform: placement.transform,
                        opacity: 1.0,
                        clip: placement.clip,
                        texture,
                    });
                }
            }
        }

        let mut target = SkiaCompositeTarget { surface, clear };
        target.surface.canvas().save();
        target.surface.canvas().translate((
            scene_origin.0 * self.content_scale,
            scene_origin.1 * self.content_scale,
        ));
        self.compositor.composite(&mut target, &quads)?;
        target.surface.canvas().restore();
        for quad in &quads {
            self.planner.note_composited(quad.layer);
        }
        for evicted in self.planner.enforce_budget(budget) {
            self.rasterizer.discard(evicted);
            self.chrome_rasterizer.discard(evicted);
        }
        Ok(target.surface)
    }
}
