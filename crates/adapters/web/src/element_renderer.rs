use std::collections::{HashMap, HashSet};

use hayate_core::{ElementId, ElementKind, ElementTree, NodeKind, vello_bridge};
use slotmap::{Key, KeyData};
use vello::peniko::color::{AlphaColor, Srgb};
use wasm_bindgen::prelude::*;
use web_sys::{Document, Element, HtmlCanvasElement, HtmlElement};

use crate::gpu_surface::GpuSurface;
use crate::style_packet;

fn document() -> Document {
    web_sys::window().unwrap().document().unwrap()
}

fn element_id_from_f64(raw: f64) -> ElementId {
    ElementId::from(KeyData::from_ffi(raw as u64))
}

fn element_id_to_f64(id: ElementId) -> f64 {
    id.data().as_ffi() as f64
}

fn kind_from_u32(v: u32) -> Result<ElementKind, JsValue> {
    ElementKind::from_u32(v).ok_or_else(|| JsValue::from_str(&format!("unknown element kind {v}")))
}

// ── Element kind discriminant getters (exposed to JS) ────────────────────

#[wasm_bindgen]
pub fn element_kind_view() -> u32 { 0 }
#[wasm_bindgen]
pub fn element_kind_text() -> u32 { 1 }
#[wasm_bindgen]
pub fn element_kind_image() -> u32 { 2 }
#[wasm_bindgen]
pub fn element_kind_button() -> u32 { 3 }
#[wasm_bindgen]
pub fn element_kind_text_input() -> u32 { 4 }
#[wasm_bindgen]
pub fn element_kind_scroll_view() -> u32 { 5 }

// ── Canvas Mode renderer ─────────────────────────────────────────────────

#[wasm_bindgen]
pub struct HayateElementRenderer {
    gpu: GpuSurface,
    tree: ElementTree,
}

#[wasm_bindgen]
impl HayateElementRenderer {
    pub async fn init(canvas: HtmlCanvasElement) -> Result<HayateElementRenderer, JsValue> {
        let width = canvas.width() as f32;
        let height = canvas.height() as f32;
        let gpu = GpuSurface::init(canvas).await?;
        let mut tree = ElementTree::new();
        tree.set_viewport(width, height);
        Ok(Self { gpu, tree })
    }

    pub fn set_viewport(&mut self, width: f32, height: f32) {
        self.tree.set_viewport(width, height);
    }

    pub fn element_create(&mut self, kind: u32) -> Result<f64, JsValue> {
        let k = kind_from_u32(kind)?;
        Ok(element_id_to_f64(self.tree.element_create(k)))
    }

    pub fn element_set_text(&mut self, id: f64, text: &str) {
        self.tree.element_set_text(element_id_from_f64(id), text);
    }

    pub fn element_set_src(&mut self, id: f64, url: &str) {
        self.tree.element_set_src(element_id_from_f64(id), url);
    }

    pub fn element_set_style(&mut self, id: f64, packed: &[f32]) -> Result<(), JsValue> {
        let props = style_packet::decode(packed)?;
        self.tree.element_set_style(element_id_from_f64(id), &props);
        Ok(())
    }

    pub fn element_append_child(&mut self, parent: f64, child: f64) {
        self.tree
            .element_append_child(element_id_from_f64(parent), element_id_from_f64(child));
    }

    pub fn element_insert_before(&mut self, parent: f64, child: f64, before: f64) {
        self.tree.element_insert_before(
            element_id_from_f64(parent),
            element_id_from_f64(child),
            element_id_from_f64(before),
        );
    }

    pub fn element_remove(&mut self, id: f64) {
        self.tree.element_remove(element_id_from_f64(id));
    }

    pub fn element_get_text(&self, id: f64) -> String {
        self.tree.element_get_text(element_id_from_f64(id))
    }

    pub fn set_root(&mut self, id: f64) {
        self.tree.set_root(element_id_from_f64(id));
    }

    pub fn render(&mut self, bg_r: f64, bg_g: f64, bg_b: f64) -> Result<(), JsValue> {
        let base_color = AlphaColor::<Srgb>::new([bg_r as f32, bg_g as f32, bg_b as f32, 1.0]);
        let sg = self.tree.render();
        let scene = vello_bridge::build_scene(sg);
        self.gpu.present(&scene, base_color)
    }
}

// ── HTML Mode renderer ───────────────────────────────────────────────────

#[wasm_bindgen]
pub struct HayateElementHtmlRenderer {
    container: HtmlElement,
    tree: ElementTree,
    // Maps SceneGraph slotmap key → live DOM element so we can diff between frames.
    dom_nodes: HashMap<u64, Element>,
}

#[wasm_bindgen]
impl HayateElementHtmlRenderer {
    pub fn new(container: HtmlElement) -> Result<HayateElementHtmlRenderer, JsValue> {
        let style = container.style();
        style.set_property("position", "relative")?;
        style.set_property("overflow", "hidden")?;
        let width = container.client_width().max(1) as f32;
        let height = container.client_height().max(1) as f32;
        let mut tree = ElementTree::new();
        tree.set_viewport(width, height);
        Ok(Self { container, tree, dom_nodes: HashMap::new() })
    }

    pub fn set_viewport(&mut self, width: f32, height: f32) {
        self.tree.set_viewport(width, height);
    }

    pub fn element_create(&mut self, kind: u32) -> Result<f64, JsValue> {
        let k = kind_from_u32(kind)?;
        Ok(element_id_to_f64(self.tree.element_create(k)))
    }

    pub fn element_set_text(&mut self, id: f64, text: &str) {
        self.tree.element_set_text(element_id_from_f64(id), text);
    }

    pub fn element_set_src(&mut self, id: f64, url: &str) {
        self.tree.element_set_src(element_id_from_f64(id), url);
    }

    pub fn element_set_style(&mut self, id: f64, packed: &[f32]) -> Result<(), JsValue> {
        let props = style_packet::decode(packed)?;
        self.tree.element_set_style(element_id_from_f64(id), &props);
        Ok(())
    }

    pub fn element_append_child(&mut self, parent: f64, child: f64) {
        self.tree
            .element_append_child(element_id_from_f64(parent), element_id_from_f64(child));
    }

    pub fn element_insert_before(&mut self, parent: f64, child: f64, before: f64) {
        self.tree.element_insert_before(
            element_id_from_f64(parent),
            element_id_from_f64(child),
            element_id_from_f64(before),
        );
    }

    pub fn element_remove(&mut self, id: f64) {
        self.tree.element_remove(element_id_from_f64(id));
    }

    pub fn element_get_text(&self, id: f64) -> String {
        self.tree.element_get_text(element_id_from_f64(id))
    }

    pub fn set_root(&mut self, id: f64) {
        self.tree.set_root(element_id_from_f64(id));
    }

    pub fn render(&mut self, bg_r: f64, bg_g: f64, bg_b: f64) -> Result<(), JsValue> {
        self.container.style().set_property(
            "background-color",
            &format!(
                "rgb({},{},{})",
                (bg_r * 255.0) as u8,
                (bg_g * 255.0) as u8,
                (bg_b * 255.0) as u8,
            ),
        )?;

        let sg = self.tree.render();
        let doc = document();
        let mut seen: HashSet<u64> = HashSet::with_capacity(sg.len());

        for (key, node) in sg.iter() {
            let raw_key = key.data().as_ffi();
            seen.insert(raw_key);
            let el = match self.dom_nodes.get(&raw_key) {
                Some(e) => e.clone(),
                None => {
                    let el = doc.create_element("div")?;
                    self.container.append_child(&el)?;
                    self.dom_nodes.insert(raw_key, el.clone());
                    el
                }
            };
            let html_el = el.unchecked_ref::<HtmlElement>();
            let style = html_el.style();
            match &node.kind {
                NodeKind::Rect { x, y, width, height, color, corner_radius } => {
                    style.set_property("position", "absolute")?;
                    style.set_property("left", &format!("{}px", x))?;
                    style.set_property("top", &format!("{}px", y))?;
                    style.set_property("width", &format!("{}px", width))?;
                    style.set_property("height", &format!("{}px", height))?;
                    style.set_property(
                        "background-color",
                        &format!(
                            "rgba({},{},{},{})",
                            (color[0] * 255.0) as u8,
                            (color[1] * 255.0) as u8,
                            (color[2] * 255.0) as u8,
                            color[3],
                        ),
                    )?;
                    if *corner_radius > 0.0 {
                        style.set_property("border-radius", &format!("{}px", corner_radius))?;
                    } else {
                        style.set_property("border-radius", "0")?;
                    }
                    html_el.set_inner_text("");
                }
                NodeKind::TextRun { x, y, color, data } => {
                    style.set_property("position", "absolute")?;
                    style.set_property("left", &format!("{}px", x))?;
                    style.set_property("top", &format!("{}px", y))?;
                    style.set_property("font-size", &format!("{}px", data.font_size))?;
                    style.set_property("white-space", "pre")?;
                    style.set_property("pointer-events", "none")?;
                    style.set_property(
                        "color",
                        &format!(
                            "rgba({},{},{},{})",
                            (color[0] * 255.0) as u8,
                            (color[1] * 255.0) as u8,
                            (color[2] * 255.0) as u8,
                            color[3],
                        ),
                    )?;
                    style.set_property("background-color", "transparent")?;
                    style.set_property("border-radius", "0")?;
                    html_el.set_inner_text(&data.text);
                }
            }
        }

        // Drop DOM nodes whose slotmap key disappeared this frame.
        let stale: Vec<u64> = self
            .dom_nodes
            .keys()
            .copied()
            .filter(|k| !seen.contains(k))
            .collect();
        for k in stale {
            if let Some(el) = self.dom_nodes.remove(&k) {
                let _ = self.container.remove_child(&el);
            }
        }

        Ok(())
    }
}
