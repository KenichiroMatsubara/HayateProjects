use hayate_core::SceneGraph;
use wasm_bindgen::prelude::*;

pub(crate) type ClearColor = [f32; 4];

#[cfg(feature = "backend-vello")]
mod vello;

#[cfg(feature = "backend-vello")]
pub(crate) use vello::SelectedBackend;

#[cfg(all(not(feature = "backend-vello"), feature = "backend-recording"))]
mod recording;

#[cfg(all(not(feature = "backend-vello"), feature = "backend-recording"))]
pub(crate) use recording::SelectedBackend;

#[cfg(all(
    not(feature = "backend-vello"),
    not(feature = "backend-recording"),
    feature = "backend-null"
))]
mod null;

#[cfg(all(
    not(feature = "backend-vello"),
    not(feature = "backend-recording"),
    feature = "backend-null"
))]
pub(crate) use null::SelectedBackend;

#[cfg(not(any(
    feature = "backend-vello",
    feature = "backend-recording",
    feature = "backend-null"
)))]
compile_error!("Enable one of: backend-vello, backend-recording, backend-null");

pub(crate) trait CanvasBackend {
    fn render_scene(&mut self, scene: &SceneGraph, clear_color: ClearColor) -> Result<(), JsValue>;
    fn clear(&mut self, clear_color: ClearColor) -> Result<(), JsValue>;
}
