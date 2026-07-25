use std::collections::HashMap;

use super::{CanvasBackend, ClearColor, SceneRendererKind, WebCanvasSurface};
use hayate_core::{ElementId, LayerTopology, SceneRecorder, SceneSnapshot};
use hayate_layer_compositor::ScrollLayerGeometry;
use wasm_bindgen::prelude::*;

pub(crate) struct SelectedBackend {
    recorder: SceneRecorder,
}

impl SelectedBackend {
    pub(crate) async fn init(canvas: WebCanvasSurface) -> Result<Self, JsValue> {
        Self::init_sync(canvas)
    }

    pub(crate) fn init_sync(canvas: WebCanvasSurface) -> Result<Self, JsValue> {
        let _ = canvas;
        Ok(Self {
            recorder: SceneRecorder::new(),
        })
    }
}

impl CanvasBackend for SelectedBackend {
    fn kind(&self) -> SceneRendererKind {
        SceneRendererKind::Recording
    }

    fn present_layers(
        &mut self,
        scene: &SceneSnapshot,
        _topology: &LayerTopology,
        _scroll_geometry: &HashMap<ElementId, ScrollLayerGeometry>,
        clear_color: ClearColor,
    ) -> Result<(), anyhow::Error> {
        self.recorder.record(scene, clear_color);
        Ok(())
    }

    fn clear(&mut self, clear_color: ClearColor) -> Result<(), anyhow::Error> {
        self.recorder.clear(clear_color);
        Ok(())
    }
}
