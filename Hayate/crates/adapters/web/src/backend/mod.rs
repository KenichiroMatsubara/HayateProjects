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
    feature = "backend-tiny-skia"
))]
mod tiny_skia_backend;

#[cfg(all(
    not(feature = "backend-vello"),
    not(feature = "backend-recording"),
    feature = "backend-tiny-skia"
))]
pub(crate) use tiny_skia_backend::SelectedBackend;

#[cfg(all(
    not(feature = "backend-vello"),
    not(feature = "backend-recording"),
    not(feature = "backend-tiny-skia"),
    feature = "backend-null"
))]
mod null;

#[cfg(all(
    not(feature = "backend-vello"),
    not(feature = "backend-recording"),
    not(feature = "backend-tiny-skia"),
    feature = "backend-null"
))]
pub(crate) use null::SelectedBackend;

#[cfg(not(any(
    feature = "backend-vello",
    feature = "backend-recording",
    feature = "backend-tiny-skia",
    feature = "backend-null"
)))]
compile_error!(
    "Enable one of: backend-vello, backend-recording, backend-tiny-skia, backend-null"
);

pub(crate) trait CanvasBackend {
    fn render_scene(&mut self, scene: &SceneGraph, clear_color: ClearColor) -> Result<(), JsValue>;
    fn clear(&mut self, clear_color: ClearColor) -> Result<(), JsValue>;

    /// Resize the render surface to match the canvas's new pixel dimensions.
    /// Backends that draw to an off-screen target (GPU texture / CPU pixmap)
    /// must reallocate it here, otherwise content stays clipped to the init
    /// size while the canvas grows. Default is a no-op for sizeless backends.
    fn resize(&mut self, _width: u32, _height: u32) {}
}
