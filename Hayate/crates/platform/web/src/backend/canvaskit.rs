//! CanvasKit command-buffer backend (#813/#814).
//!
//! CanvasKit's JS objects and WebGL surface remain Host-owned. This adapter only lowers the
//! existing SceneGraph into a compact frame command stream and calls the opaque Host bridge once.

use hayate_core::SceneGraph;
use js_sys::{Array, Float32Array, Function, Object, Reflect, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::HtmlCanvasElement;

use crate::canvaskit_command::{
    encode_clear, encode_scene, CanvasKitFrame, ResourcePacket, ResourceRegistry,
};
use super::{CanvasBackend, ClearColor, SceneRendererKind};

const HOST_BRIDGE_KEY: &str = "__hayateCanvasKitBridge";
const REPLAY_METHOD: &str = "replay";
const RESIZE_METHOD: &str = "resize";

pub(crate) struct SelectedBackend {
    canvas: HtmlCanvasElement,
    content_scale: f32,
    resources: ResourceRegistry,
}

impl SelectedBackend {
    pub(crate) fn init(canvas: HtmlCanvasElement) -> Result<Self, JsValue> {
        // The generated Host loader prepares the CanvasKit surface before importing this WASM
        // target. Failing here makes it an init failure, so #811's boot-only policy can advance.
        bridge_method(REPLAY_METHOD)?;
        Ok(Self {
            canvas,
            content_scale: 1.0,
            resources: ResourceRegistry::default(),
        })
    }

    fn replay(&self, frame: &CanvasKitFrame) -> Result<(), anyhow::Error> {
        let bridge = host_bridge().map_err(super::js_to_anyhow)?;
        let replay = bridge_method_from(&bridge, REPLAY_METHOD).map_err(super::js_to_anyhow)?;
        let payload = Float32Array::new_with_length(frame.commands.len() as u32);
        payload.copy_from(&frame.commands);
        let resources = Array::new();
        for packet in &frame.resources {
            resources.push(&resource_js_value(packet).map_err(super::js_to_anyhow)?);
        }
        replay
            .call3(&bridge, self.canvas.as_ref(), payload.as_ref(), resources.as_ref())
            .map(|_| ())
            .map_err(super::js_to_anyhow)
    }
}

impl CanvasBackend for SelectedBackend {
    fn kind(&self) -> SceneRendererKind {
        SceneRendererKind::CanvasKit
    }

    fn render_scene(&mut self, scene: &SceneGraph, clear_color: ClearColor) -> Result<(), anyhow::Error> {
        let frame = encode_scene(scene, clear_color, self.content_scale, &mut self.resources);
        self.replay(&frame)
    }

    fn clear(&mut self, clear_color: ClearColor) -> Result<(), anyhow::Error> {
        self.replay(&encode_clear(clear_color))
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

fn resource_js_value(packet: &ResourcePacket) -> Result<JsValue, JsValue> {
    let object = Object::new();
    let set = |key: &str, value: &JsValue| Reflect::set(&object, &JsValue::from_str(key), value).map(|_| ());
    match packet {
        ResourcePacket::Font { id, bytes } => {
            set("kind", &JsValue::from_str("font"))?;
            set("id", &JsValue::from_f64(*id as f64))?;
            set("bytes", Uint8Array::from(bytes.as_slice()).as_ref())?;
        }
        ResourcePacket::Image { id, width, height, alpha_type, bytes } => {
            set("kind", &JsValue::from_str("image"))?;
            set("id", &JsValue::from_f64(*id as f64))?;
            set("width", &JsValue::from_f64(*width as f64))?;
            set("height", &JsValue::from_f64(*height as f64))?;
            set("alphaType", &JsValue::from_f64(*alpha_type as f64))?;
            set("bytes", Uint8Array::from(bytes.as_slice()).as_ref())?;
        }
    }
    Ok(object.into())
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
