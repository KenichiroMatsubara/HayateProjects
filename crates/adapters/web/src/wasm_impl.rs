use std::collections::HashMap;

use hayate_core::{Node, NodeKind, SceneGraph, vello_bridge};
use slotmap::{Key, KeyData};
use vello::{Scene, peniko::color::{AlphaColor, Srgb}};
use wasm_bindgen::prelude::*;
use web_sys::{Document, Element, HtmlCanvasElement, HtmlElement};

use crate::gpu_surface::GpuSurface;

fn document() -> Document {
    web_sys::window().unwrap().document().unwrap()
}

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Debug);
}

/// Raw Layer renderer — exposes direct Rect node mutation backed by Vello.
#[wasm_bindgen]
pub struct HayateRenderer {
    gpu: GpuSurface,
    scene_graph: SceneGraph,
}

#[wasm_bindgen]
impl HayateRenderer {
    /// Initialise wgpu (WebGPU) + Vello from an HTML canvas element.
    /// Returns a `Promise<HayateRenderer>` because GPU requests are async.
    pub async fn init(canvas: HtmlCanvasElement) -> Result<HayateRenderer, JsValue> {
        let gpu = GpuSurface::init(canvas).await?;
        Ok(HayateRenderer { gpu, scene_graph: SceneGraph::new() })
    }

    /// Add a Rect node to the scene graph. Returns an opaque node ID (as f64).
    pub fn node_create(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        r: f32,
        g: f32,
        b: f32,
        a: f32,
        corner_radius: f32,
    ) -> f64 {
        let node = Node {
            kind: NodeKind::Rect { x, y, width, height, color: [r, g, b, a], corner_radius },
            children: Vec::new(),
        };
        let id = self.scene_graph.insert(node);
        id.data().as_ffi() as f64
    }

    /// Remove a node previously created with node_create.
    pub fn node_remove(&mut self, raw_id: f64) {
        use hayate_core::NodeId;
        let key_data = KeyData::from_ffi(raw_id as u64);
        let id = NodeId::from(key_data);
        self.scene_graph.remove(id);
    }

    /// Render the current scene graph to the canvas.
    pub fn render(&mut self, bg_r: f64, bg_g: f64, bg_b: f64) -> Result<(), JsValue> {
        let base_color = AlphaColor::<Srgb>::new([bg_r as f32, bg_g as f32, bg_b as f32, 1.0]);
        let scene = vello_bridge::build_scene(&self.scene_graph);
        self.gpu.present(&scene, base_color)
    }

    /// Clear the canvas to an RGB solid colour (components in 0.0 – 1.0).
    pub fn clear(&mut self, r: f64, g: f64, b: f64) -> Result<(), JsValue> {
        let base_color = AlphaColor::<Srgb>::new([r as f32, g as f32, b as f32, 1.0]);
        let scene = Scene::new();
        self.gpu.present(&scene, base_color)
    }
}

/// HTML fallback renderer — maps Rect nodes to absolutely-positioned divs.
/// Same JS-facing API as `HayateRenderer` so calling code doesn't need to branch.
#[wasm_bindgen]
pub struct HayateHtmlRenderer {
    container: HtmlElement,
    nodes: HashMap<u64, Element>,
    next_id: u64,
}

#[wasm_bindgen]
impl HayateHtmlRenderer {
    /// Create an HTML renderer backed by `container` (a div).
    /// Sets `position:relative; overflow:hidden` on the container.
    pub fn new(container: HtmlElement) -> Result<HayateHtmlRenderer, JsValue> {
        let style = container.style();
        style.set_property("position", "relative")?;
        style.set_property("overflow", "hidden")?;
        Ok(HayateHtmlRenderer { container, nodes: HashMap::new(), next_id: 1 })
    }

    /// Add a Rect node. Returns an opaque node ID (as f64) matching `HayateRenderer`'s API.
    pub fn node_create(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        r: f32,
        g: f32,
        b: f32,
        a: f32,
        corner_radius: f32,
    ) -> Result<f64, JsValue> {
        let doc = document();
        let el = doc.create_element("div")?;
        let html_el = el.unchecked_ref::<HtmlElement>();
        let style = html_el.style();
        style.set_property("position", "absolute")?;
        style.set_property("left", &format!("{}px", x))?;
        style.set_property("top", &format!("{}px", y))?;
        style.set_property("width", &format!("{}px", width))?;
        style.set_property("height", &format!("{}px", height))?;
        style.set_property(
            "background-color",
            &format!("rgba({},{},{},{})", (r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8, a),
        )?;
        if corner_radius > 0.0 {
            style.set_property("border-radius", &format!("{}px", corner_radius))?;
        }
        self.container.append_child(&el)?;

        let id = self.next_id;
        self.next_id += 1;
        self.nodes.insert(id, el);
        Ok(id as f64)
    }

    /// Remove a node previously created with `node_create`.
    pub fn node_remove(&mut self, raw_id: f64) {
        let id = raw_id as u64;
        if let Some(el) = self.nodes.remove(&id) {
            let _ = self.container.remove_child(&el);
        }
    }

    /// Update the container's background colour. DOM nodes update instantly so no repaint needed.
    pub fn render(&self, bg_r: f64, bg_g: f64, bg_b: f64) -> Result<(), JsValue> {
        self.container.style().set_property(
            "background-color",
            &format!("rgb({},{},{})", (bg_r * 255.0) as u8, (bg_g * 255.0) as u8, (bg_b * 255.0) as u8),
        )
    }
}
