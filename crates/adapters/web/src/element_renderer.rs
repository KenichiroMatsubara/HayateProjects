use std::sync::Arc;

use hayate_core::{ElementId, ElementKind, ElementTree, Event, vello_bridge};
use slotmap::{Key, KeyData, SlotMap};
use vello::peniko::{Blob, ImageAlphaType, ImageData, ImageFormat, color::{AlphaColor, Srgb}};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Document, Element, HtmlCanvasElement, HtmlElement, HtmlInputElement, Node};

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

// ── Style tag constants (exposed to JS) ──────────────────────────────────

#[wasm_bindgen] pub fn style_tag_z_index() -> u32 { crate::style_packet::TAG_Z_INDEX }

// ── Event kind constants (exposed to JS) ─────────────────────────────────

#[wasm_bindgen] pub fn event_kind_click()               -> f64 { 0.0 }
#[wasm_bindgen] pub fn event_kind_focus()               -> f64 { 1.0 }
#[wasm_bindgen] pub fn event_kind_blur()                -> f64 { 2.0 }
#[wasm_bindgen] pub fn event_kind_text_input()          -> f64 { 3.0 }
#[wasm_bindgen] pub fn event_kind_composition_start()   -> f64 { 4.0 }
#[wasm_bindgen] pub fn event_kind_composition_update()  -> f64 { 5.0 }
#[wasm_bindgen] pub fn event_kind_composition_end()     -> f64 { 6.0 }
#[wasm_bindgen] pub fn event_kind_scroll()              -> f64 { 7.0 }
#[wasm_bindgen] pub fn event_kind_resize()              -> f64 { 8.0 }
#[wasm_bindgen] pub fn event_kind_pointer_up()          -> f64 { 9.0 }
#[wasm_bindgen] pub fn event_kind_pointer_enter()       -> f64 { 10.0 }
#[wasm_bindgen] pub fn event_kind_pointer_leave()       -> f64 { 11.0 }
#[wasm_bindgen] pub fn event_kind_key_down()            -> f64 { 12.0 }

// ── Modifier key bitmask constants (exposed to JS) ───────────────────────
// Match KeyboardEvent.getModifierState flags for JS interop.

#[wasm_bindgen] pub fn modifier_shift() -> u32 { 1 }
#[wasm_bindgen] pub fn modifier_ctrl()  -> u32 { 2 }
#[wasm_bindgen] pub fn modifier_alt()   -> u32 { 4 }
#[wasm_bindgen] pub fn modifier_meta()  -> u32 { 8 }

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
    focused_element: Option<ElementId>,
    hovered_element: Option<ElementId>,
    last_pointer_pos: Option<(f32, f32)>,
}

#[wasm_bindgen]
impl HayateElementRenderer {
    pub async fn init(canvas: HtmlCanvasElement) -> Result<HayateElementRenderer, JsValue> {
        let width = canvas.width() as f32;
        let height = canvas.height() as f32;
        let gpu = GpuSurface::init(canvas).await?;
        let mut tree = ElementTree::new();
        tree.set_viewport(width, height);
        Ok(Self { gpu, tree, focused_element: None, hovered_element: None, last_pointer_pos: None })
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

    /// Set a 2D affine transform on the element. Pass exactly 6 f64 coefficients [a,b,c,d,e,f],
    /// or an empty slice to clear.
    pub fn element_set_transform(&mut self, id: f64, matrix: &[f64]) {
        let m = if matrix.len() == 6 {
            Some([matrix[0], matrix[1], matrix[2], matrix[3], matrix[4], matrix[5]])
        } else {
            None
        };
        self.tree.element_set_transform(element_id_from_f64(id), m);
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

    /// Fetch an image (PNG / JPEG / WebP) from `url` and attach it to the Image element.
    /// Call this after element_set_src; the element renders blank until this resolves.
    pub async fn load_image(&mut self, id: f64, url: String) -> Result<(), JsValue> {
        let eid = element_id_from_f64(id);
        let image_data = fetch_image(&url).await?;
        self.tree.element_set_image(eid, Arc::new(image_data));
        Ok(())
    }

    pub fn on_pointer_down(&mut self, x: f32, y: f32) {
        let hit = self.tree.hit_test(x, y);
        if let Some(target) = hit {
            self.tree.push_event(Event::Click { target, x, y });
            if self.focused_element != hit {
                if let Some(prev) = self.focused_element {
                    self.tree.push_event(Event::Blur(prev));
                    self.tree.element_set_cursor_visible(prev, false);
                }
                self.focused_element = hit;
                self.tree.push_event(Event::Focus(target));
                self.tree.element_set_cursor_visible(target, true);
            }
        } else if let Some(prev) = self.focused_element.take() {
            self.tree.push_event(Event::Blur(prev));
            self.tree.element_set_cursor_visible(prev, false);
        }
    }

    pub fn on_pointer_up(&mut self, x: f32, y: f32) {
        if let Some(target) = self.tree.hit_test(x, y) {
            self.tree.push_event(Event::PointerUp { target, x, y });
        }
    }

    pub fn on_pointer_move(&mut self, x: f32, y: f32) {
        // Skip if position hasn't moved by at least 1px (P3 throttle).
        if let Some((lx, ly)) = self.last_pointer_pos {
            if (x - lx).abs() < 1.0 && (y - ly).abs() < 1.0 {
                return;
            }
        }
        self.last_pointer_pos = Some((x, y));
        let hit = self.tree.hit_test(x, y);
        if hit != self.hovered_element {
            if let Some(prev) = self.hovered_element {
                self.tree.push_event(Event::PointerLeave { target: prev });
            }
            if let Some(cur) = hit {
                self.tree.push_event(Event::PointerEnter { target: cur });
            }
            self.hovered_element = hit;
        }
    }

    pub fn on_wheel(&mut self, x: f32, y: f32, delta_x: f32, delta_y: f32) {
        if let Some(target) = self.tree.hit_test(x, y) {
            if let Some(sv) = nearest_scroll_view(&self.tree, target) {
                let (ox, oy) = self.tree.element_get_scroll_offset(sv);
                let (content_w, content_h) = self.tree.element_content_size(sv);
                let sv_rect = self.tree.element_layout_rect(sv).unwrap_or((0.0, 0.0, 0.0, 0.0));
                let max_x = (content_w - sv_rect.2).max(0.0);
                let max_y = (content_h - sv_rect.3).max(0.0);
                let new_x = (ox + delta_x).clamp(0.0, max_x);
                let new_y = (oy + delta_y).clamp(0.0, max_y);
                self.tree.element_set_scroll_offset(sv, new_x, new_y);
            }
            self.tree.push_event(Event::Scroll { target, delta_x, delta_y });
        }
    }

    pub fn on_resize(&mut self, width: f32, height: f32) {
        self.tree.set_viewport(width, height);
        self.tree.push_event(Event::Resize { width, height });
    }

    pub fn element_set_scroll_offset(&mut self, id: f64, x: f32, y: f32) {
        self.tree.element_set_scroll_offset(element_id_from_f64(id), x, y);
    }

    pub fn element_set_font_family(&mut self, id: f64, family: &str) {
        self.tree.element_set_font_family(element_id_from_f64(id), family);
    }

    pub fn element_set_aria_label(&mut self, id: f64, label: &str) {
        self.tree.element_set_aria_label(element_id_from_f64(id), label);
    }

    pub fn element_set_role(&mut self, id: f64, role: &str) {
        self.tree.element_set_role(element_id_from_f64(id), role);
    }

    /// Register a custom font from raw bytes. After this, the family_name can be used
    /// with `element_set_font_family`.
    pub fn register_font_bytes(&mut self, family_name: &str, data: &[u8]) {
        self.tree.register_font(family_name, data.to_vec());
    }

    /// Fetch a font file from a URL and register it under `family_name`.
    pub async fn load_font_from_url(&mut self, family_name: String, url: String) -> Result<(), JsValue> {
        let bytes = fetch_bytes(&url).await?;
        self.tree.register_font(&family_name, bytes);
        Ok(())
    }

    /// Paste text into the currently focused element. JS calls this from the paste event handler.
    pub fn on_clipboard_paste(&mut self, text: &str) {
        if let Some(focused) = self.focused_element {
            self.tree.element_append_text_content(focused, text);
            self.tree.push_event(Event::TextInput { target: focused, text: text.to_string() });
        }
    }

    /// Return the focused element's id (as f64), or 0.0 if nothing is focused.
    /// JS can use this with `element_get_text_content` to implement copy/cut.
    pub fn focused_element_id(&self) -> f64 {
        self.focused_element.map(element_id_to_f64).unwrap_or(0.0)
    }

    /// Handle a key press on the focused element.
    /// `key` is KeyboardEvent.key; `modifiers` is a bitmask of modifier_shift/ctrl/alt/meta.
    pub fn on_key_down(&mut self, key: &str, modifiers: u32) {
        let focused = match self.focused_element {
            Some(id) => id,
            None => return,
        };
        match key {
            "Backspace" => {
                self.tree.element_backspace(focused);
            }
            "Enter" => {
                self.tree.element_append_text_content(focused, "\n");
                self.tree.push_event(Event::TextInput { target: focused, text: "\n".to_string() });
            }
            _ => {}
        }
        self.tree.push_event(Event::KeyDown { target: focused, key: key.to_string(), modifiers });
    }

    /// Toggle the cursor visibility for blinking. JS calls this from requestAnimationFrame
    /// with the current timestamp; the cursor alternates every 500 ms.
    pub fn tick_cursor(&mut self, timestamp_ms: f64) {
        if let Some(focused) = self.focused_element {
            let visible = ((timestamp_ms as u64) / 500) % 2 == 0;
            self.tree.element_set_cursor_visible(focused, visible);
        }
    }

    /// Called by JS when the user types printable text into the focused TextInput.
    pub fn on_text_input(&mut self, id: f64, text: &str) {
        let eid = element_id_from_f64(id);
        self.tree.element_append_text_content(eid, text);
        self.tree.push_event(Event::TextInput { target: eid, text: text.to_string() });
    }

    /// Called by JS when an IME composition begins.
    pub fn on_composition_start(&mut self, id: f64, text: &str) {
        let eid = element_id_from_f64(id);
        self.tree.element_set_preedit(eid, text);
        self.tree.push_event(Event::CompositionStart { target: eid, text: text.to_string() });
    }

    /// Called by JS when the IME preedit updates.
    pub fn on_composition_update(&mut self, id: f64, text: &str) {
        let eid = element_id_from_f64(id);
        self.tree.element_set_preedit(eid, text);
        self.tree.push_event(Event::CompositionUpdate { target: eid, text: text.to_string() });
    }

    /// Called by JS when IME composition is finalized.
    pub fn on_composition_end(&mut self, id: f64, text: &str) {
        let eid = element_id_from_f64(id);
        self.tree.element_set_preedit(eid, "");
        self.tree.element_append_text_content(eid, text);
        self.tree.push_event(Event::CompositionEnd { target: eid, text: text.to_string() });
    }

    pub fn element_set_text_content(&mut self, id: f64, text: &str) {
        self.tree.element_set_text_content(element_id_from_f64(id), text);
    }

    pub fn element_get_text_content(&self, id: f64) -> String {
        self.tree.element_get_text_content(element_id_from_f64(id))
    }

    pub fn poll_events(&mut self) -> Box<[f64]> {
        let events = self.tree.poll_events();
        encode_events(&events)
    }
}

// ── HTML Mode renderer (ADR-0029: browser CSS layout) ────────────────────
//
// Each Hayate Element maps to a real DOM element parented exactly like the
// element tree. Hayate CSS props are translated 1:1 to browser CSS so the
// browser engine performs layout. Taffy is not invoked — the previous
// "Taffy → absolutely-positioned div" pipeline (ADR-0016 元方式) is gone.

struct HtmlNode {
    kind: ElementKind,
    dom: Element,
    parent: Option<ElementId>,
    children: Vec<ElementId>,
    text: Option<String>,
    src: Option<String>,
}

#[wasm_bindgen]
pub struct HayateElementHtmlRenderer {
    container: HtmlElement,
    nodes: SlotMap<ElementId, HtmlNode>,
    root: Option<ElementId>,
    event_queue: Vec<Event>,
    focused_element: Option<ElementId>,
    hovered_element: Option<ElementId>,
}

#[wasm_bindgen]
impl HayateElementHtmlRenderer {
    pub fn new(container: HtmlElement) -> Result<HayateElementHtmlRenderer, JsValue> {
        let style = container.style();
        style.set_property("position", "relative")?;
        style.set_property("overflow", "hidden")?;
        Ok(Self {
            container,
            nodes: SlotMap::with_key(),
            root: None,
            event_queue: Vec::new(),
            focused_element: None,
            hovered_element: None,
        })
    }

    /// Viewport is browser-managed in HTML Mode; this is kept for API parity
    /// with the Canvas renderer and only emits a Resize event.
    pub fn set_viewport(&mut self, width: f32, height: f32) {
        self.event_queue.push(Event::Resize { width, height });
    }

    pub fn element_create(&mut self, kind: u32) -> Result<f64, JsValue> {
        let k = kind_from_u32(kind)?;
        let dom = create_dom_for_kind(&document(), k)?;
        apply_kind_baseline(&dom, k)?;

        let id = self.nodes.insert(HtmlNode {
            kind: k,
            dom: dom.clone(),
            parent: None,
            children: Vec::new(),
            text: None,
            src: None,
        });
        dom.set_attribute("data-element-id", &format!("{}", id.data().as_ffi()))?;
        if self.root.is_none() {
            self.root = Some(id);
            self.container.append_child(&dom)?;
        }
        Ok(element_id_to_f64(id))
    }

    pub fn element_set_text(&mut self, id: f64, text: &str) {
        let eid = element_id_from_f64(id);
        if let Some(n) = self.nodes.get_mut(eid) {
            n.text = Some(text.to_string());
            match n.kind {
                ElementKind::TextInput => {
                    if let Some(input) = n.dom.dyn_ref::<HtmlInputElement>() {
                        input.set_value(text);
                    }
                }
                _ => {
                    if let Some(html_el) = n.dom.dyn_ref::<HtmlElement>() {
                        html_el.set_inner_text(text);
                    }
                }
            }
        }
    }

    pub fn element_set_src(&mut self, id: f64, url: &str) {
        let eid = element_id_from_f64(id);
        if let Some(n) = self.nodes.get_mut(eid) {
            n.src = Some(url.to_string());
            if n.kind == ElementKind::Image {
                let _ = n.dom.set_attribute("src", url);
            }
        }
    }

    pub fn element_set_style(&mut self, id: f64, packed: &[f32]) -> Result<(), JsValue> {
        let props = style_packet::decode(packed)?;
        let eid = element_id_from_f64(id);
        if let Some(n) = self.nodes.get(eid) {
            if let Some(html_el) = n.dom.dyn_ref::<HtmlElement>() {
                style_packet::apply_props_to_dom(&html_el.style(), &props)?;
            }
        }
        Ok(())
    }

    /// Apply a 2D affine transform via CSS `transform: matrix(a,b,c,d,e,f)`,
    /// or clear it when an empty slice is passed.
    pub fn element_set_transform(&mut self, id: f64, matrix: &[f64]) {
        let eid = element_id_from_f64(id);
        let n = match self.nodes.get(eid) {
            Some(n) => n,
            None => return,
        };
        let html_el = match n.dom.dyn_ref::<HtmlElement>() {
            Some(e) => e,
            None => return,
        };
        let style = html_el.style();
        if matrix.len() == 6 {
            let css = format!(
                "matrix({},{},{},{},{},{})",
                matrix[0], matrix[1], matrix[2], matrix[3], matrix[4], matrix[5]
            );
            let _ = style.set_property("transform", &css);
        } else {
            let _ = style.set_property("transform", "none");
        }
    }

    pub fn element_append_child(&mut self, parent: f64, child: f64) {
        let pid = element_id_from_f64(parent);
        let cid = element_id_from_f64(child);
        if !self.nodes.contains_key(pid) || !self.nodes.contains_key(cid) {
            return;
        }
        self.detach_from_current_parent(cid);
        let (parent_dom, child_dom) = (
            self.nodes[pid].dom.clone(),
            self.nodes[cid].dom.clone(),
        );
        let _ = parent_dom.append_child(child_dom.as_ref());
        self.nodes[pid].children.push(cid);
        self.nodes[cid].parent = Some(pid);
    }

    pub fn element_insert_before(&mut self, parent: f64, child: f64, before: f64) {
        let pid = element_id_from_f64(parent);
        let cid = element_id_from_f64(child);
        let bid = element_id_from_f64(before);
        if !self.nodes.contains_key(pid)
            || !self.nodes.contains_key(cid)
            || !self.nodes.contains_key(bid)
        {
            return;
        }
        self.detach_from_current_parent(cid);
        let index = match self.nodes[pid].children.iter().position(|&c| c == bid) {
            Some(i) => i,
            None => {
                self.element_append_child(parent, child);
                return;
            }
        };
        let parent_dom = self.nodes[pid].dom.clone();
        let child_dom = self.nodes[cid].dom.clone();
        let before_dom = self.nodes[bid].dom.clone();
        let _ = parent_dom
            .unchecked_ref::<Node>()
            .insert_before(child_dom.as_ref(), Some(before_dom.as_ref()));
        self.nodes[pid].children.insert(index, cid);
        self.nodes[cid].parent = Some(pid);
    }

    pub fn element_remove(&mut self, id: f64) {
        let target = element_id_from_f64(id);
        if !self.nodes.contains_key(target) {
            return;
        }
        self.detach_from_current_parent(target);
        // DOM removeChild cascades to descendants; we only need to drop the
        // top-level DOM node from its parent (or the container if it was root).
        let top_dom = self.nodes[target].dom.clone();
        if let Some(parent_dom) = top_dom.parent_node() {
            let _ = parent_dom.remove_child(top_dom.as_ref());
        }
        // Drop the slotmap entries for the subtree.
        let mut stack = vec![target];
        while let Some(node) = stack.pop() {
            if let Some(n) = self.nodes.remove(node) {
                stack.extend(n.children.iter().copied());
            }
        }
        if self.root == Some(target) {
            self.root = None;
        }
        if self.focused_element == Some(target) {
            self.focused_element = None;
        }
        if self.hovered_element == Some(target) {
            self.hovered_element = None;
        }
    }

    pub fn element_get_text(&self, id: f64) -> String {
        self.nodes
            .get(element_id_from_f64(id))
            .and_then(|n| n.text.clone())
            .unwrap_or_default()
    }

    pub fn set_root(&mut self, id: f64) {
        let new_root = element_id_from_f64(id);
        if !self.nodes.contains_key(new_root) {
            return;
        }
        // Detach the previous root from the container (if any).
        if let Some(prev) = self.root {
            if prev != new_root {
                let prev_dom = self.nodes[prev].dom.clone();
                let _ = self.container.remove_child(prev_dom.as_ref());
            }
        }
        // Lift the new root out of any prior parent and mount it on the container.
        self.detach_from_current_parent(new_root);
        let dom = self.nodes[new_root].dom.clone();
        let _ = self.container.append_child(dom.as_ref());
        self.root = Some(new_root);
    }

    /// In HTML Mode `render` only sets the container's background colour —
    /// the browser handles incremental reflow on every prior style mutation.
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
        Ok(())
    }

    // ── Input wiring ─────────────────────────────────────────────────────
    // HTML Mode does not run Taffy, so hit-tests cannot use a layout cache.
    // JS reads `data-element-id` from `event.target` and dispatches via the
    // explicit-target methods below. The legacy positional methods are
    // retained as no-ops so callers shared with Canvas Mode keep compiling.

    pub fn on_pointer_down(&mut self, target_id: f64, x: f32, y: f32) {
        let target = element_id_from_f64(target_id);
        if !self.nodes.contains_key(target) {
            return;
        }
        self.event_queue.push(Event::Click { target, x, y });
        if self.focused_element != Some(target) {
            if let Some(prev) = self.focused_element {
                self.event_queue.push(Event::Blur(prev));
            }
            self.focused_element = Some(target);
            self.event_queue.push(Event::Focus(target));
        }
    }

    pub fn on_pointer_up(&mut self, target_id: f64, x: f32, y: f32) {
        let target = element_id_from_f64(target_id);
        if self.nodes.contains_key(target) {
            self.event_queue.push(Event::PointerUp { target, x, y });
        }
    }

    pub fn on_pointer_enter(&mut self, target_id: f64) {
        let target = element_id_from_f64(target_id);
        if !self.nodes.contains_key(target) {
            return;
        }
        if self.hovered_element != Some(target) {
            if let Some(prev) = self.hovered_element {
                self.event_queue.push(Event::PointerLeave { target: prev });
            }
            self.hovered_element = Some(target);
            self.event_queue.push(Event::PointerEnter { target });
        }
    }

    pub fn on_pointer_leave(&mut self, target_id: f64) {
        let target = element_id_from_f64(target_id);
        if self.hovered_element == Some(target) {
            self.hovered_element = None;
            self.event_queue.push(Event::PointerLeave { target });
        }
    }

    pub fn on_wheel(&mut self, target_id: f64, delta_x: f32, delta_y: f32) {
        let target = element_id_from_f64(target_id);
        if self.nodes.contains_key(target) {
            self.event_queue.push(Event::Scroll { target, delta_x, delta_y });
        }
    }

    pub fn on_resize(&mut self, width: f32, height: f32) {
        self.event_queue.push(Event::Resize { width, height });
    }

    pub fn element_set_scroll_offset(&mut self, id: f64, x: f32, y: f32) {
        let eid = element_id_from_f64(id);
        if let Some(n) = self.nodes.get(eid) {
            n.dom.set_scroll_left(x as i32);
            n.dom.set_scroll_top(y as i32);
        }
    }

    pub fn element_set_font_family(&mut self, id: f64, family: &str) {
        let eid = element_id_from_f64(id);
        if let Some(n) = self.nodes.get(eid) {
            if let Some(html_el) = n.dom.dyn_ref::<HtmlElement>() {
                let _ = html_el.style().set_property("font-family", family);
            }
        }
    }

    pub fn element_set_aria_label(&mut self, id: f64, label: &str) {
        let eid = element_id_from_f64(id);
        if let Some(n) = self.nodes.get(eid) {
            let _ = n.dom.set_attribute("aria-label", label);
        }
    }

    pub fn element_set_role(&mut self, id: f64, role: &str) {
        let eid = element_id_from_f64(id);
        if let Some(n) = self.nodes.get(eid) {
            let _ = n.dom.set_attribute("role", role);
        }
    }

    /// Register a Web Font via CSS `@font-face`. Browser renders text in HTML
    /// Mode, so font registration is delegated to the document's CSS engine.
    pub fn register_font_bytes(&mut self, family_name: &str, data: &[u8]) {
        let _ = inject_font_face(family_name, data);
    }

    pub async fn load_font_from_url(&mut self, family_name: String, url: String) -> Result<(), JsValue> {
        let bytes = fetch_bytes(&url).await?;
        let _ = inject_font_face(&family_name, &bytes);
        Ok(())
    }

    /// Clipboard paste is reported as a synthetic TextInput event targeting
    /// the focused element. Native DOM IME already commits the text itself.
    pub fn on_clipboard_paste(&mut self, text: &str) {
        if let Some(focused) = self.focused_element {
            self.event_queue.push(Event::TextInput { target: focused, text: text.to_string() });
        }
    }

    pub fn focused_element_id(&self) -> f64 {
        self.focused_element.map(element_id_to_f64).unwrap_or(0.0)
    }

    pub fn on_key_down(&mut self, key: &str, modifiers: u32) {
        let focused = match self.focused_element {
            Some(id) => id,
            None => return,
        };
        self.event_queue.push(Event::KeyDown { target: focused, key: key.to_string(), modifiers });
    }

    pub fn on_text_input(&mut self, id: f64, text: &str) {
        let eid = element_id_from_f64(id);
        if self.nodes.contains_key(eid) {
            self.event_queue.push(Event::TextInput { target: eid, text: text.to_string() });
        }
    }

    pub fn on_composition_start(&mut self, id: f64, text: &str) {
        let eid = element_id_from_f64(id);
        if self.nodes.contains_key(eid) {
            self.event_queue.push(Event::CompositionStart { target: eid, text: text.to_string() });
        }
    }

    pub fn on_composition_update(&mut self, id: f64, text: &str) {
        let eid = element_id_from_f64(id);
        if self.nodes.contains_key(eid) {
            self.event_queue.push(Event::CompositionUpdate { target: eid, text: text.to_string() });
        }
    }

    pub fn on_composition_end(&mut self, id: f64, text: &str) {
        let eid = element_id_from_f64(id);
        if self.nodes.contains_key(eid) {
            self.event_queue.push(Event::CompositionEnd { target: eid, text: text.to_string() });
        }
    }

    pub fn element_set_text_content(&mut self, id: f64, text: &str) {
        let eid = element_id_from_f64(id);
        if let Some(n) = self.nodes.get_mut(eid) {
            n.text = Some(text.to_string());
            if let Some(input) = n.dom.dyn_ref::<HtmlInputElement>() {
                input.set_value(text);
            } else if let Some(html_el) = n.dom.dyn_ref::<HtmlElement>() {
                html_el.set_inner_text(text);
            }
        }
    }

    pub fn element_get_text_content(&self, id: f64) -> String {
        let eid = element_id_from_f64(id);
        let n = match self.nodes.get(eid) {
            Some(n) => n,
            None => return String::new(),
        };
        if let Some(input) = n.dom.dyn_ref::<HtmlInputElement>() {
            return input.value();
        }
        n.text.clone().unwrap_or_default()
    }

    /// Set the image's `src` to the URL. The browser fetches and decodes it.
    pub async fn load_image(&mut self, id: f64, url: String) -> Result<(), JsValue> {
        let eid = element_id_from_f64(id);
        if let Some(n) = self.nodes.get_mut(eid) {
            if n.kind == ElementKind::Image {
                n.src = Some(url.clone());
                let _ = n.dom.set_attribute("src", &url);
            }
        }
        Ok(())
    }

    pub fn poll_events(&mut self) -> Box<[f64]> {
        let events: Vec<Event> = std::mem::take(&mut self.event_queue);
        encode_events(&events)
    }
}

impl HayateElementHtmlRenderer {
    fn detach_from_current_parent(&mut self, child: ElementId) {
        let parent = match self.nodes.get(child).and_then(|c| c.parent) {
            Some(p) => p,
            None => return,
        };
        self.nodes[parent].children.retain(|&c| c != child);
        self.nodes[child].parent = None;
    }
}

fn create_dom_for_kind(doc: &Document, kind: ElementKind) -> Result<Element, JsValue> {
    let tag = match kind {
        ElementKind::Image => "img",
        ElementKind::TextInput => "input",
        ElementKind::Button => "button",
        _ => "div",
    };
    let el = doc.create_element(tag)?;
    if kind == ElementKind::TextInput {
        let _ = el.set_attribute("type", "text");
    }
    Ok(el)
}

/// Per-kind baseline CSS — keep it minimal so user-supplied styles via
/// `element_set_style` cleanly override. Mirrors React Native Web's
/// resetStyle approach: predictable box model, no inherited surprises.
fn apply_kind_baseline(el: &Element, kind: ElementKind) -> Result<(), JsValue> {
    let html_el = match el.dyn_ref::<HtmlElement>() {
        Some(e) => e,
        None => return Ok(()),
    };
    let style = html_el.style();
    style.set_property("box-sizing", "border-box")?;
    style.set_property("position", "relative")?;
    style.set_property("margin", "0")?;
    style.set_property("padding", "0")?;
    style.set_property("border", "0 solid black")?;
    style.set_property("min-width", "0")?;
    style.set_property("min-height", "0")?;
    match kind {
        ElementKind::ScrollView => {
            style.set_property("overflow", "auto")?;
            style.set_property("display", "flex")?;
            style.set_property("flex-direction", "column")?;
        }
        ElementKind::Image => {
            style.set_property("display", "block")?;
            style.set_property("object-fit", "fill")?;
        }
        ElementKind::TextInput => {
            style.set_property("outline", "none")?;
            style.set_property("background", "transparent")?;
            style.set_property("font", "inherit")?;
            style.set_property("color", "inherit")?;
        }
        ElementKind::Button => {
            style.set_property("cursor", "pointer")?;
            style.set_property("background", "transparent")?;
            style.set_property("font", "inherit")?;
            style.set_property("color", "inherit")?;
        }
        _ => {}
    }
    Ok(())
}

/// Inject a CSS `@font-face` rule into the document so the browser can
/// render text in `font-family: <family_name>`. The font bytes are passed
/// as a data URL — adequate for the demo + development use cases that the
/// HTML Mode targets.
fn inject_font_face(family: &str, data: &[u8]) -> Result<(), JsValue> {
    use js_sys::Uint8Array;
    // Base64-encode the bytes via btoa over a binary string built from raw bytes.
    let bin: String = data.iter().map(|&b| b as char).collect();
    let window = web_sys::window().ok_or("no window")?;
    let b64 = window.btoa(&bin)?;
    let css = format!(
        "@font-face {{ font-family: '{family}'; src: url(data:font/ttf;base64,{b64}); }}"
    );
    let doc = window.document().ok_or("no document")?;
    let head = doc.head().ok_or("no head")?;
    let style_el = doc.create_element("style")?;
    style_el.set_text_content(Some(&css));
    head.append_child(style_el.as_ref())?;
    // `_` to acknowledge that Uint8Array isn't used; keeps the import optional
    // when we later switch to FontFace API.
    let _ = Uint8Array::new_with_length(0);
    Ok(())
}

/// Walk up the element tree to find the nearest ScrollView at or above `id`.
fn nearest_scroll_view(tree: &ElementTree, mut id: ElementId) -> Option<ElementId> {
    loop {
        if tree.element_kind(id) == Some(ElementKind::ScrollView) {
            return Some(id);
        }
        id = tree.element_parent(id)?;
    }
}

/// Encode an event list into a flat f64 array for JS consumption.
///
/// Format per event:
///   click:         [0,  target_ffi, x, y]
///   focus:         [1,  target_ffi]
///   blur:          [2,  target_ffi]
///   text_input:    [3,  target_ffi]
///   comp_start:    [4,  target_ffi]
///   comp_update:   [5,  target_ffi]
///   comp_end:      [6,  target_ffi]
///   scroll:        [7,  target_ffi, delta_x, delta_y]
///   resize:        [8,  width, height]
///   pointer_up:    [9,  target_ffi, x, y]
///   pointer_enter: [10, target_ffi]
///   pointer_leave: [11, target_ffi]
///   key_down:      [12, target_ffi, modifiers]
fn encode_events(events: &[Event]) -> Box<[f64]> {
    use slotmap::Key;
    let mut out: Vec<f64> = Vec::with_capacity(events.len() * 4);
    for event in events {
        match event {
            Event::Click { target, x, y } => {
                out.push(0.0);
                out.push(target.data().as_ffi() as f64);
                out.push(*x as f64);
                out.push(*y as f64);
            }
            Event::Focus(target) => {
                out.push(1.0);
                out.push(target.data().as_ffi() as f64);
            }
            Event::Blur(target) => {
                out.push(2.0);
                out.push(target.data().as_ffi() as f64);
            }
            Event::TextInput { target, .. } => {
                out.push(3.0);
                out.push(target.data().as_ffi() as f64);
            }
            Event::CompositionStart { target, .. } => {
                out.push(4.0);
                out.push(target.data().as_ffi() as f64);
            }
            Event::CompositionUpdate { target, .. } => {
                out.push(5.0);
                out.push(target.data().as_ffi() as f64);
            }
            Event::CompositionEnd { target, .. } => {
                out.push(6.0);
                out.push(target.data().as_ffi() as f64);
            }
            Event::Scroll { target, delta_x, delta_y } => {
                out.push(7.0);
                out.push(target.data().as_ffi() as f64);
                out.push(*delta_x as f64);
                out.push(*delta_y as f64);
            }
            Event::Resize { width, height } => {
                out.push(8.0);
                out.push(*width as f64);
                out.push(*height as f64);
            }
            Event::PointerUp { target, x, y } => {
                out.push(9.0);
                out.push(target.data().as_ffi() as f64);
                out.push(*x as f64);
                out.push(*y as f64);
            }
            Event::PointerEnter { target } => {
                out.push(10.0);
                out.push(target.data().as_ffi() as f64);
            }
            Event::PointerLeave { target } => {
                out.push(11.0);
                out.push(target.data().as_ffi() as f64);
            }
            Event::KeyDown { target, modifiers, .. } => {
                out.push(12.0);
                out.push(target.data().as_ffi() as f64);
                out.push(*modifiers as f64);
            }
        }
    }
    out.into_boxed_slice()
}

/// Fetch raw bytes from a URL.
async fn fetch_bytes(url: &str) -> Result<Vec<u8>, JsValue> {
    use js_sys::{ArrayBuffer, Uint8Array};
    let window = web_sys::window().ok_or("no window")?;
    let resp: web_sys::Response =
        JsFuture::from(window.fetch_with_str(url)).await?.dyn_into()?;
    let buf: ArrayBuffer = JsFuture::from(resp.array_buffer()?).await?.dyn_into()?;
    Ok(Uint8Array::new(&buf).to_vec())
}

/// Fetch a URL and decode it as RGBA8, supporting PNG / JPEG / WebP.
async fn fetch_image(url: &str) -> Result<ImageData, JsValue> {
    use js_sys::{ArrayBuffer, Uint8Array};

    let window = web_sys::window().ok_or("no window")?;
    let resp: web_sys::Response =
        JsFuture::from(window.fetch_with_str(url)).await?.dyn_into()?;
    let buf: ArrayBuffer = JsFuture::from(resp.array_buffer()?).await?.dyn_into()?;
    let bytes = Uint8Array::new(&buf).to_vec();

    let img = image::load_from_memory(&bytes)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let rgba = img.into_rgba8();
    let width = rgba.width();
    let height = rgba.height();
    let raw = rgba.into_raw();

    let blob = Blob::new(Arc::new(raw));
    Ok(ImageData {
        data: blob,
        format: ImageFormat::Rgba8,
        alpha_type: ImageAlphaType::Alpha,
        width,
        height,
    })
}
