use std::collections::HashMap;

use super::{CanvasBackend, ClearColor, SceneRendererKind, WebCanvasSurface};
use hayate_core::{ElementId, LayerTopology, SceneSnapshot};
use hayate_layer_compositor::ScrollLayerGeometry;
use wasm_bindgen::prelude::*;

pub(crate) struct SelectedBackend;

impl SelectedBackend {
    pub(crate) async fn init(canvas: WebCanvasSurface) -> Result<Self, JsValue> {
        Self::init_sync(canvas)
    }

    pub(crate) fn init_sync(canvas: WebCanvasSurface) -> Result<Self, JsValue> {
        let _ = canvas;
        Ok(Self)
    }
}

impl CanvasBackend for SelectedBackend {
    fn kind(&self) -> SceneRendererKind {
        SceneRendererKind::Null
    }

    fn present_layers(
        &mut self,
        _scene: &SceneSnapshot,
        _topology: &LayerTopology,
        _scroll_geometry: &HashMap<ElementId, ScrollLayerGeometry>,
        _clear_color: ClearColor,
    ) -> Result<(), anyhow::Error> {
        Ok(())
    }

    fn clear(&mut self, _clear_color: ClearColor) -> Result<(), anyhow::Error> {
        Ok(())
    }
}
