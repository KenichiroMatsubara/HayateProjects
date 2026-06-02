use hayate_core::SceneGraph;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use super::{CanvasBackend, ClearColor};

pub(crate) struct SelectedBackend;

impl SelectedBackend {
    pub(crate) async fn init(canvas: HtmlCanvasElement) -> Result<Self, JsValue> {
        let _ = canvas;
        Ok(Self)
    }
}

impl CanvasBackend for SelectedBackend {
    fn render_scene(
        &mut self,
        _scene: &SceneGraph,
        _clear_color: ClearColor,
    ) -> Result<(), JsValue> {
        Ok(())
    }

    fn clear(&mut self, _clear_color: ClearColor) -> Result<(), JsValue> {
        Ok(())
    }
}
