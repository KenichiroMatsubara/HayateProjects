use hayate_core::{RecordingBackend, SceneGraph};
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use super::{CanvasBackend, ClearColor, SceneRendererKind};

pub(crate) struct SelectedBackend {
    recorder: RecordingBackend,
}

impl SelectedBackend {
    pub(crate) async fn init(canvas: HtmlCanvasElement) -> Result<Self, JsValue> {
        let _ = canvas;
        Ok(Self {
            recorder: RecordingBackend::new(),
        })
    }
}

impl CanvasBackend for SelectedBackend {
    fn kind(&self) -> SceneRendererKind {
        SceneRendererKind::Recording
    }

    fn render_scene(&mut self, scene: &SceneGraph, clear_color: ClearColor) -> Result<(), JsValue> {
        self.recorder.render(scene, clear_color);
        Ok(())
    }

    fn clear(&mut self, clear_color: ClearColor) -> Result<(), JsValue> {
        self.recorder.clear(clear_color);
        Ok(())
    }
}
