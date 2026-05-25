use std::collections::{HashMap, HashSet};

use hayate_core::{ElementId, ElementKind, ElementTree, ResolvedElement, vello_bridge};
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
    // Maps stable ElementId → live DOM element. ElementId persists for the
    // element's lifetime, so this mapping is correct across structural changes
    // (unlike SceneGraph NodeId which is reassigned on every build).
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

        let resolved = self.tree.resolved_elements();
        let doc = document();
        let mut seen: HashSet<u64> = HashSet::with_capacity(resolved.len());

        for (id, el) in &resolved {
            // Use ElementId as the stable DOM key — valid across structural changes.
            let raw_id = id.data().as_ffi();
            seen.insert(raw_id);

            let dom_el = match self.dom_nodes.get(&raw_id) {
                Some(e) => e.clone(),
                None => {
                    let new_el = doc.create_element("div")?;
                    self.container.append_child(&new_el)?;
                    self.dom_nodes.insert(raw_id, new_el.clone());
                    new_el
                }
            };

            apply_resolved_to_dom(dom_el.unchecked_ref(), el)?;
        }

        // Remove DOM elements whose ElementId is no longer in the tree.
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

fn apply_resolved_to_dom(html_el: &HtmlElement, el: &ResolvedElement) -> Result<(), JsValue> {
    let style = html_el.style();
    style.set_property("position", "absolute")?;
    style.set_property("left", &format!("{}px", el.x))?;
    style.set_property("top", &format!("{}px", el.y))?;
    style.set_property("width", &format!("{}px", el.width))?;
    style.set_property("height", &format!("{}px", el.height))?;
    style.set_property("opacity", &format!("{}", el.opacity))?;

    if el.border_radius > 0.0 {
        style.set_property("border-radius", &format!("{}px", el.border_radius))?;
    } else {
        style.set_property("border-radius", "0")?;
    }

    if let Some(bg) = el.background_color {
        let arr = bg.to_array_f32();
        style.set_property(
            "background-color",
            &format!(
                "rgba({},{},{},{})",
                (arr[0] * 255.0) as u8,
                (arr[1] * 255.0) as u8,
                (arr[2] * 255.0) as u8,
                arr[3],
            ),
        )?;
    } else {
        style.set_property("background-color", "transparent")?;
    }

    if el.border_width > 0.0 {
        let border_color = el.border_color.unwrap_or(hayate_core::Color::BLACK);
        let arr = border_color.to_array_f32();
        style.set_property(
            "border",
            &format!(
                "{}px solid rgba({},{},{},{})",
                el.border_width,
                (arr[0] * 255.0) as u8,
                (arr[1] * 255.0) as u8,
                (arr[2] * 255.0) as u8,
                arr[3],
            ),
        )?;
        style.set_property("box-sizing", "border-box")?;
    } else {
        style.set_property("border", "none")?;
    }

    if let Some(text) = &el.text {
        let arr = el.text_color.to_array_f32();
        style.set_property("font-size", &format!("{}px", el.font_size))?;
        style.set_property(
            "color",
            &format!(
                "rgba({},{},{},{})",
                (arr[0] * 255.0) as u8,
                (arr[1] * 255.0) as u8,
                (arr[2] * 255.0) as u8,
                arr[3],
            ),
        )?;
        style.set_property("white-space", "pre-wrap")?;
        style.set_property("overflow", "hidden")?;
        html_el.set_inner_text(text);
    } else {
        html_el.set_inner_text("");
    }

    Ok(())
}
