//! CanvasKit tracer-bullet backend (#813).
//!
//! CanvasKit's JS objects and WebGL surface remain Host-owned. This adapter only lowers the
//! existing SceneGraph into a compact float command stream and calls the opaque Host bridge once
//! per frame. The deliberately small vocabulary is clear + filled rectangle; #814 expands it.

use hayate_core::{
    DrawFillRule, PathVerb, RenderImage, SceneGraph, ScenePainter, StrokeStyle, TextRunData,
    render_scene_graph,
};
use js_sys::{Float32Array, Function, Reflect};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::HtmlCanvasElement;

use super::{CanvasBackend, ClearColor, SceneRendererKind};

const HOST_BRIDGE_KEY: &str = "__hayateCanvasKitBridge";
const REPLAY_METHOD: &str = "replay";
const RESIZE_METHOD: &str = "resize";
const CLEAR: f32 = 0.0;
const FILL_RECT: f32 = 1.0;

pub(crate) struct SelectedBackend {
    canvas: HtmlCanvasElement,
    content_scale: f32,
}

impl SelectedBackend {
    pub(crate) fn init(canvas: HtmlCanvasElement) -> Result<Self, JsValue> {
        // The generated Host loader prepares the CanvasKit surface before importing this WASM
        // target. Failing here makes it an init failure, so #811's boot-only policy can advance.
        bridge_method(REPLAY_METHOD)?;
        Ok(Self {
            canvas,
            content_scale: 1.0,
        })
    }

    fn replay(&self, commands: &[f32]) -> Result<(), anyhow::Error> {
        let bridge = host_bridge().map_err(super::js_to_anyhow)?;
        let replay = bridge_method_from(&bridge, REPLAY_METHOD).map_err(super::js_to_anyhow)?;
        let payload = Float32Array::new_with_length(commands.len() as u32);
        payload.copy_from(commands);
        replay
            .call2(&bridge, self.canvas.as_ref(), payload.as_ref())
            .map(|_| ())
            .map_err(super::js_to_anyhow)
    }
}

impl CanvasBackend for SelectedBackend {
    fn kind(&self) -> SceneRendererKind {
        SceneRendererKind::CanvasKit
    }

    fn render_scene(&mut self, scene: &SceneGraph, clear_color: ClearColor) -> Result<(), anyhow::Error> {
        let mut commands = vec![CLEAR, clear_color[0], clear_color[1], clear_color[2], clear_color[3]];
        let mut painter = CommandPainter {
            commands: &mut commands,
            content_scale: self.content_scale,
        };
        render_scene_graph(scene, &mut painter);
        self.replay(&commands)
    }

    fn clear(&mut self, clear_color: ClearColor) -> Result<(), anyhow::Error> {
        self.replay(&[CLEAR, clear_color[0], clear_color[1], clear_color[2], clear_color[3]])
    }

    fn resize(&mut self, _width: u32, _height: u32, content_scale: f32) {
        self.content_scale = content_scale.max(1.0);
        if let (Ok(bridge), Ok(resize)) = (host_bridge(), bridge_method(RESIZE_METHOD)) {
            if let Err(error) = resize.call1(&bridge, self.canvas.as_ref()) {
                log::warn!("CanvasKit surface resize failed: {error:?}");
            }
        }
    }
}

struct CommandPainter<'a> {
    commands: &'a mut Vec<f32>,
    content_scale: f32,
}

impl ScenePainter for CommandPainter<'_> {
    fn fill_rect(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: [f32; 4],
        corner_radius: f32,
    ) {
        let scale = self.content_scale;
        self.commands.extend_from_slice(&[
            FILL_RECT,
            x * scale,
            y * scale,
            width * scale,
            height * scale,
            color[0],
            color[1],
            color[2],
            color[3],
            corner_radius * scale,
        ]);
    }

    fn fill_rounded_ring(
        &mut self,
        _x: f32,
        _y: f32,
        _width: f32,
        _height: f32,
        _outer_radius: f32,
        _border_width: f32,
        _color: [f32; 4],
    ) {
    }

    fn stroke_dashed_border(
        &mut self,
        _x: f32,
        _y: f32,
        _width: f32,
        _height: f32,
        _outer_radius: f32,
        _border_width: f32,
        _color: [f32; 4],
    ) {
    }

    fn fill_path(
        &mut self,
        _x: f32,
        _y: f32,
        _verbs: &[PathVerb],
        _fill_rule: DrawFillRule,
        _color: [f32; 4],
    ) {
    }

    fn stroke_path(
        &mut self,
        _x: f32,
        _y: f32,
        _verbs: &[PathVerb],
        _stroke: &StrokeStyle,
        _color: [f32; 4],
    ) {
    }

    fn draw_text_run(&mut self, _x: f32, _y: f32, _color: [f32; 4], _data: &TextRunData) {}

    fn draw_image(
        &mut self,
        _x: f32,
        _y: f32,
        _width: f32,
        _height: f32,
        _data: &RenderImage,
    ) {
    }

    fn push_transform(&mut self, _transform: [f64; 6]) {}

    fn pop_transform(&mut self) {}

    fn push_clip_rect(
        &mut self,
        _x: f32,
        _y: f32,
        _width: f32,
        _height: f32,
        _corner_radii: [f32; 4],
    ) {
    }

    fn push_clip_draw_path(&mut self, _verbs: &[PathVerb]) {}

    fn pop_clip(&mut self) {}
}

fn host_bridge() -> Result<JsValue, JsValue> {
    let bridge = Reflect::get(&js_sys::global(), &JsValue::from_str(HOST_BRIDGE_KEY))?;
    if bridge.is_undefined() || bridge.is_null() {
        return Err(JsValue::from_str("CanvasKit Host bridge unavailable"));
    }
    Ok(bridge)
}

fn bridge_method(name: &str) -> Result<Function, JsValue> {
    bridge_method_from(&host_bridge()?, name)
}

fn bridge_method_from(bridge: &JsValue, name: &str) -> Result<Function, JsValue> {
    Reflect::get(bridge, &JsValue::from_str(name))?
        .dyn_into::<Function>()
        .map_err(|_| JsValue::from_str(&format!("CanvasKit Host bridge method unavailable: {name}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_painter_encodes_a_scaled_filled_rect() {
        let mut commands = Vec::new();
        let mut painter = CommandPainter {
            commands: &mut commands,
            content_scale: 2.0,
        };

        painter.fill_rect(1.0, 2.0, 3.0, 4.0, [0.1, 0.2, 0.3, 1.0], 5.0);

        assert_eq!(
            commands,
            vec![FILL_RECT, 2.0, 4.0, 6.0, 8.0, 0.1, 0.2, 0.3, 1.0, 10.0]
        );
    }
}
