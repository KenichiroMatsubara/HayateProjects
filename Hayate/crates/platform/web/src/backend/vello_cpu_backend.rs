use std::collections::{HashMap, HashSet};

use hayate_core::element::id::ElementId;
use hayate_core::SceneGraph;
use hayate_layer_compositor::layer_scene::{
    collect_layer_placements, extract_layer_scene, extract_root_scene,
};
use hayate_layer_compositor::{
    CompositeQuad, LayerCompositor, LayerRasterizer, PresentPlanner, ScrollLayerGeometry,
};
use hayate_scene_renderer_vello_cpu::{
    premultiplied_to_straight, VelloCpuCompositeTarget, VelloCpuLayerCompositor,
    VelloCpuLayerRasterizer, VelloCpuSceneRenderer,
};
use vello_cpu::Pixmap;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use super::{js_to_anyhow, CanvasBackend, ClearColor, SceneRendererKind};

fn clamp_u16(v: u32) -> u16 {
    v.min(u32::from(u16::MAX)) as u16
}

pub(crate) struct SelectedBackend {
    ctx: web_sys::CanvasRenderingContext2d,
    pixmap: Pixmap,
    scene_renderer: VelloCpuSceneRenderer,
    width: u32,
    height: u32,
    content_scale: f32,
    // per-layer present（tiny-skia backend と同じ設計）。dirty レイヤだけ Pixmap 再 raster し、
    // clean レイヤはキャッシュ面を合成するだけにする。
    planner: PresentPlanner,
    rasterizer: VelloCpuLayerRasterizer,
    compositor: VelloCpuLayerCompositor,
    prev_layers: HashSet<ElementId>,
    // ADR-0138 比較用トグル。既定 ON（tiny-skia backend と同じ設計）——`HayateElementRenderer::init`
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

        let pixmap = Pixmap::new(clamp_u16(width.max(1)), clamp_u16(height.max(1)));

        Ok(Self {
            ctx,
            pixmap,
            scene_renderer: VelloCpuSceneRenderer::new(),
            width,
            height,
            content_scale: 1.0,
            planner: PresentPlanner::new(),
            rasterizer: VelloCpuLayerRasterizer::new(width, height, 1.0),
            compositor: VelloCpuLayerCompositor::new(1.0),
            prev_layers: HashSet::new(),
            layer_present_enabled: true,
        })
    }
}

impl CanvasBackend for SelectedBackend {
    fn kind(&self) -> SceneRendererKind {
        SceneRendererKind::VelloCpu
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
        self.layer_present_enabled
    }

    fn set_layer_present_enabled(&mut self, enabled: bool) {
        self.layer_present_enabled = enabled;
    }

    fn present_layers(
        &mut self,
        scene: &SceneGraph,
        layers: &[ElementId],
        layer_dirty: &HashSet<ElementId>,
        // #707 (ADR-0127): scroll-band overscan sizing is vello-only for now (see vello.rs's
        // `present_layers`) — vello_cpu's per-layer path stays exactly as before this parameter
        // existed (every layer, including `ScrollView`s, gets a full-surface `Pixmap`).
        _scroll_geometry: &HashMap<ElementId, ScrollLayerGeometry>,
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
                .rasterize(layer, &extracted, None)
                .map_err(|e| anyhow::anyhow!(e))?;
            self.planner
                .note_layer_rasterized(layer, self.rasterizer.texture_bytes_per_layer());
        }

        // キャッシュ Pixmap を placement quad（transform/clip、保持シーンから毎フレーム導出）で
        // 合成する。composite-only フレームは上の raster ループが空＝全面 render_scene は走らない。
        let placements = collect_layer_placements(scene, root, &boundaries);
        let quads: Vec<CompositeQuad<'_, std::sync::Arc<Pixmap>>> = placements
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
        let mut target = VelloCpuCompositeTarget {
            pixmap: std::mem::replace(&mut self.pixmap, Pixmap::new(1, 1)),
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
        self.pixmap = Pixmap::new(clamp_u16(width), clamp_u16(height));
        self.width = width;
        self.height = height;
        // レイヤキャッシュ面はサーフェスサイズ＝作り直し。台帳ごと invalidate（古いサイズを
        // 合成し続けない）。
        self.rasterizer.resize(width, height, self.content_scale);
        self.planner.invalidate();
    }
}

fn blit_to_canvas(
    ctx: &web_sys::CanvasRenderingContext2d,
    pixmap: &Pixmap,
    width: u32,
    height: u32,
) -> Result<(), JsValue> {
    let mut straight = pixmap.data_as_u8_slice().to_vec();
    premultiplied_to_straight(&mut straight);

    let image_data = web_sys::ImageData::new_with_u8_clamped_array_and_sh(
        wasm_bindgen::Clamped(&straight),
        width,
        height,
    )?;
    ctx.put_image_data(&image_data, 0.0, 0.0)
}
