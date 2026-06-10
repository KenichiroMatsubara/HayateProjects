//! Canvas Mode renderer (`HayateElementRenderer`). See ADR-0077.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use hayate_core::{
    DocumentEventKind, ElementId, ElementTree, Event, RenderImage, RenderImageAlphaType,
    RenderImageFormat, StylePropKind,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::HtmlCanvasElement;

use crate::apply_mutations_dispatch::{
    apply_mutations_batch, unset_kind_from_u32, ApplyMutationsHost,
};
use crate::backend::{CanvasBackend, SelectedBackend};
use crate::builtin_fonts::builtin_font_url;
use crate::generated::encode_deliveries;
use crate::ime_bridge::{sync_ime_character_bounds, WebImeBridge};
use crate::style_packet;

use crate::shared::{element_id_from_f64, element_id_to_f64, fetch_bytes, kind_from_u32};

/// Fonts fetched asynchronously by the adapter; drained into the tree on the
/// next `poll_events()` call (single-threaded WASM — Rc<RefCell> is safe).
type FontQueue = Rc<RefCell<Vec<(String, Vec<u8>)>>>;

// ── Canvas Mode renderer ─────────────────────────────────────────────────

#[wasm_bindgen]
pub struct HayateElementRenderer {
    backend: SelectedBackend,
    tree: ElementTree,
    /// wgpu surface clear colour. Decoupled from `render(timestamp_ms)` because
    /// the WIT `render` signature no longer carries it (ADR-0032 keeps render
    /// timestamp-only); call `set_background_color` separately.
    background: [f32; 4],
    /// Fonts fetched by spawned futures; applied to the tree on next poll_events.
    font_queue: FontQueue,
    /// IME candidate-window bounds synced each render (ADR-0069).
    ime: WebImeBridge,
}

#[wasm_bindgen]
impl HayateElementRenderer {
    pub async fn init(canvas: HtmlCanvasElement) -> Result<HayateElementRenderer, JsValue> {
        let width = canvas.width() as f32;
        let height = canvas.height() as f32;
        let backend = SelectedBackend::init(canvas).await?;
        let mut tree = ElementTree::new();
        tree.set_viewport(width, height);
        Ok(Self {
            backend,
            tree,
            background: [0.0, 0.0, 0.0, 1.0],
            font_queue: Rc::new(RefCell::new(Vec::new())),
            ime: WebImeBridge::default(),
        })
    }

    /// Set the wgpu surface clear colour used by every subsequent `render()`.
    /// Not part of the WIT — it complements the timestamp-only `render` from
    /// ADR-0032 so demos can still drive their colour pickers without
    /// re-issuing the colour each frame.
    pub fn set_background_color(&mut self, r: f64, g: f64, b: f64) {
        self.background = [r as f32, g as f32, b as f32, 1.0];
    }

    pub fn set_viewport(&mut self, width: f32, height: f32) {
        self.tree.set_viewport(width, height);
    }

    /// Registers an element with a caller-supplied ID. JS generates the ID
    /// via a monotonic counter, eliminating the WASM round-trip for ID allocation.
    pub fn element_create(&mut self, id: f64, kind: u32) -> Result<(), JsValue> {
        let k = kind_from_u32(kind)?;
        self.tree.element_create(id as u64, k);
        Ok(())
    }

    pub fn element_set_text(&mut self, id: f64, text: &str) {
        self.tree.element_set_text(element_id_from_f64(id), text);
    }

    pub fn element_set_src(&mut self, id: f64, url: &str) {
        self.tree.element_set_src(element_id_from_f64(id), url);
    }

    pub fn element_set_disabled(&mut self, id: f64, disabled: bool) {
        self.tree
            .element_set_disabled(element_id_from_f64(id), disabled);
    }

    pub fn element_set_style(&mut self, id: f64, packed: &[f32]) -> Result<(), JsValue> {
        let props = style_packet::decode(packed)?;
        self.tree.element_set_style(element_id_from_f64(id), &props);
        Ok(())
    }

    /// Hayate CSS pseudo-class block (`:hover` / `:active` / `:focus`).
    pub fn element_set_pseudo_style(
        &mut self,
        id: f64,
        state: u32,
        packed: &[f32],
    ) -> Result<(), JsValue> {
        let pseudo = hayate_core::PseudoState::from_u32(state)
            .ok_or_else(|| JsValue::from_str(&format!("unknown pseudo-state {state}")))?;
        let props = style_packet::decode(packed)?;
        self.tree
            .element_set_pseudo_style(element_id_from_f64(id), pseudo, &props);
        Ok(())
    }

    /// Apply a 2D affine transform on top of layout. Arguments map to the WIT
    /// `affine` record fields (column-major: xx,yx,xy,yy,dx,dy). Pass identity
    /// (1,0,0,1,0,0) to neutralise an earlier transform.
    pub fn element_set_transform(
        &mut self,
        id: f64,
        xx: f64,
        yx: f64,
        xy: f64,
        yy: f64,
        dx: f64,
        dy: f64,
    ) {
        self.tree
            .element_set_transform(element_id_from_f64(id), Some([xx, yx, xy, yy, dx, dy]));
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
        let eid = element_id_from_f64(id);
        self.remove_subtree(eid);
    }

    fn remove_subtree(&mut self, id: ElementId) {
        self.tree.element_remove(id);
    }

    /// Returns the element's current text. Canvas Mode applies `element_set_text`
    /// eagerly (ADR-0037), so this reflects the latest setter call immediately.
    pub fn element_get_text(&self, id: f64) -> String {
        self.tree.element_get_text(element_id_from_f64(id))
    }

    /// Return the element's absolute bounds [x, y, width, height] from the
    /// most recent layout pass. Zeroed when the id is unknown or the element
    /// has not been laid out yet. WIT-aligned (`element-get-bounds`).
    pub fn element_get_bounds(&self, id: f64) -> Box<[f32]> {
        let eid = element_id_from_f64(id);
        let (x, y, w, h) = self
            .tree
            .element_layout_rect(eid)
            .unwrap_or((0.0, 0.0, 0.0, 0.0));
        vec![x, y, w, h].into_boxed_slice()
    }

    pub fn set_root(&mut self, id: f64) {
        self.tree.set_root(element_id_from_f64(id));
    }

    /// Advance cursor blink, run layout, and present. `timestamp_ms` should be a
    /// monotonic clock (e.g. `performance.now()`). Mutations are applied eagerly
    /// by the `element_*` setters (ADR-0037), so `render` only drives layout.
    pub fn render(&mut self, timestamp_ms: f64) -> Result<(), JsValue> {
        // フェッチ完了フォントを layout より前に登録することで、同フレーム内で
        // fonts_dirty → compute_layout → 正しいグリフ、が成立する。
        // （poll_events より先に render が呼ばれる raf ループでも豆腐にならない）
        let loaded: Vec<(String, Vec<u8>)> = self.font_queue.borrow_mut().drain(..).collect();
        for (family, bytes) in loaded {
            self.tree.register_font(&family, bytes);
        }
        let sg = self.tree.render(timestamp_ms);
        let present = self.backend.render_scene(sg, self.background);
        if let Some(focused) = self.tree.focused_element() {
            sync_ime_character_bounds(&self.tree, focused, &mut self.ime);
        }
        present
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
        self.tree.on_pointer_down(x, y);
    }

    pub fn on_pointer_up(&mut self, x: f32, y: f32) {
        self.tree.on_pointer_up(x, y);
    }

    pub fn on_pointer_move(&mut self, x: f32, y: f32) {
        let _ = self.tree.on_pointer_move(x, y);
    }

    pub fn on_wheel(&mut self, x: f32, y: f32, delta_x: f32, delta_y: f32) {
        if let Some(target) = self.tree.hit_test(x, y) {
            self.tree.apply_wheel_delta(target, delta_x, delta_y);
            self.tree.on_wheel(target, delta_x, delta_y);
        }
    }

    pub fn on_resize(&mut self, width: f32, height: f32) {
        self.tree.set_viewport(width, height);
        self.backend.resize(width as u32, height as u32);
        self.tree.on_resize(width, height);
    }

    pub fn register_listener(&mut self, element_id: f64, event_kind: u32) -> Result<f64, JsValue> {
        let kind = DocumentEventKind::from_u32(event_kind)
            .ok_or_else(|| JsValue::from_str(&format!("unknown event kind {event_kind}")))?;
        let id = self
            .tree
            .register_listener(element_id_from_f64(element_id), kind);
        Ok(id.to_u64() as f64)
    }

    /// Returns element ids in `id` and its descendants. Query Hayate before remove.
    pub fn element_subtree_ids(&self, id: f64) -> Vec<f64> {
        self.tree
            .subtree_element_ids(element_id_from_f64(id))
            .into_iter()
            .map(element_id_to_f64)
            .collect()
    }

    pub fn element_set_scroll_offset(&mut self, id: f64, x: f32, y: f32) {
        self.tree
            .element_set_scroll_offset(element_id_from_f64(id), x, y);
    }

    pub fn element_set_font_family(&mut self, id: f64, family: &str) {
        self.tree
            .element_set_font_family(element_id_from_f64(id), family);
    }

    /// Unset one or more inheritable text-style properties on `id`, reverting
    /// them to inherit from the parent (ADR-0047).
    /// `kinds` is a packed u32 array: 0 = Color, 1 = FontSize, 2 = FontFamily.
    pub fn element_unset_style(&mut self, id: f64, kinds: &[u32]) -> Result<(), JsValue> {
        let parsed: Result<Vec<StylePropKind>, JsValue> = kinds
            .iter()
            .map(|&kind| unset_kind_from_u32(kind))
            .collect();
        self.tree
            .element_unset_style(element_id_from_f64(id), &parsed?);
        Ok(())
    }

    pub fn element_set_aria_label(&mut self, id: f64, label: &str) {
        self.tree
            .element_set_aria_label(element_id_from_f64(id), label);
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
    pub async fn load_font_from_url(
        &mut self,
        family_name: String,
        url: String,
    ) -> Result<(), JsValue> {
        let bytes = fetch_bytes(&url).await?;
        self.tree.register_font(&family_name, bytes);
        Ok(())
    }

    /// Preload fonts declared in the app's `hayate.config.json`.
    ///
    /// Accepts a JS array of `{ family: string, url: string }` objects.
    /// Fetches each font sequentially and blocks until all are registered,
    /// so the first `render()` frame uses the correct fonts (no FOUT).
    ///
    /// # Example (JS)
    /// ```js
    /// const cfg = await fetch('./hayate.config.json').then(r => r.json());
    /// await renderer.configure_fonts(cfg.fonts);
    /// ```
    pub async fn configure_fonts(&mut self, fonts: JsValue) -> Result<(), JsValue> {
        use js_sys::{Array, Reflect};
        let arr = Array::from(&fonts);
        for i in 0..arr.length() {
            let item = arr.get(i);
            let family = Reflect::get(&item, &JsValue::from_str("family"))?
                .as_string()
                .ok_or_else(|| JsValue::from_str("configure_fonts: missing 'family'"))?;
            let url = Reflect::get(&item, &JsValue::from_str("url"))?
                .as_string()
                .ok_or_else(|| JsValue::from_str("configure_fonts: missing 'url'"))?;
            let bytes = fetch_bytes(&url).await?;
            self.tree.register_font(&family, bytes);
        }
        Ok(())
    }

    /// Load a font using the family name embedded in the font file. Backs the
    /// WIT `element-load-font` export.
    pub fn element_load_font(&mut self, data: &[u8]) {
        self.tree.register_font_bytes(data.to_vec());
    }

    /// Deliver pasted text to a specific TextInput element. WIT-aligned
    /// (`element-paste`); replaces the implicit-focus `on_clipboard_paste`.
    pub fn element_paste(&mut self, id: f64, text: &str) {
        self.tree.element_paste(element_id_from_f64(id), text);
    }

    /// Return the focused element's id (as f64), or 0.0 if nothing is focused.
    /// JS can use this with `element_get_text_content` to implement copy/cut.
    pub fn focused_element_id(&self) -> f64 {
        self.tree
            .focused_element()
            .map(element_id_to_f64)
            .unwrap_or(0.0)
    }

    /// Handle a key press on the focused element (edit semantics in core — ADR-0069).
    pub fn on_key_down(&mut self, key: &str, modifiers: u32) {
        self.tree.on_key_down(key, modifiers);
    }

    /// Called by JS when the user types printable text into the focused TextInput.
    pub fn on_text_input(&mut self, id: f64, text: &str) {
        self.tree.on_text_input(element_id_from_f64(id), text);
    }

    /// Called by JS when an IME composition begins.
    pub fn on_composition_start(&mut self, id: f64, text: &str) {
        self.tree
            .on_composition_start(element_id_from_f64(id), text);
    }

    /// Called by JS when the IME preedit updates.
    pub fn on_composition_update(&mut self, id: f64, text: &str) {
        self.tree
            .on_composition_update(element_id_from_f64(id), text);
    }

    /// Called by JS when IME composition is finalized.
    pub fn on_composition_end(&mut self, id: f64, text: &str) {
        self.tree.on_composition_end(element_id_from_f64(id), text);
    }

    /// Cursor character bounds for IME (ADR-0069). `[x, y, width, height]` in layout space.
    pub fn element_character_bounds(&self, id: f64) -> Box<[f32]> {
        let eid = element_id_from_f64(id);
        match self.tree.element_character_bounds(eid) {
            Some(b) => vec![b.x, b.y, b.width, b.height].into_boxed_slice(),
            None => vec![0.0, 0.0, 0.0, 0.0].into_boxed_slice(),
        }
    }

    /// Last IME character bounds synced during the most recent `render()`.
    pub fn ime_character_bounds(&self) -> Box<[f32]> {
        let b = self.ime.last_bounds();
        vec![b.x, b.y, b.width, b.height].into_boxed_slice()
    }

    pub fn element_set_text_content(&mut self, id: f64, text: &str) {
        self.tree
            .element_set_text_content(element_id_from_f64(id), text);
    }

    /// Batch apply: invoked once per frame by Tsubame Canvas Mode (ADR-0052).
    /// `ops` is a flat Float64Array of fixed-length records; `styles` is the
    /// style_packet Float32Array referenced by OP_SET_STYLE records; `texts` is
    /// the string table referenced by OP_SET_TEXT records.
    pub fn apply_mutations(
        &mut self,
        ops: &[f64],
        styles: &[f32],
        texts: js_sys::Array,
    ) -> Result<(), JsValue> {
        apply_mutations_batch(self, ops, styles, &texts)
    }

    /// Returns the editable text content from the live tree.
    pub fn element_get_text_content(&self, id: f64) -> String {
        self.tree.element_get_text_content(element_id_from_f64(id))
    }

    /// ADR-0053: delivery rows `[listener_id, kind, ...fields]`.
    /// `fetch_font` is consumed here and never delivered to the host.
    pub fn poll_events(&mut self) -> js_sys::Array {
        for event in self.tree.poll_events() {
            if let Event::FetchFont { family } = event {
                if let Some(url) = builtin_font_url(&family) {
                    let queue = self.font_queue.clone();
                    let url = url.to_string();
                    wasm_bindgen_futures::spawn_local(async move {
                        match fetch_bytes(&url).await {
                            Ok(bytes) => queue.borrow_mut().push((family, bytes)),
                            Err(e) => web_sys::console::warn_1(&e),
                        }
                    });
                } else {
                    web_sys::console::warn_1(&JsValue::from_str(&format!(
                        "FetchFont: no URL for \"{family}\""
                    )));
                }
            }
        }
        encode_deliveries(&self.tree.poll_deliveries())
    }

    /// JSON-encoded AccessKit `TreeUpdate` (ADR-0041). Returns null before layout.
    pub fn poll_accessibility(&self) -> JsValue {
        match self.tree.accessibility_update() {
            Some(update) => match serde_json::to_string(&update) {
                Ok(json) => JsValue::from_str(&json),
                Err(_) => JsValue::NULL,
            },
            None => JsValue::NULL,
        }
    }
}

impl ApplyMutationsHost for HayateElementRenderer {
    fn tree_mut(&mut self) -> &mut ElementTree {
        &mut self.tree
    }

    fn remove_subtree(&mut self, id: ElementId) {
        self.tree.element_remove(id);
    }

    fn apply_focus(&mut self, id: ElementId) {
        self.tree.on_focus(id);
    }

    fn apply_blur(&mut self, id: ElementId) {
        self.tree.on_blur(id);
    }
}

/// Fetch a URL and decode it as RGBA8, supporting PNG / JPEG / WebP.
async fn fetch_image(url: &str) -> Result<RenderImage, JsValue> {
    use js_sys::{ArrayBuffer, Uint8Array};

    let window = web_sys::window().ok_or("no window")?;
    let resp: web_sys::Response = JsFuture::from(window.fetch_with_str(url))
        .await?
        .dyn_into()?;
    let buf: ArrayBuffer = JsFuture::from(resp.array_buffer()?).await?.dyn_into()?;
    let bytes = Uint8Array::new(&buf).to_vec();

    let img = image::load_from_memory(&bytes).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let rgba = img.into_rgba8();
    let width = rgba.width();
    let height = rgba.height();
    let raw = rgba.into_raw();

    Ok(RenderImage {
        data: Arc::from(raw.into_boxed_slice()),
        format: RenderImageFormat::Rgba8,
        alpha_type: RenderImageAlphaType::Alpha,
        width,
        height,
    })
}
