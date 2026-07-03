use std::collections::HashSet;

use hayate_core::element::id::ElementId;
use hayate_core::SceneGraph;
use hayate_layer_compositor::layer_scene::{
    collect_layer_placements, extract_layer_scene, extract_root_scene,
};
use hayate_layer_compositor::{CompositeQuad, LayerCompositor, LayerRasterizer, PresentPlanner};
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
    compositor: TinySkiaLayerCompositor,
    prev_layers: HashSet<ElementId>,
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
            compositor: TinySkiaLayerCompositor::new(1.0),
            prev_layers: HashSet::new(),
        })
    }
}

impl CanvasBackend for SelectedBackend {
    fn kind(&self) -> SceneRendererKind {
        SceneRendererKind::TinySkia
    }

    fn render_scene(&mut self, scene: &SceneGraph, clear_color: ClearColor) -> Result<(), anyhow::Error> {
        self.scene_renderer.render_scene(
            scene,
            &mut self.pixmap,
            clear_color,
            self.content_scale,
        );
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
        true
    }

    fn present_layers(
        &mut self,
        scene: &SceneGraph,
        layers: &[ElementId],
        layer_dirty: &HashSet<ElementId>,
        clear_color: ClearColor,
    ) -> Result<(), anyhow::Error> {
        let Some(&root) = layers.first() else {
            return Ok(());
        };
        let boundaries: HashSet<ElementId> = layers.iter().copied().collect();

        // 消えたレイヤ（transition 終了等）のキャッシュ面と台帳を掃除する。
        for stale in self.prev_layers.difference(&boundaries).copied().collect::<Vec<_>>() {
            self.rasterizer.discard(stale);
            self.planner.evict(stale);
        }
        self.prev_layers = boundaries.clone();

        // dirty / 未キャッシュのレイヤだけ Pixmap へ再 raster（plan_raster の raster/reuse どおり）。
        let plan = self.planner.plan_layers(layers, layer_dirty);
        for &layer in &plan.raster {
            let extracted = if layer == root {
                extract_root_scene(scene, root, &boundaries)
            } else {
                match extract_layer_scene(scene, layer, &boundaries) {
                    Some(extracted) => extracted,
                    None => continue, // 未 lowering（次フレームで raster される）
                }
            };
            self.rasterizer
                .rasterize(layer, &extracted)
                .map_err(|e| anyhow::anyhow!(e))?;
            self.planner
                .note_layer_rasterized(layer, self.rasterizer.texture_bytes_per_layer());
        }

        // キャッシュ Pixmap を placement quad（transform/clip、保持シーンから毎フレーム導出）で
        // 合成する。composite-only フレームは上の raster ループが空＝全面 render_scene は走らない。
        let placements = collect_layer_placements(scene, root, &boundaries);
        let quads: Vec<CompositeQuad<'_, Pixmap>> = placements
            .iter()
            .filter_map(|p| {
                self.rasterizer.texture(p.layer).map(|texture| CompositeQuad {
                    layer: p.layer,
                    transform: p.transform,
                    opacity: 1.0,
                    clip: p.clip,
                    texture,
                })
            })
            .collect();
        let mut target = TinySkiaCompositeTarget {
            pixmap: std::mem::replace(
                &mut self.pixmap,
                Pixmap::new(1, 1).expect("1x1 pixmap"),
            ),
            clear: clear_color,
        };
        let result = self.compositor.composite(&mut target, &quads);
        self.pixmap = target.pixmap;
        result.map_err(|e| anyhow::anyhow!(e))?;
        for quad in &quads {
            self.planner.note_composited(quad.layer);
        }
        blit_to_canvas(&self.ctx, &self.pixmap, self.width, self.height).map_err(js_to_anyhow)
    }

    fn resize(&mut self, width: u32, height: u32, content_scale: f32) {
        self.content_scale = content_scale.max(1.0);
        self.compositor.set_content_scale(self.content_scale);
        if width == 0 || height == 0 || (width == self.width && height == self.height) {
            // DPR だけ変わっても content_scale は反映済み。キャッシュ面はスケール込みなので作り直す。
            self.rasterizer.resize(self.width, self.height, self.content_scale);
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
