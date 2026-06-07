use hayate_core::SceneGraph;
use hayate_scene_renderer_tiny_skia::{premultiplied_to_straight, TinySkiaSceneRenderer};
use tiny_skia::Pixmap;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use super::{CanvasBackend, ClearColor, SceneRendererKind};

pub(crate) struct SelectedBackend {
    ctx: web_sys::CanvasRenderingContext2d,
    pixmap: Pixmap,
    scene_renderer: TinySkiaSceneRenderer,
    width: u32,
    height: u32,
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
        })
    }
}

impl CanvasBackend for SelectedBackend {
    fn kind(&self) -> SceneRendererKind {
        SceneRendererKind::TinySkia
    }

    fn render_scene(&mut self, scene: &SceneGraph, clear_color: ClearColor) -> Result<(), JsValue> {
        self.scene_renderer
            .render_scene(scene, &mut self.pixmap, clear_color);
        blit_to_canvas(&self.ctx, &self.pixmap, self.width, self.height)
    }

    fn clear(&mut self, clear_color: ClearColor) -> Result<(), JsValue> {
        self.scene_renderer
            .render_scene(&SceneGraph::new(), &mut self.pixmap, clear_color);
        blit_to_canvas(&self.ctx, &self.pixmap, self.width, self.height)
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 || (width == self.width && height == self.height) {
            return;
        }
        if let Some(pixmap) = Pixmap::new(width, height) {
            self.pixmap = pixmap;
            self.width = width;
            self.height = height;
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
