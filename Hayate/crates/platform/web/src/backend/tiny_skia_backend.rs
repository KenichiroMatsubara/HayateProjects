use std::collections::HashMap;

use hayate_core::{ElementId, LayerTopology, SceneSnapshot};
use hayate_layer_compositor::{
    tunables, GpuBudget, LayerPresentation, LayerPresentationFrame, ScrollLayerGeometry,
};
use hayate_scene_renderer_tiny_skia::{
    premultiplied_to_straight, TinySkiaLayerCompositor, TinySkiaLayerPresentationAdapter,
    TinySkiaLayerRasterizer,
};
use tiny_skia::{Color, Pixmap};
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use super::{js_to_anyhow, CanvasBackend, ClearColor, SceneRendererKind};

pub(crate) struct SelectedBackend {
    ctx: web_sys::CanvasRenderingContext2d,
    pixmap: Pixmap,
    width: u32,
    height: u32,
    content_scale: f32,
    presentation: LayerPresentation,
    rasterizer: TinySkiaLayerRasterizer,
    chrome_rasterizer: TinySkiaLayerRasterizer,
    compositor: TinySkiaLayerCompositor,
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
            .map_err(|error| JsValue::from_str(&format!("get_context(\"2d\"): {error:?}")))?
            .ok_or_else(|| JsValue::from_str("canvas 2d context unavailable"))?
            .dyn_into::<web_sys::CanvasRenderingContext2d>()
            .map_err(|_| JsValue::from_str("failed to cast to CanvasRenderingContext2d"))?;
        let pixmap = Pixmap::new(width, height)
            .ok_or_else(|| JsValue::from_str("failed to create Pixmap (zero size?)"))?;
        Ok(Self {
            ctx,
            pixmap,
            width,
            height,
            content_scale: 1.0,
            presentation: LayerPresentation::new(),
            rasterizer: TinySkiaLayerRasterizer::new(width, height, 1.0),
            chrome_rasterizer: TinySkiaLayerRasterizer::new(width, height, 1.0),
            compositor: TinySkiaLayerCompositor::new(1.0),
        })
    }
}

impl CanvasBackend for SelectedBackend {
    fn kind(&self) -> SceneRendererKind {
        SceneRendererKind::TinySkia
    }

    fn clear(&mut self, clear_color: ClearColor) -> Result<(), anyhow::Error> {
        let [r, g, b, a] = clear_color;
        self.pixmap.fill(Color::from_rgba(
            r.clamp(0.0, 1.0),
            g.clamp(0.0, 1.0),
            b.clamp(0.0, 1.0),
            a.clamp(0.0, 1.0),
        ));
        blit_to_canvas(&self.ctx, &self.pixmap, self.width, self.height).map_err(js_to_anyhow)
    }

    fn present_layers(
        &mut self,
        snapshot: &SceneSnapshot,
        topology: &LayerTopology,
        scroll_geometry: &HashMap<ElementId, ScrollLayerGeometry>,
        clear_color: ClearColor,
    ) -> Result<(), anyhow::Error> {
        let mut adapter = TinySkiaLayerPresentationAdapter {
            rasterizer: &mut self.rasterizer,
            chrome_rasterizer: &mut self.chrome_rasterizer,
            compositor: &mut self.compositor,
            target: &mut self.pixmap,
            clear: clear_color,
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
            .map_err(|error| anyhow::anyhow!("layer presentation: {error:?}"))?;
        let budget = GpuBudget::from_viewports(
            self.width,
            self.height,
            tunables::GPU_BUDGET_VIEWPORTS_DESKTOP,
        );
        self.presentation.enforce_budget(budget, &mut adapter);
        drop(adapter);
        blit_to_canvas(&self.ctx, &self.pixmap, self.width, self.height).map_err(js_to_anyhow)
    }

    fn resize(&mut self, width: u32, height: u32, content_scale: f32) {
        self.content_scale = content_scale.max(1.0);
        self.compositor.set_content_scale(self.content_scale);
        if width == 0 || height == 0 || (width == self.width && height == self.height) {
            self.rasterizer
                .resize(self.width, self.height, self.content_scale);
            self.chrome_rasterizer
                .resize(self.width, self.height, self.content_scale);
            self.presentation.invalidate();
            return;
        }
        if let Some(pixmap) = Pixmap::new(width, height) {
            self.pixmap = pixmap;
            self.width = width;
            self.height = height;
            self.rasterizer.resize(width, height, self.content_scale);
            self.chrome_rasterizer
                .resize(width, height, self.content_scale);
            self.presentation.invalidate();
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
