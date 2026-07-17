//! CanvasKit command-buffer backend (#813/#814).
//!
//! CanvasKit's JS objects and WebGL surface remain Host-owned. This adapter only lowers the
//! existing SceneGraph into compact full-frame or dirty-layer command streams and calls the opaque
//! Host bridge. CanvasKit surfaces/images remain behind that bridge.

use std::collections::{HashMap, HashSet};

use hayate_core::element::id::ElementId;
use hayate_core::SceneGraph;
use hayate_layer_compositor::layer_scene::{
    collect_layer_placements, compose, extract_layer_scene, extract_root_scene,
    extract_scroll_layer_scene,
};
use hayate_layer_compositor::{PresentPlanner, ScrollLayerGeometry};
use js_sys::{Array, Float32Array, Float64Array, Function, Object, Reflect, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::HtmlCanvasElement;

use super::{CanvasBackend, ClearColor, SceneRendererKind};
use crate::canvaskit_command::{
    encode_clear, encode_scene, encode_scene_at, CanvasKitFrame, ResourcePacket, ResourceRegistry,
};

const HOST_BRIDGE_KEY: &str = "__hayateCanvasKitBridge";
const REPLAY_METHOD: &str = "replay";
const REPLAY_LAYER_METHOD: &str = "replayLayer";
const COMPOSITE_LAYERS_METHOD: &str = "compositeLayers";
const RESIZE_METHOD: &str = "resize";
const LAYER_PLACEMENT_SLOTS: usize = 12;
const TRANSPARENT: ClearColor = [0.0, 0.0, 0.0, 0.0];

pub(crate) struct SelectedBackend {
    canvas: HtmlCanvasElement,
    content_scale: f32,
    resources: ResourceRegistry,
    command_payload: Option<Float32Array>,
    layer_payloads: HashMap<ElementId, Float32Array>,
    resource_packets: Array,
    placement_payload: Option<Float64Array>,
    background_payload: Float32Array,
    planner: PresentPlanner,
    prev_layers: HashSet<ElementId>,
    layer_present_enabled: bool,
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
            command_payload: None,
            layer_payloads: HashMap::new(),
            resource_packets: Array::new(),
            placement_payload: None,
            background_payload: Float32Array::new_with_length(4),
            planner: PresentPlanner::new(),
            prev_layers: HashSet::new(),
            layer_present_enabled: true,
        })
    }

    fn replay_layer(
        &mut self,
        layer: ElementId,
        frame: &CanvasKitFrame,
    ) -> Result<(), anyhow::Error> {
        let bridge = host_bridge().map_err(super::js_to_anyhow)?;
        let replay =
            bridge_method_from(&bridge, REPLAY_LAYER_METHOD).map_err(super::js_to_anyhow)?;
        self.fill_resource_packets(frame)?;
        let payload = self
            .layer_payloads
            .entry(layer)
            .or_insert_with(|| Float32Array::new_with_length(frame.commands.len() as u32));
        if payload.length() != frame.commands.len() as u32 {
            *payload = Float32Array::new_with_length(frame.commands.len() as u32);
        }
        payload.copy_from(&frame.commands);
        replay
            .call5(
                &bridge,
                self.canvas.as_ref(),
                &JsValue::from_f64(layer.to_u64() as f64),
                payload.as_ref(),
                self.resource_packets.as_ref(),
                &JsValue::from_f64(frame.commands.len() as f64),
            )
            .map(|_| ())
            .map_err(super::js_to_anyhow)
    }

    fn fill_resource_packets(&mut self, frame: &CanvasKitFrame) -> Result<(), anyhow::Error> {
        self.resource_packets.set_length(0);
        for packet in &frame.resources {
            self.resource_packets
                .push(&resource_js_value(packet).map_err(super::js_to_anyhow)?);
        }
        Ok(())
    }

    fn replay(&mut self, frame: &CanvasKitFrame) -> Result<(), anyhow::Error> {
        let bridge = host_bridge().map_err(super::js_to_anyhow)?;
        let replay = bridge_method_from(&bridge, REPLAY_METHOD).map_err(super::js_to_anyhow)?;
        let payload = self
            .command_payload
            .get_or_insert_with(|| Float32Array::new_with_length(frame.commands.len() as u32));
        if payload.length() != frame.commands.len() as u32 {
            *payload = Float32Array::new_with_length(frame.commands.len() as u32);
        }
        payload.copy_from(&frame.commands);
        self.resource_packets.set_length(0);
        for packet in &frame.resources {
            self.resource_packets
                .push(&resource_js_value(packet).map_err(super::js_to_anyhow)?);
        }
        replay
            .call4(
                &bridge,
                self.canvas.as_ref(),
                payload.as_ref(),
                self.resource_packets.as_ref(),
                &JsValue::from_f64(frame.commands.len() as f64),
            )
            .map(|_| ())
            .map_err(super::js_to_anyhow)
    }
}

impl CanvasBackend for SelectedBackend {
    fn kind(&self) -> SceneRendererKind {
        SceneRendererKind::CanvasKit
    }

    fn render_scene(
        &mut self,
        scene: &SceneGraph,
        clear_color: ClearColor,
    ) -> Result<(), anyhow::Error> {
        let frame = encode_scene(scene, clear_color, self.content_scale, &mut self.resources);
        self.replay(&frame)
    }

    fn clear(&mut self, clear_color: ClearColor) -> Result<(), anyhow::Error> {
        let frame = encode_clear(clear_color);
        self.replay(&frame)
    }

    fn supports_layer_present(&self) -> bool {
        self.layer_present_enabled
    }

    fn set_layer_present_enabled(&mut self, enabled: bool) {
        self.layer_present_enabled = enabled;
    }

    fn present_layers(
        &mut self,
        scene: &SceneGraph,
        layers: &[ElementId],
        layer_dirty: &HashSet<ElementId>,
        scroll_geometry: &HashMap<ElementId, ScrollLayerGeometry>,
        clear_color: ClearColor,
    ) -> Result<(), anyhow::Error> {
        let Some(&root) = layers.first() else {
            return self.clear(clear_color);
        };
        let boundaries: HashSet<ElementId> = layers.iter().copied().collect();
        for stale in self
            .prev_layers
            .difference(&boundaries)
            .copied()
            .collect::<Vec<_>>()
        {
            self.planner.evict(stale);
            self.layer_payloads.remove(&stale);
        }
        self.prev_layers = boundaries.clone();

        let non_scroll_layers: Vec<ElementId> = layers
            .iter()
            .copied()
            .filter(|layer| !scroll_geometry.contains_key(layer))
            .collect();
        let plan = self.planner.plan_layers(&non_scroll_layers, layer_dirty);
        for &layer in &plan.raster {
            let extracted = if layer == root {
                extract_root_scene(scene, root, &boundaries)
            } else {
                extract_layer_scene(scene, layer, &boundaries).ok_or_else(|| {
                    anyhow::anyhow!("CanvasKit layer {} is missing", layer.to_u64())
                })?
            };
            let frame = encode_scene(
                &extracted,
                TRANSPARENT,
                self.content_scale,
                &mut self.resources,
            );
            self.replay_layer(layer, &frame)?;
            let bytes = u64::from(self.canvas.width())
                * u64::from(self.canvas.height())
                * hayate_layer_compositor::tunables::BYTES_PER_PIXEL;
            self.planner.note_layer_rasterized(layer, bytes);
        }

        for &layer in layers {
            let Some(geometry) = scroll_geometry.get(&layer) else {
                continue;
            };
            if !geometry.content_dirty
                && !self.planner.scroll_layer_needs_raster(
                    layer,
                    geometry.visible_top,
                    geometry.viewport_height,
                )
            {
                continue;
            }
            let extracted = if layer == root {
                extract_root_scene(scene, root, &boundaries)
            } else {
                extract_scroll_layer_scene(scene, layer, &boundaries, geometry.scroll_affine)
                    .ok_or_else(|| {
                        anyhow::anyhow!("CanvasKit layer {} is missing", layer.to_u64())
                    })?
            };
            // CanvasKit uses a full-surface compatible offscreen Surface. When the requested
            // overscan band is taller than that fixed surface, slide the recorded band around the
            // current viewport instead of truncating only its tail. Otherwise a scroll can expose
            // rows beyond the texture even on the very frame that re-rasterized it.
            let cached_band = geometry.band.fit_to_capacity(
                geometry.visible_top,
                geometry.viewport_height,
                self.canvas.height() as f32 / self.content_scale,
            );
            let origin_y = geometry.absolute_top + cached_band.top;
            let frame = encode_scene_at(
                &extracted,
                TRANSPARENT,
                self.content_scale,
                origin_y,
                &mut self.resources,
            );
            self.replay_layer(layer, &frame)?;
            let bytes = u64::from(self.canvas.width())
                * u64::from(self.canvas.height())
                * hayate_layer_compositor::tunables::BYTES_PER_PIXEL;
            self.planner
                .note_scroll_rasterized(layer, cached_band, bytes);
        }

        let placements = collect_layer_placements(scene, root, &boundaries);
        let mut values = Vec::with_capacity(placements.len() * LAYER_PLACEMENT_SLOTS);
        for placement in &placements {
            values.push(placement.layer.to_u64() as f64);
            let transform = match (
                self.planner.cached_scroll_band(placement.layer),
                scroll_geometry.get(&placement.layer),
            ) {
                (Some(cached_band), Some(geometry)) => compose(
                    placement.transform,
                    geometry.composite_affine_for_band(cached_band),
                ),
                _ => placement.transform,
            };
            values.extend_from_slice(&transform);
            if let Some([x, y, width, height]) = placement.clip {
                values.extend_from_slice(&[1.0, x as f64, y as f64, width as f64, height as f64]);
            } else {
                values.extend_from_slice(&[0.0; 5]);
            }
        }
        let payload = self
            .placement_payload
            .get_or_insert_with(|| Float64Array::new_with_length(values.len() as u32));
        if payload.length() != values.len() as u32 {
            *payload = Float64Array::new_with_length(values.len() as u32);
        }
        payload.copy_from(&values);
        self.background_payload.copy_from(&clear_color);
        let bridge = host_bridge().map_err(super::js_to_anyhow)?;
        let composite =
            bridge_method_from(&bridge, COMPOSITE_LAYERS_METHOD).map_err(super::js_to_anyhow)?;
        composite
            .call4(
                &bridge,
                self.canvas.as_ref(),
                payload.as_ref(),
                self.background_payload.as_ref(),
                &JsValue::from_f64(self.content_scale as f64),
            )
            .map_err(super::js_to_anyhow)?;
        for placement in &placements {
            self.planner.note_composited(placement.layer);
        }
        Ok(())
    }

    fn resize(&mut self, _width: u32, _height: u32, content_scale: f32) {
        self.content_scale = content_scale.max(1.0);
        self.planner.invalidate();
        self.prev_layers.clear();
        self.layer_payloads.clear();
        self.placement_payload = None;
        if let (Ok(bridge), Ok(resize)) = (host_bridge(), bridge_method(RESIZE_METHOD)) {
            if let Err(error) = resize.call1(&bridge, self.canvas.as_ref()) {
                log::warn!("CanvasKit surface resize failed: {error:?}");
            }
        }
    }
}

fn resource_js_value(packet: &ResourcePacket) -> Result<JsValue, JsValue> {
    let object = Object::new();
    let set = |key: &str, value: &JsValue| {
        Reflect::set(&object, &JsValue::from_str(key), value).map(|_| ())
    };
    match packet {
        ResourcePacket::Font { id, bytes } => {
            set("kind", &JsValue::from_str("font"))?;
            set("id", &JsValue::from_f64(*id as f64))?;
            set("bytes", Uint8Array::from(bytes.as_slice()).as_ref())?;
        }
        ResourcePacket::Image {
            id,
            width,
            height,
            alpha_type,
            bytes,
        } => {
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
        .map_err(|_| {
            JsValue::from_str(&format!("CanvasKit Host bridge method unavailable: {name}"))
        })
}
