use std::collections::{HashMap, HashSet};

use hayate_core::element::id::ElementId;
use hayate_core::{LayerRasterBounds, SceneGraph};
use hayate_layer_compositor::layer_scene::{
    collect_layer_placements, compose, extract_layer_scene, extract_root_scene,
    extract_scroll_chrome_scene, extract_scroll_layer_scene,
};
use hayate_layer_compositor::{
    tunables, CompositeQuad, GpuBudget, LayerCompositor, LayerRasterizer, PresentPlanner,
    ScrollLayerGeometry,
};
use hayate_scene_renderer_tiny_skia::{
    premultiplied_to_straight, TinySkiaCompositeTarget, TinySkiaLayerCompositor,
    TinySkiaLayerRasterizer, TinySkiaSceneRenderer,
};
use tiny_skia::Pixmap;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use super::{js_to_anyhow, CanvasBackend, ClearColor, SceneRendererKind};

pub(crate) struct SelectedBackend {
    ctx: web_sys::CanvasRenderingContext2d,
    pixmap: Pixmap,
    scene_renderer: TinySkiaSceneRenderer,
    width: u32,
    height: u32,
    content_scale: f32,
    // per-layer present（#636）。dirty レイヤだけ Pixmap 再 raster し、clean レイヤはキャッシュ面を
    // draw_pixmap 合成する。planner は backend 非依存の台帳（`plan_raster` / 予算）を握る。
    planner: PresentPlanner,
    rasterizer: TinySkiaLayerRasterizer,
    chrome_rasterizer: TinySkiaLayerRasterizer,
    compositor: TinySkiaLayerCompositor,
    prev_layers: HashSet<ElementId>,
    // ADR-0138 比較用トグル。既定 ON（#636 の per-layer 経路を維持）——`HayateElementRenderer::init`
    // の `layer_present_enabled` 引数で OFF にすると `supports_layer_present()` が false を返し、
    // 呼び出し側（`canvas.rs`）が全面 `render_scene` にフォールバックする。
    layer_present_enabled: bool,
}

impl SelectedBackend {
    pub(crate) async fn init(canvas: HtmlCanvasElement) -> Result<Self, JsValue> {
        Self::init_sync(canvas)
    }

    pub(crate) fn init_sync(canvas: HtmlCanvasElement) -> Result<Self, JsValue> {
        let width = canvas.width();
        let height = canvas.height();

        let ctx = canvas
            .get_context("2d")
            .map_err(|e| JsValue::from_str(&format!("get_context(\"2d\"): {e:?}")))?
            .ok_or_else(|| JsValue::from_str("canvas 2d context unavailable"))?
            .dyn_into::<web_sys::CanvasRenderingContext2d>()
            .map_err(|_| JsValue::from_str("failed to cast to CanvasRenderingContext2d"))?;

        let pixmap = Pixmap::new(width, height)
            .ok_or_else(|| JsValue::from_str("failed to create Pixmap (zero size?)"))?;

        Ok(Self {
            ctx,
            pixmap,
            scene_renderer: TinySkiaSceneRenderer::new(),
            width,
            height,
            content_scale: 1.0,
            planner: PresentPlanner::new(),
            rasterizer: TinySkiaLayerRasterizer::new(width, height, 1.0),
            chrome_rasterizer: TinySkiaLayerRasterizer::new(width, height, 1.0),
            compositor: TinySkiaLayerCompositor::new(1.0),
            prev_layers: HashSet::new(),
            layer_present_enabled: true,
        })
    }
}

impl CanvasBackend for SelectedBackend {
    fn kind(&self) -> SceneRendererKind {
        SceneRendererKind::TinySkia
    }

    fn render_scene(
        &mut self,
        scene: &SceneGraph,
        clear_color: ClearColor,
    ) -> Result<(), anyhow::Error> {
        self.scene_renderer
            .render_scene(scene, &mut self.pixmap, clear_color, self.content_scale);
        blit_to_canvas(&self.ctx, &self.pixmap, self.width, self.height).map_err(js_to_anyhow)
    }

    fn clear(&mut self, clear_color: ClearColor) -> Result<(), anyhow::Error> {
        self.scene_renderer.render_scene(
            &SceneGraph::new(),
            &mut self.pixmap,
            clear_color,
            self.content_scale,
        );
        blit_to_canvas(&self.ctx, &self.pixmap, self.width, self.height).map_err(js_to_anyhow)
    }

    fn supports_layer_present(&self) -> bool {
        self.layer_present_enabled
    }

    fn set_layer_present_enabled(&mut self, enabled: bool) {
        self.layer_present_enabled = enabled;
    }

    fn present_layers(
        &mut self,
        scene: &SceneGraph,
        layers: &[ElementId],
        layer_raster_bounds: &[LayerRasterBounds],
        layer_dirty: &HashSet<ElementId>,
        scroll_geometry: &HashMap<ElementId, ScrollLayerGeometry>,
        clear_color: ClearColor,
    ) -> Result<(), anyhow::Error> {
        let Some(&root) = layers.first() else {
            return Ok(());
        };
        let boundaries: HashSet<ElementId> = layers.iter().copied().collect();
        let raster_bounds: HashMap<ElementId, LayerRasterBounds> = layer_raster_bounds
            .iter()
            .map(|bounds| (bounds.layer, *bounds))
            .collect();

        // 消えたレイヤ（transition 終了等）のキャッシュ面と台帳を掃除する。
        for stale in self
            .prev_layers
            .difference(&boundaries)
            .copied()
            .collect::<Vec<_>>()
        {
            self.rasterizer.discard(stale);
            self.chrome_rasterizer.discard(stale);
            self.planner.evict(stale);
        }
        self.prev_layers = boundaries.clone();

        // Non-scroll layers use Core's full conservative 2D extent. Scroll content/chrome are
        // handled separately below so their band and viewport surfaces retain that actual width.
        let non_scroll_layers: Vec<ElementId> = layers
            .iter()
            .copied()
            .filter(|layer| *layer == root || !scroll_geometry.contains_key(layer))
            .collect();
        let plan = self.planner.plan_layers(&non_scroll_layers, layer_dirty);
        for &layer in &plan.raster {
            let extracted = if layer == root {
                extract_root_scene(scene, root, &boundaries)
            } else {
                match extract_layer_scene(scene, layer, &boundaries) {
                    Some(extracted) => extracted,
                    None => continue, // 未 lowering（次フレームで raster される）
                }
            };
            if layer == root {
                self.rasterizer
                    .rasterize(layer, &extracted, None)
                    .map_err(|e| anyhow::anyhow!(e))?;
            } else {
                let bounds = raster_bounds.get(&layer).copied().ok_or_else(|| {
                    anyhow::anyhow!("missing Core raster bounds for layer {layer:?}")
                })?;
                self.rasterizer
                    .rasterize_with_bounds(layer, &extracted, bounds, None)
                    .map_err(|e| anyhow::anyhow!(e))?;
            }
            self.planner
                .note_layer_rasterized(layer, self.rasterizer.texture_bytes(layer));
        }

        for &layer in layers {
            if layer == root {
                continue; // The implicit root cache deliberately remains full-surface.
            }
            let Some(geometry) = scroll_geometry.get(&layer) else {
                continue;
            };
            let bounds = raster_bounds.get(&layer).copied().ok_or_else(|| {
                anyhow::anyhow!("missing Core raster bounds for scroll layer {layer:?}")
            })?;

            if layer_dirty.contains(&layer) || self.chrome_rasterizer.texture(layer).is_none() {
                if let Some(chrome) = extract_scroll_chrome_scene(scene, layer, &boundaries) {
                    let chrome_bounds = LayerRasterBounds {
                        origin_y: geometry.absolute_top,
                        height: geometry.viewport_height,
                        ..bounds
                    };
                    self.chrome_rasterizer
                        .rasterize_with_bounds(layer, &chrome, chrome_bounds, None)
                        .map_err(|e| anyhow::anyhow!(e))?;
                }
            }

            let needs_content_raster = geometry.content_dirty
                || self.planner.scroll_layer_needs_raster(
                    layer,
                    geometry.visible_top,
                    geometry.viewport_height,
                );
            if needs_content_raster {
                let extracted = if layer == root {
                    extract_root_scene(scene, root, &boundaries)
                } else {
                    extract_scroll_layer_scene(scene, layer, &boundaries, geometry.scroll_affine)
                        .ok_or_else(|| anyhow::anyhow!("scroll layer {layer:?} is missing"))?
                };
                self.rasterizer
                    .rasterize_with_bounds(layer, &extracted, bounds, Some(geometry.raster_band()))
                    .map_err(|e| anyhow::anyhow!(e))?;
                self.planner.note_scroll_rasterized(
                    layer,
                    geometry.band,
                    self.rasterizer.texture_bytes(layer)
                        + self.chrome_rasterizer.texture_bytes(layer),
                );
            }
        }

        // キャッシュ Pixmap を placement quad（transform/clip、保持シーンから毎フレーム導出）で
        // 合成する。composite-only フレームは上の raster ループが空＝全面 render_scene は走らない。
        let placements = collect_layer_placements(scene, root, &boundaries);
        let mut quads = Vec::new();
        for placement in &placements {
            if let Some(texture) = self.rasterizer.texture(placement.layer) {
                let transform = match (
                    self.planner.cached_scroll_band(placement.layer),
                    scroll_geometry.get(&placement.layer),
                ) {
                    // TinySkiaLayerTexture restores its absolute cached-band origin itself;
                    // unlike the wgpu/Skia textures whose row 0 is local, only the live scroll
                    // affine remains to apply here.
                    (Some(_), Some(geometry)) => {
                        compose(placement.transform, geometry.scroll_affine)
                    }
                    _ => placement.transform,
                };
                quads.push(CompositeQuad {
                    layer: placement.layer,
                    transform,
                    opacity: 1.0,
                    clip: placement.clip,
                    texture,
                });
            }
            if let (Some(_geometry), Some(texture)) = (
                scroll_geometry.get(&placement.layer),
                self.chrome_rasterizer.texture(placement.layer),
            ) {
                quads.push(CompositeQuad {
                    layer: placement.layer,
                    // Chrome texture also carries its absolute layer-local origin.
                    transform: placement.transform,
                    opacity: 1.0,
                    clip: placement.clip,
                    texture,
                });
            }
        }
        let mut target = TinySkiaCompositeTarget {
            pixmap: std::mem::replace(&mut self.pixmap, Pixmap::new(1, 1).expect("1x1 pixmap")),
            clear: clear_color,
        };
        let result = self.compositor.composite(&mut target, &quads);
        self.pixmap = target.pixmap;
        result.map_err(|e| anyhow::anyhow!(e))?;
        for quad in &quads {
            self.planner.note_composited(quad.layer);
        }
        let budget = GpuBudget::from_viewports(
            self.width,
            self.height,
            tunables::GPU_BUDGET_VIEWPORTS_DESKTOP,
        );
        for evicted in self.planner.enforce_budget(budget) {
            self.rasterizer.discard(evicted);
            self.chrome_rasterizer.discard(evicted);
        }
        blit_to_canvas(&self.ctx, &self.pixmap, self.width, self.height).map_err(js_to_anyhow)
    }

    fn resize(&mut self, width: u32, height: u32, content_scale: f32) {
        self.content_scale = content_scale.max(1.0);
        self.compositor.set_content_scale(self.content_scale);
        if width == 0 || height == 0 || (width == self.width && height == self.height) {
            // DPR だけ変わっても content_scale は反映済み。キャッシュ面はスケール込みなので作り直す。
            self.rasterizer
                .resize(self.width, self.height, self.content_scale);
            self.chrome_rasterizer
                .resize(self.width, self.height, self.content_scale);
            self.planner.invalidate();
            return;
        }
        if let Some(pixmap) = Pixmap::new(width, height) {
            self.pixmap = pixmap;
            self.width = width;
            self.height = height;
            // レイヤキャッシュ面はサーフェスサイズ＝作り直し。台帳ごと invalidate（古いサイズを
            // 合成し続けない）。
            self.rasterizer.resize(width, height, self.content_scale);
            self.chrome_rasterizer
                .resize(width, height, self.content_scale);
            self.planner.invalidate();
        }
    }
}

fn blit_to_canvas(
    ctx: &web_sys::CanvasRenderingContext2d,
    pixmap: &Pixmap,
    width: u32,
    height: u32,
) -> Result<(), JsValue> {
    let mut straight = pixmap.data().to_vec();
    premultiplied_to_straight(&mut straight);

    let image_data = web_sys::ImageData::new_with_u8_clamped_array_and_sh(
        wasm_bindgen::Clamped(&straight),
        width,
        height,
    )?;
    ctx.put_image_data(&image_data, 0.0, 0.0)
}
