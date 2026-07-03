use hayate_core::SceneGraph;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use super::{CanvasBackend, ClearColor, SceneRendererKind};

pub(crate) struct SelectedBackend;

impl SelectedBackend {
    pub(crate) async fn init(canvas: HtmlCanvasElement) -> Result<Self, JsValue> {
        Self::init_sync(canvas)
    }

    pub(crate) fn init_sync(canvas: HtmlCanvasElement) -> Result<Self, JsValue> {
        let _ = canvas;
        Ok(Self)
    }
}

impl CanvasBackend for SelectedBackend {
    fn kind(&self) -> SceneRendererKind {
        SceneRendererKind::Null
    }

    fn render_scene(
        &mut self,
        _scene: &SceneGraph,
        _clear_color: ClearColor,
    ) -> Result<(), anyhow::Error> {
        Ok(())
    }

    fn clear(&mut self, _clear_color: ClearColor) -> Result<(), anyhow::Error> {
        Ok(())
    }
}
