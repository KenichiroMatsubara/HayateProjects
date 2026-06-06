use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use hayate_core::{
    DocumentEventKind, ElementId, ElementKind, ElementTree, Event, RenderImage, RenderImageAlphaType,
    RenderImageFormat, StyleProp, StylePropKind,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Document, Element, HtmlCanvasElement, HtmlElement, HtmlInputElement, Node};

/// Fonts fetched asynchronously by the adapter; drained into the tree on the
/// next `poll_events()` call (single-threaded WASM — Rc<RefCell> is safe).
type FontQueue = Rc<RefCell<Vec<(String, Vec<u8>)>>>;

/// Built-in family-name → CDN URL table for fonts the web adapter fetches
/// automatically (ADR-0043). Named fonts are fetched proactively when set via
/// font-family; script-specific fonts are fetched on .notdef detection.
///
/// All URLs point to TTF files from google/fonts via jsDelivr.
/// fontique/skrifa does NOT support WOFF2 — TTF/OTF only.
/// Variable font axes in filenames are URL-encoded: `[` → `%5B`, `]` → `%5D`, `,` → `%2C`.
/// Static (non-variable) fonts are noted where applicable.
fn builtin_font_url(family: &str) -> Option<&'static str> {
    match family {
        // ── CJK ──────────────────────────────────────────────────────────
        "Noto Sans JP" => Some(
            "https://cdn.jsdelivr.net/gh/google/fonts@main/ofl/notosansjp/NotoSansJP%5Bwght%5D.ttf"
        ),
        "Noto Sans KR" => Some(
            "https://cdn.jsdelivr.net/gh/google/fonts@main/ofl/notosanskr/NotoSansKR%5Bwght%5D.ttf"
        ),
        "Noto Sans SC" => Some(
            "https://cdn.jsdelivr.net/gh/google/fonts@main/ofl/notosanssc/NotoSansSC%5Bwght%5D.ttf"
        ),
        "Noto Sans TC" => Some(
            "https://cdn.jsdelivr.net/gh/google/fonts@main/ofl/notosanstc/NotoSansTC%5Bwght%5D.ttf"
        ),
        // ── Arabic ───────────────────────────────────────────────────────
        "Noto Sans Arabic" => Some(
            "https://cdn.jsdelivr.net/gh/google/fonts@main/ofl/notosansarabic/NotoSansArabic%5Bwdth%2Cwght%5D.ttf"
        ),
        // ── Thai ─────────────────────────────────────────────────────────
        "Noto Sans Thai" => Some(
            "https://cdn.jsdelivr.net/gh/google/fonts@main/ofl/notosansthai/NotoSansThai%5Bwdth%2Cwght%5D.ttf"
        ),
        // ── Devanagari ───────────────────────────────────────────────────
        "Noto Sans Devanagari" => Some(
            "https://cdn.jsdelivr.net/gh/google/fonts@main/ofl/notosansdevanagari/NotoSansDevanagari%5Bwdth%2Cwght%5D.ttf"
        ),
        // ── Hebrew ───────────────────────────────────────────────────────
        "Noto Sans Hebrew" => Some(
            "https://cdn.jsdelivr.net/gh/google/fonts@main/ofl/notosanshebrew/NotoSansHebrew%5Bwdth%2Cwght%5D.ttf"
        ),
        // ── Generic family targets (resolved from CSS keywords in text.rs) ─
        "Noto Serif" => Some(
            "https://cdn.jsdelivr.net/gh/google/fonts@main/ofl/notoserif/NotoSerif%5Bwdth%2Cwght%5D.ttf"
        ),
        "Noto Sans Mono" => Some(
            "https://cdn.jsdelivr.net/gh/google/fonts@main/ofl/notosansmono/NotoSansMono%5Bwdth%2Cwght%5D.ttf"
        ),
        // ── Popular sans-serif ────────────────────────────────────────────
        "Inter" => Some(
            "https://cdn.jsdelivr.net/gh/google/fonts@main/ofl/inter/Inter%5Bslnt%2Cwght%5D.ttf"
        ),
        "Roboto" => Some(
            "https://cdn.jsdelivr.net/gh/google/fonts@main/apache/roboto/Roboto%5Bwdth%2Cwght%5D.ttf"
        ),
        "Open Sans" => Some(
            "https://cdn.jsdelivr.net/gh/google/fonts@main/ofl/opensans/OpenSans%5Bwdth%2Cwght%5D.ttf"
        ),
        "Lato" => Some(
            // static — no variable version in google/fonts
            "https://cdn.jsdelivr.net/gh/google/fonts@main/ofl/lato/Lato-Regular.ttf"
        ),
        "Poppins" => Some(
            // static — no variable version in google/fonts
            "https://cdn.jsdelivr.net/gh/google/fonts@main/ofl/poppins/Poppins-Regular.ttf"
        ),
        "Montserrat" => Some(
            "https://cdn.jsdelivr.net/gh/google/fonts@main/ofl/montserrat/Montserrat%5Bwght%5D.ttf"
        ),
        "Raleway" => Some(
            "https://cdn.jsdelivr.net/gh/google/fonts@main/ofl/raleway/Raleway%5Bwght%5D.ttf"
        ),
        "Nunito" => Some(
            "https://cdn.jsdelivr.net/gh/google/fonts@main/ofl/nunito/Nunito%5Bwght%5D.ttf"
        ),
        "Oswald" => Some(
            "https://cdn.jsdelivr.net/gh/google/fonts@main/ofl/oswald/Oswald%5Bwght%5D.ttf"
        ),
        "Source Sans 3" => Some(
            "https://cdn.jsdelivr.net/gh/google/fonts@main/ofl/sourcesans3/SourceSans3%5Bwght%5D.ttf"
        ),
        // ── Popular serif ─────────────────────────────────────────────────
        "Playfair Display" => Some(
            "https://cdn.jsdelivr.net/gh/google/fonts@main/ofl/playfairdisplay/PlayfairDisplay%5Bwght%5D.ttf"
        ),
        "Merriweather" => Some(
            // static — no variable version in google/fonts
            "https://cdn.jsdelivr.net/gh/google/fonts@main/ofl/merriweather/Merriweather-Regular.ttf"
        ),
        "Lora" => Some(
            "https://cdn.jsdelivr.net/gh/google/fonts@main/ofl/lora/Lora%5Bwght%5D.ttf"
        ),
        // ── Popular monospace (code) ──────────────────────────────────────
        "JetBrains Mono" => Some(
            "https://cdn.jsdelivr.net/gh/google/fonts@main/ofl/jetbrainsmono/JetBrainsMono%5Bwght%5D.ttf"
        ),
        "Fira Code" => Some(
            "https://cdn.jsdelivr.net/gh/google/fonts@main/ofl/firacode/FiraCode%5Bwght%5D.ttf"
        ),
        "Source Code Pro" => Some(
            "https://cdn.jsdelivr.net/gh/google/fonts@main/ofl/sourcecodepro/SourceCodePro%5Bital%2Cwght%5D.ttf"
        ),
        _ => None,
    }
}

use crate::apply_mutations_dispatch::{apply_mutations_batch, unset_kind_from_u32, ApplyMutationsHost};
use crate::backend::{CanvasBackend, SelectedBackend};
use crate::generated::{encode_deliveries, encode_events};
use crate::renderer_event_state::RendererEventState;
use crate::style_packet;

// ── Deferred command queue (ADR-0030, HTML Mode only per ADR-0037) ────────
//
// In HTML Mode every JS-facing `element_*` mutator pushes a `Command` onto a
// per-renderer queue and returns immediately. `render()` is the sole flush
// boundary that drains the queue and applies the commands, batching DOM
// mutations so the browser reflows once per frame.
//
// Canvas Mode no longer queues (ADR-0037): Tsubame batches a frame's mutations
// on the JS side and hands them over in one `apply_mutations` call, so the
// `HayateElementRenderer` setters apply to the `ElementTree` eagerly.

enum Command {
    SetText {
        id: ElementId,
        text: String,
    },
    SetSrc {
        id: ElementId,
        url: String,
    },
    SetStyle {
        id: ElementId,
        props: Vec<StyleProp>,
    },
    UnsetStyle {
        id: ElementId,
        kinds: Vec<u32>,
    },
    SetTransform {
        id: ElementId,
        matrix: Option<[f64; 6]>,
    },
    SetScrollOffset {
        id: ElementId,
        x: f32,
        y: f32,
    },
    SetFontFamily {
        id: ElementId,
        family: String,
    },
    SetAriaLabel {
        id: ElementId,
        label: String,
    },
    SetRole {
        id: ElementId,
        role: String,
    },
    SetTextContent {
        id: ElementId,
        text: String,
    },
    AppendChild {
        parent: ElementId,
        child: ElementId,
    },
    InsertBefore {
        parent: ElementId,
        child: ElementId,
        before: ElementId,
    },
    Remove {
        id: ElementId,
    },
    SetRoot {
        id: ElementId,
    },
    /// HTML Mode only: materialise the DOM element for an already-allocated
    /// slot. Canvas Mode allocates the tree entry eagerly inside
    /// `element_create` and does not emit this command.
    HtmlCreate {
        id: ElementId,
        kind: ElementKind,
    },
}

fn document() -> Document {
    web_sys::window().unwrap().document().unwrap()
}

fn element_id_from_f64(raw: f64) -> ElementId {
    ElementId::from_u64(raw as u64)
}

fn element_id_to_f64(id: ElementId) -> f64 {
    id.to_u64() as f64
}

fn kind_from_u32(v: u32) -> Result<ElementKind, JsValue> {
    ElementKind::from_u32(v).ok_or_else(|| JsValue::from_str(&format!("unknown element kind {v}")))
}

// ── Style tag constants (exposed to JS) ──────────────────────────────────

#[wasm_bindgen]
pub fn style_tag_z_index() -> u32 {
    crate::generated::TAG_Z_INDEX
}
#[wasm_bindgen]
pub fn style_tag_font_family() -> u32 {
    crate::generated::TAG_FONT_FAMILY
}

// ── Canvas Mode renderer ─────────────────────────────────────────────────

#[wasm_bindgen]
pub struct HayateElementRenderer {
    backend: SelectedBackend,
    tree: ElementTree,
    events: RendererEventState,
    /// wgpu surface clear colour. Decoupled from `render(timestamp_ms)` because
    /// the WIT `render` signature no longer carries it (ADR-0032 keeps render
    /// timestamp-only); call `set_background_color` separately.
    background: [f32; 4],
    /// Fonts fetched by spawned futures; applied to the tree on next poll_events.
    font_queue: FontQueue,
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
            events: RendererEventState::new(),
            background: [0.0, 0.0, 0.0, 1.0],
            font_queue: Rc::new(RefCell::new(Vec::new())),
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

    pub fn element_set_style(&mut self, id: f64, packed: &[f32]) -> Result<(), JsValue> {
        let props = style_packet::decode(packed)?;
        self.tree.element_set_style(element_id_from_f64(id), &props);
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

    /// Shared removal path: clears dangling hovered/active pointers for the
    /// entire subtree, then delegates to ElementTree (which clears focused_element).
    fn remove_subtree(&mut self, id: ElementId) {
        self.events.on_subtree_remove(|c| is_in_subtree(&self.tree, c, id));
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
        self.backend.render_scene(sg, self.background)
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
        let old_focus = self.events.focused_element;
        self.events
            .pointer_down(Some(&mut self.tree), hit, x, y);
        let new_focus = self.events.focused_element;
        if old_focus != new_focus {
            if let Some(p) = old_focus {
                self.tree.element_blur(p);
            }
            if let Some(n) = new_focus {
                self.tree.element_focus(n);
            }
        }
    }

    pub fn on_pointer_up(&mut self, x: f32, y: f32) {
        // `active-end` reports the element that received `active-start`, even if
        // the pointer drifted off it before release (ADR-0031: drag-then-release
        // is one active session). The release coordinate has no field on the
        // event variant — callers that need it should track PointerMove.
        let fallback = self.tree.hit_test(x, y);
        self.events.pointer_up(Some(&mut self.tree), fallback);
    }

    pub fn on_pointer_move(&mut self, x: f32, y: f32) {
        if !self.tree.has_layout() {
            return;
        }
        // Per ADR-0031 `pointer-move` is a target-less coordinate stream; emit
        // alongside any hover state changes so dragging code can track motion.
        // 1 px throttle (ADR-0019) is enforced inside pointer_move_to.
        let hit = self.tree.hit_test(x, y);
        self.events
            .pointer_move_to(Some(&mut self.tree), hit, x, y);
    }

    pub fn on_wheel(&mut self, x: f32, y: f32, delta_x: f32, delta_y: f32) {
        if let Some(target) = self.tree.hit_test(x, y) {
            self.tree.apply_wheel_delta(target, delta_x, delta_y);
            self.events
                .wheel(Some(&mut self.tree), target, delta_x, delta_y);
        }
    }

    pub fn on_resize(&mut self, width: f32, height: f32) {
        self.tree.set_viewport(width, height);
        self.backend.resize(width as u32, height as u32);
        self.events
            .resize(Some(&mut self.tree), width, height);
    }

    pub fn register_listener(&mut self, element_id: f64, event_kind: u32) -> Result<f64, JsValue> {
        let kind = DocumentEventKind::from_u32(event_kind).ok_or_else(|| {
            JsValue::from_str(&format!("unknown event kind {event_kind}"))
        })?;
        let id = self
            .tree
            .register_listener(element_id_from_f64(element_id), kind);
        Ok(id.to_u64() as f64)
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
        self.events
            .focused_element
            .map(element_id_to_f64)
            .unwrap_or(0.0)
    }

    /// Handle a key press on the focused element.
    /// `key` is KeyboardEvent.key; `modifiers` is a bitmask of modifier_shift/ctrl/alt/meta.
    pub fn on_key_down(&mut self, key: &str, modifiers: u32) {
        let focused = match self.events.focused_element {
            Some(id) => id,
            None => return,
        };
        match key {
            "Backspace" => {
                self.tree.element_backspace(focused);
            }
            "Enter" => {
                self.tree.element_append_text_content(focused, "\n");
                self.events
                    .text_input(Some(&mut self.tree), focused, "\n");
            }
            _ => {}
        }
        self.events
            .key_down(Some(&mut self.tree), key, modifiers);
    }

    /// Called by JS when the user types printable text into the focused TextInput.
    pub fn on_text_input(&mut self, id: f64, text: &str) {
        let eid = element_id_from_f64(id);
        self.tree.element_append_text_content(eid, text);
        self.events
            .text_input(Some(&mut self.tree), eid, text);
    }

    /// Called by JS when an IME composition begins.
    pub fn on_composition_start(&mut self, id: f64, text: &str) {
        let eid = element_id_from_f64(id);
        self.tree.element_set_preedit(eid, text);
        self.events
            .composition_start(Some(&mut self.tree), eid, text);
    }

    /// Called by JS when the IME preedit updates.
    pub fn on_composition_update(&mut self, id: f64, text: &str) {
        let eid = element_id_from_f64(id);
        self.tree.element_set_preedit(eid, text);
        self.events
            .composition_update(Some(&mut self.tree), eid, text);
    }

    /// Called by JS when IME composition is finalized.
    pub fn on_composition_end(&mut self, id: f64, text: &str) {
        let eid = element_id_from_f64(id);
        self.tree.element_set_preedit(eid, "");
        self.tree.element_append_text_content(eid, text);
        self.events
            .composition_end(Some(&mut self.tree), eid, text);
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
}

impl ApplyMutationsHost for HayateElementRenderer {
    fn tree_mut(&mut self) -> &mut ElementTree {
        &mut self.tree
    }

    fn events_mut(&mut self) -> &mut RendererEventState {
        &mut self.events
    }

    fn remove_subtree(&mut self, id: ElementId) {
        self.events
            .on_subtree_remove(|c| is_in_subtree(&self.tree, c, id));
        self.tree.element_remove(id);
    }

    fn apply_focus(&mut self, id: ElementId) {
        let old = self.events.focused_element;
        self.events.focus(Some(&mut self.tree), id);
        if old != Some(id) {
            if let Some(prev) = old {
                self.tree.element_blur(prev);
            }
            self.tree.element_focus(id);
        }
    }

    fn apply_blur(&mut self, id: ElementId) {
        self.events.blur(Some(&mut self.tree), id);
        self.tree.element_blur(id);
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
    /// `Some` once the deferred `HtmlCreate` has been flushed in `render()`.
    /// Operations queued before the first flush observe the slotmap entry but
    /// no DOM element yet (ADR-0030).
    dom: Option<Element>,
    parent: Option<ElementId>,
    children: Vec<ElementId>,
    text: Option<String>,
    src: Option<String>,
}

#[wasm_bindgen]
pub struct HayateElementHtmlRenderer {
    container: HtmlElement,
    nodes: HashMap<ElementId, HtmlNode>,
    root: Option<ElementId>,
    events: RendererEventState,
    /// Container CSS background colour. HTML Mode delegates rendering to the
    /// browser; `set_background_color` stores it and `render(timestamp_ms)`
    /// applies it once at flush time.
    background_css: String,
    /// Deferred mutations applied at the start of every `render()` (ADR-0030).
    pending: Vec<Command>,
}

#[wasm_bindgen]
impl HayateElementHtmlRenderer {
    pub fn new(container: HtmlElement) -> Result<HayateElementHtmlRenderer, JsValue> {
        inject_baseline_stylesheet()?;
        let style = container.style();
        style.set_property("position", "relative")?;
        style.set_property("overflow", "hidden")?;
        Ok(Self {
            container,
            nodes: HashMap::new(),
            root: None,
            events: RendererEventState::new(),
            background_css: "rgb(0,0,0)".to_string(),
            pending: Vec::new(),
        })
    }

    /// Store the container's CSS background colour for the next `render()`.
    /// Pairs with `HayateElementRenderer::set_background_color` so demos can
    /// drive either mode with the same setter.
    pub fn set_background_color(&mut self, r: f64, g: f64, b: f64) {
        self.background_css = format!(
            "rgb({},{},{})",
            (r * 255.0) as u8,
            (g * 255.0) as u8,
            (b * 255.0) as u8,
        );
    }

    /// Viewport is browser-managed in HTML Mode; this is kept for API parity
    /// with the Canvas renderer and only emits a Resize event.
    pub fn set_viewport(&mut self, width: f32, height: f32) {
        self.events.resize(None, width, height);
    }

    /// Registers an element with a caller-supplied ID and queues the DOM creation.
    /// The actual DOM element is materialised on the next `render()` (ADR-0030).
    pub fn element_create(&mut self, id: f64, kind: u32) -> Result<(), JsValue> {
        let k = kind_from_u32(kind)?;
        let eid = element_id_from_f64(id);
        self.nodes.insert(
            eid,
            HtmlNode {
                kind: k,
                dom: None,
                parent: None,
                children: Vec::new(),
                text: None,
                src: None,
            },
        );
        self.pending.push(Command::HtmlCreate { id: eid, kind: k });
        Ok(())
    }

    pub fn element_set_text(&mut self, id: f64, text: &str) {
        self.pending.push(Command::SetText {
            id: element_id_from_f64(id),
            text: text.to_string(),
        });
    }

    pub fn element_set_src(&mut self, id: f64, url: &str) {
        self.pending.push(Command::SetSrc {
            id: element_id_from_f64(id),
            url: url.to_string(),
        });
    }

    pub fn element_set_style(&mut self, id: f64, packed: &[f32]) -> Result<(), JsValue> {
        let props = style_packet::decode(packed)?;
        self.pending.push(Command::SetStyle {
            id: element_id_from_f64(id),
            props,
        });
        Ok(())
    }

    /// Queue a 2D affine transform update applied as CSS
    /// `transform: matrix(xx,yx,xy,yy,dx,dy)`. Matches the WIT `affine` record
    /// — identity is (1,0,0,1,0,0); there is no clear path.
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
        self.pending.push(Command::SetTransform {
            id: element_id_from_f64(id),
            matrix: Some([xx, yx, xy, yy, dx, dy]),
        });
    }

    pub fn element_append_child(&mut self, parent: f64, child: f64) {
        self.pending.push(Command::AppendChild {
            parent: element_id_from_f64(parent),
            child: element_id_from_f64(child),
        });
    }

    pub fn element_insert_before(&mut self, parent: f64, child: f64, before: f64) {
        self.pending.push(Command::InsertBefore {
            parent: element_id_from_f64(parent),
            child: element_id_from_f64(child),
            before: element_id_from_f64(before),
        });
    }

    pub fn element_remove(&mut self, id: f64) {
        self.pending.push(Command::Remove {
            id: element_id_from_f64(id),
        });
    }

    /// Returns the text committed by the most recent `render()`. Queued
    /// `element_set_text` calls are not visible until the next flush (ADR-0030).
    pub fn element_get_text(&self, id: f64) -> String {
        self.nodes
            .get(&element_id_from_f64(id))
            .and_then(|n| n.text.clone())
            .unwrap_or_default()
    }

    pub fn set_root(&mut self, id: f64) {
        self.pending.push(Command::SetRoot {
            id: element_id_from_f64(id),
        });
    }

    /// Drains the queued element mutations, then refreshes the container's
    /// background colour. The browser handles reflow for the freshly-applied
    /// styles in a single batch. `timestamp_ms` is accepted for API parity with
    /// the Canvas renderer (HTML Mode has no cursor blink to advance — the
    /// native `<input>` element handles it).
    pub fn render(&mut self, _timestamp_ms: f64) -> Result<(), JsValue> {
        self.flush_pending()?;
        self.container
            .style()
            .set_property("background-color", &self.background_css)?;
        Ok(())
    }

    // ── Input wiring ─────────────────────────────────────────────────────
    // HTML Mode does not run Taffy, so hit-tests cannot use a layout cache.
    // JS reads `data-element-id` from `event.target` and dispatches via the
    // explicit-target methods below. The legacy positional methods are
    // retained as no-ops so callers shared with Canvas Mode keep compiling.

    pub fn on_pointer_down(&mut self, target_id: f64, x: f32, y: f32) {
        let target = element_id_from_f64(target_id);
        if !self.nodes.contains_key(&target) {
            return;
        }
        self.events.pointer_down(None, Some(target), x, y);
    }

    pub fn on_pointer_up(&mut self, target_id: f64, _x: f32, _y: f32) {
        // Per ADR-0031 active-end reports the element that received active-start,
        // matching the natural drag/release semantics. Coordinates are no longer
        // part of the variant — use PointerMove for trailing-position tracking.
        let explicit = element_id_from_f64(target_id);
        let fallback = self.nodes.contains_key(&explicit).then_some(explicit);
        self.events.pointer_up(None, fallback);
    }

    pub fn on_pointer_move(&mut self, x: f32, y: f32) {
        // Target-less coordinate stream — hover state is driven separately by
        // the DOM mouseenter/mouseleave events.
        self.events.push_raw(Event::PointerMove { x, y });
    }

    pub fn on_pointer_enter(&mut self, target_id: f64) {
        let target = element_id_from_f64(target_id);
        if !self.nodes.contains_key(&target) {
            return;
        }
        self.events.hover_enter(None, target);
    }

    pub fn on_pointer_leave(&mut self, target_id: f64) {
        let target = element_id_from_f64(target_id);
        self.events.hover_leave(None, target);
    }

    pub fn on_wheel(&mut self, target_id: f64, delta_x: f32, delta_y: f32) {
        let target = element_id_from_f64(target_id);
        if self.nodes.contains_key(&target) {
            self.events.wheel(None, target, delta_x, delta_y);
        }
    }

    pub fn on_resize(&mut self, width: f32, height: f32) {
        self.events.resize(None, width, height);
    }

    pub fn element_set_scroll_offset(&mut self, id: f64, x: f32, y: f32) {
        self.pending.push(Command::SetScrollOffset {
            id: element_id_from_f64(id),
            x,
            y,
        });
    }

    pub fn element_set_font_family(&mut self, id: f64, family: &str) {
        self.pending.push(Command::SetFontFamily {
            id: element_id_from_f64(id),
            family: family.to_string(),
        });
    }

    /// Unset one or more inheritable text-style properties, delegating them to
    /// browser CSS inheritance (ADR-0047).
    /// `kinds` is a packed u32 array: 0 = Color, 1 = FontSize, 2 = FontFamily.
    pub fn element_unset_style(&mut self, id: f64, kinds: &[u32]) {
        self.pending.push(Command::UnsetStyle {
            id: element_id_from_f64(id),
            kinds: kinds.to_vec(),
        });
    }

    pub fn element_set_aria_label(&mut self, id: f64, label: &str) {
        self.pending.push(Command::SetAriaLabel {
            id: element_id_from_f64(id),
            label: label.to_string(),
        });
    }

    pub fn element_set_role(&mut self, id: f64, role: &str) {
        self.pending.push(Command::SetRole {
            id: element_id_from_f64(id),
            role: role.to_string(),
        });
    }

    /// Register a Web Font via CSS `@font-face`. Browser renders text in HTML
    /// Mode, so font registration is delegated to the document's CSS engine.
    pub fn register_font_bytes(&mut self, family_name: &str, data: &[u8]) {
        let _ = inject_font_face(family_name, data);
    }

    pub async fn load_font_from_url(
        &mut self,
        family_name: String,
        url: String,
    ) -> Result<(), JsValue> {
        let bytes = fetch_bytes(&url).await?;
        let _ = inject_font_face(&family_name, &bytes);
        Ok(())
    }

    /// Preload fonts declared in `hayate.config.json` before the first render.
    /// HTML Mode injects each as a CSS `@font-face` rule so the browser uses them.
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
            let _ = inject_font_face(&family, &bytes);
        }
        Ok(())
    }

    /// WIT `element-load-font`: HTML Mode cannot read the family name out of
    /// the font bytes (no Parley FontContext on the JS side). Surface as an
    /// `@font-face` with a synthetic family name so the data URL is at least
    /// resident in the document; consumers needing a specific family name
    /// should keep using `register_font_bytes`.
    pub fn element_load_font(&mut self, data: &[u8]) {
        // Generate a stable-but-unique family name from a content hash.
        let mut h: u64 = 0xcbf29ce484222325;
        for b in data {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        let family = format!("hayate-font-{h:016x}");
        let _ = inject_font_face(&family, data);
    }

    /// WIT `element-paste`: deliver pasted text to a specific TextInput,
    /// emitting a TextInput event. The browser commits the text into its
    /// native `<input>` value separately on the DOM `paste` event.
    pub fn element_paste(&mut self, id: f64, text: &str) {
        let eid = element_id_from_f64(id);
        if self.nodes.contains_key(&eid) {
            self.events.paste(None, eid, text);
        }
    }

    /// WIT `element-get-bounds`: return the element's CSS bounding box
    /// [x, y, width, height] in container-relative pixels. Returns zeroes when
    /// the element has not been laid out yet.
    pub fn element_get_bounds(&self, id: f64) -> Box<[f32]> {
        let eid = element_id_from_f64(id);
        let dom = match self.nodes.get(&eid).and_then(|n| n.dom.as_ref()) {
            Some(d) => d,
            None => return vec![0.0, 0.0, 0.0, 0.0].into_boxed_slice(),
        };
        let html_el = match dom.dyn_ref::<HtmlElement>() {
            Some(e) => e,
            None => return vec![0.0, 0.0, 0.0, 0.0].into_boxed_slice(),
        };
        // offsetLeft/Top are relative to the offsetParent — for our container-
        // rooted tree this matches the WIT "canvas coordinates" expectation.
        vec![
            html_el.offset_left() as f32,
            html_el.offset_top() as f32,
            html_el.offset_width() as f32,
            html_el.offset_height() as f32,
        ]
        .into_boxed_slice()
    }

    pub fn focused_element_id(&self) -> f64 {
        self.events.focused_element.map(element_id_to_f64).unwrap_or(0.0)
    }

    pub fn on_key_down(&mut self, key: &str, modifiers: u32) {
        self.events.key_down(None, key, modifiers);
    }

    pub fn on_text_input(&mut self, id: f64, text: &str) {
        let eid = element_id_from_f64(id);
        if self.nodes.contains_key(&eid) {
            self.events.text_input(None, eid, text);
        }
    }

    pub fn on_composition_start(&mut self, id: f64, text: &str) {
        let eid = element_id_from_f64(id);
        if self.nodes.contains_key(&eid) {
            self.events.composition_start(None, eid, text);
        }
    }

    pub fn on_composition_update(&mut self, id: f64, text: &str) {
        let eid = element_id_from_f64(id);
        if self.nodes.contains_key(&eid) {
            self.events.composition_update(None, eid, text);
        }
    }

    pub fn on_composition_end(&mut self, id: f64, text: &str) {
        let eid = element_id_from_f64(id);
        if self.nodes.contains_key(&eid) {
            self.events.composition_end(None, eid, text);
        }
    }

    pub fn element_set_text_content(&mut self, id: f64, text: &str) {
        self.pending.push(Command::SetTextContent {
            id: element_id_from_f64(id),
            text: text.to_string(),
        });
    }

    /// Returns the editable text content committed by the most recent `render()`.
    /// For TextInput elements this falls through to the live DOM value, which
    /// already reflects user typing (browser-driven, not queue-driven). Queued
    /// `element_set_text_content` calls are not visible until the next flush
    /// (ADR-0030).
    pub fn element_get_text_content(&self, id: f64) -> String {
        let eid = element_id_from_f64(id);
        let n = match self.nodes.get(&eid) {
            Some(n) => n,
            None => return String::new(),
        };
        if let Some(dom) = n.dom.as_ref() {
            if let Some(input) = dom.dyn_ref::<HtmlInputElement>() {
                return input.value();
            }
        }
        n.text.clone().unwrap_or_default()
    }

    /// Set the image's `src` to the URL. The browser fetches and decodes it.
    /// `src` is applied to the DOM eagerly here so the browser fetch can start
    /// before the next `render()`; the slotmap mirror is updated too so reads
    /// observe the new URL immediately.
    pub async fn load_image(&mut self, id: f64, url: String) -> Result<(), JsValue> {
        let eid = element_id_from_f64(id);
        if let Some(n) = self.nodes.get_mut(&eid) {
            if n.kind == ElementKind::Image {
                n.src = Some(url.clone());
                if let Some(dom) = n.dom.as_ref() {
                    let _ = dom.set_attribute("src", &url);
                }
            }
        }
        Ok(())
    }

    pub fn poll_events(&mut self) -> js_sys::Array {
        let events = self.events.drain_raw();
        encode_events(&events)
    }
}

impl HayateElementHtmlRenderer {
    fn detach_from_current_parent(&mut self, child: ElementId) {
        let parent = match self.nodes.get(&child).and_then(|c| c.parent) {
            Some(p) => p,
            None => return,
        };
        if let Some(p) = self.nodes.get_mut(&parent) {
            p.children.retain(|&c| c != child);
        }
        if let Some(c) = self.nodes.get_mut(&child) {
            c.parent = None;
        }
    }

    /// Drain the pending command queue and apply each mutation to the DOM and
    /// slotmap. Called from `render()` (the sole flush boundary per ADR-0030).
    fn flush_pending(&mut self) -> Result<(), JsValue> {
        let commands = std::mem::take(&mut self.pending);
        for cmd in commands {
            self.apply_command(cmd)?;
        }
        Ok(())
    }

    fn apply_command(&mut self, cmd: Command) -> Result<(), JsValue> {
        match cmd {
            Command::HtmlCreate { id, kind } => self.flush_create(id, kind)?,
            Command::SetText { id, text } => self.flush_set_text(id, &text),
            Command::SetSrc { id, url } => self.flush_set_src(id, &url),
            Command::SetStyle { id, props } => self.flush_set_style(id, &props)?,
            Command::UnsetStyle { id, kinds } => self.flush_unset_style(id, &kinds),
            Command::SetTransform { id, matrix } => self.flush_set_transform(id, matrix),
            Command::SetScrollOffset { id, x, y } => self.flush_set_scroll_offset(id, x, y),
            Command::SetFontFamily { id, family } => self.flush_set_font_family(id, &family),
            Command::SetAriaLabel { id, label } => self.flush_set_aria_label(id, &label),
            Command::SetRole { id, role } => self.flush_set_role(id, &role),
            Command::SetTextContent { id, text } => self.flush_set_text_content(id, &text),
            Command::AppendChild { parent, child } => self.flush_append_child(parent, child),
            Command::InsertBefore {
                parent,
                child,
                before,
            } => {
                self.flush_insert_before(parent, child, before);
            }
            Command::Remove { id } => self.flush_remove(id),
            Command::SetRoot { id } => self.flush_set_root(id),
        }
        Ok(())
    }

    fn flush_create(&mut self, id: ElementId, kind: ElementKind) -> Result<(), JsValue> {
        // The slot was inserted eagerly in `element_create`; if it's missing it
        // was removed by a subsequent queued `Remove` — skip silently.
        if !self.nodes.contains_key(&id) {
            return Ok(());
        }
        let dom = create_dom_for_kind(&document(), kind)?;
        apply_kind_baseline(&dom, kind)?;
        dom.set_attribute("data-element-id", &format!("{}", id.to_u64()))?;
        if let Some(n) = self.nodes.get_mut(&id) {
            n.dom = Some(dom.clone());
        }
        // Preserve the legacy auto-root behaviour: the first element created
        // when no root exists becomes the root and is mounted on the container.
        if self.root.is_none() {
            self.root = Some(id);
            self.container.append_child(&dom)?;
        }
        Ok(())
    }

    fn flush_set_text(&mut self, id: ElementId, text: &str) {
        let n = match self.nodes.get_mut(&id) {
            Some(n) => n,
            None => return,
        };
        n.text = Some(text.to_string());
        let dom = match n.dom.as_ref() {
            Some(d) => d,
            None => return,
        };
        match n.kind {
            ElementKind::TextInput => {
                if let Some(input) = dom.dyn_ref::<HtmlInputElement>() {
                    input.set_value(text);
                }
            }
            _ => {
                if let Some(html_el) = dom.dyn_ref::<HtmlElement>() {
                    html_el.set_inner_text(text);
                }
            }
        }
    }

    fn flush_set_src(&mut self, id: ElementId, url: &str) {
        let n = match self.nodes.get_mut(&id) {
            Some(n) => n,
            None => return,
        };
        n.src = Some(url.to_string());
        if n.kind == ElementKind::Image {
            if let Some(dom) = n.dom.as_ref() {
                let _ = dom.set_attribute("src", url);
            }
        }
    }

    fn flush_set_style(&mut self, id: ElementId, props: &[StyleProp]) -> Result<(), JsValue> {
        let dom = match self.nodes.get(&id).and_then(|n| n.dom.clone()) {
            Some(d) => d,
            None => return Ok(()),
        };
        if let Some(html_el) = dom.dyn_ref::<HtmlElement>() {
            style_packet::apply_props_to_dom(&html_el.style(), props)?;
        }
        Ok(())
    }

    fn flush_unset_style(&mut self, id: ElementId, kinds: &[u32]) {
        let dom = match self.nodes.get(&id).and_then(|n| n.dom.clone()) {
            Some(d) => d,
            None => return,
        };
        if let Some(html_el) = dom.dyn_ref::<HtmlElement>() {
            let style = html_el.style();
            for &kind in kinds {
                match kind {
                    0 => {
                        let _ = style.remove_property("color");
                    }
                    1 => {
                        let _ = style.remove_property("font-size");
                    }
                    2 => {
                        let _ = style.remove_property("font-family");
                    }
                    3 => {
                        let _ = style.remove_property("font-weight");
                    }
                    _ => {}
                }
            }
        }
    }

    fn flush_set_transform(&mut self, id: ElementId, matrix: Option<[f64; 6]>) {
        let dom = match self.nodes.get(&id).and_then(|n| n.dom.clone()) {
            Some(d) => d,
            None => return,
        };
        let html_el = match dom.dyn_ref::<HtmlElement>() {
            Some(e) => e,
            None => return,
        };
        let style = html_el.style();
        match matrix {
            Some(m) => {
                let css = format!(
                    "matrix({},{},{},{},{},{})",
                    m[0], m[1], m[2], m[3], m[4], m[5]
                );
                let _ = style.set_property("transform", &css);
            }
            None => {
                let _ = style.set_property("transform", "none");
            }
        }
    }

    fn flush_set_scroll_offset(&mut self, id: ElementId, x: f32, y: f32) {
        if let Some(dom) = self.nodes.get(&id).and_then(|n| n.dom.as_ref()) {
            dom.set_scroll_left(x as i32);
            dom.set_scroll_top(y as i32);
        }
    }

    fn flush_set_font_family(&mut self, id: ElementId, family: &str) {
        let dom = match self.nodes.get(&id).and_then(|n| n.dom.clone()) {
            Some(d) => d,
            None => return,
        };
        if let Some(html_el) = dom.dyn_ref::<HtmlElement>() {
            let _ = html_el.style().set_property("font-family", family);
        }
    }

    fn flush_set_aria_label(&mut self, id: ElementId, label: &str) {
        if let Some(dom) = self.nodes.get(&id).and_then(|n| n.dom.as_ref()) {
            let _ = dom.set_attribute("aria-label", label);
        }
    }

    fn flush_set_role(&mut self, id: ElementId, role: &str) {
        if let Some(dom) = self.nodes.get(&id).and_then(|n| n.dom.as_ref()) {
            let _ = dom.set_attribute("role", role);
        }
    }

    fn flush_set_text_content(&mut self, id: ElementId, text: &str) {
        let n = match self.nodes.get_mut(&id) {
            Some(n) => n,
            None => return,
        };
        n.text = Some(text.to_string());
        let dom = match n.dom.as_ref() {
            Some(d) => d,
            None => return,
        };
        if let Some(input) = dom.dyn_ref::<HtmlInputElement>() {
            input.set_value(text);
        } else if let Some(html_el) = dom.dyn_ref::<HtmlElement>() {
            html_el.set_inner_text(text);
        }
    }

    fn flush_append_child(&mut self, pid: ElementId, cid: ElementId) {
        if !self.nodes.contains_key(&pid) || !self.nodes.contains_key(&cid) {
            return;
        }
        self.detach_from_current_parent(cid);
        let parent_dom = self.nodes[&pid].dom.clone();
        let child_dom = self.nodes[&cid].dom.clone();
        if let (Some(p), Some(c)) = (parent_dom, child_dom) {
            let _ = p.append_child(c.as_ref());
        }
        if let Some(p) = self.nodes.get_mut(&pid) {
            p.children.push(cid);
        }
        if let Some(c) = self.nodes.get_mut(&cid) {
            c.parent = Some(pid);
        }
    }

    fn flush_insert_before(&mut self, pid: ElementId, cid: ElementId, bid: ElementId) {
        if !self.nodes.contains_key(&pid)
            || !self.nodes.contains_key(&cid)
            || !self.nodes.contains_key(&bid)
        {
            return;
        }
        self.detach_from_current_parent(cid);
        let index = match self.nodes[&pid].children.iter().position(|&c| c == bid) {
            Some(i) => i,
            None => {
                self.flush_append_child(pid, cid);
                return;
            }
        };
        let parent_dom = self.nodes[&pid].dom.clone();
        let child_dom = self.nodes[&cid].dom.clone();
        let before_dom = self.nodes[&bid].dom.clone();
        if let (Some(p), Some(c), Some(b)) = (parent_dom, child_dom, before_dom) {
            let _ = p
                .unchecked_ref::<Node>()
                .insert_before(c.as_ref(), Some(b.as_ref()));
        }
        if let Some(p) = self.nodes.get_mut(&pid) {
            p.children.insert(index, cid);
        }
        if let Some(c) = self.nodes.get_mut(&cid) {
            c.parent = Some(pid);
        }
    }

    fn flush_remove(&mut self, target: ElementId) {
        if !self.nodes.contains_key(&target) {
            return;
        }
        self.detach_from_current_parent(target);
        // DOM removeChild cascades to descendants; we only need to drop the
        // top-level DOM node from its parent (or the container if it was root).
        if let Some(top_dom) = self.nodes[&target].dom.clone() {
            if let Some(parent_dom) = top_dom.parent_node() {
                let _ = parent_dom.remove_child(top_dom.as_ref());
            }
        }
        // Drop the slotmap entries for the subtree.
        let mut stack = vec![target];
        while let Some(node) = stack.pop() {
            if let Some(n) = self.nodes.remove(&node) {
                stack.extend(n.children.iter().copied());
            }
        }
        if self.root == Some(target) {
            self.root = None;
        }
        // Clear any pointer-state that referred to a removed node (including
        // descendants — the subtree walk above already removed them from self.nodes).
        self.events.on_subtree_remove(|c| !self.nodes.contains_key(&c));
    }

    fn flush_set_root(&mut self, new_root: ElementId) {
        if !self.nodes.contains_key(&new_root) {
            return;
        }
        // Detach the previous root from the container (if any).
        if let Some(prev) = self.root {
            if prev != new_root {
                if let Some(prev_dom) = self.nodes[&prev].dom.clone() {
                    let _ = self.container.remove_child(prev_dom.as_ref());
                }
            }
        }
        // Lift the new root out of any prior parent and mount it on the container.
        self.detach_from_current_parent(new_root);
        if let Some(dom) = self.nodes[&new_root].dom.clone() {
            let _ = self.container.append_child(dom.as_ref());
        }
        self.root = Some(new_root);
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

/// Document-level CSS baseline injected once per page load.
///
/// Uses a `<style id="hayate-reset">` sentinel to be idempotent.
/// A global rule covers all elements in the document — including hidden
/// DOM trees created by Canvas-mode mocks — with no per-element overhead.
fn inject_baseline_stylesheet() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("no window")?;
    let doc = window.document().ok_or("no document")?;
    if doc.get_element_by_id("hayate-reset").is_some() {
        return Ok(());
    }
    let head = doc.head().ok_or("no head")?;
    let style_el = doc.create_element("style")?;
    style_el.set_id("hayate-reset");
    style_el.set_text_content(Some(
        "*, *::before, *::after { box-sizing: border-box; margin: 0; padding: 0; } \
         html { font-size: 16px; line-height: 1; -webkit-text-size-adjust: 100%; } \
         body { font-size: inherit; line-height: inherit; } \
         img, canvas, svg, video { display: block; } \
         input, button, select, textarea { font: inherit; color: inherit; appearance: none; }",
    ));
    head.append_child(style_el.as_ref())?;
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
    let css =
        format!("@font-face {{ font-family: '{family}'; src: url(data:font/ttf;base64,{b64}); }}");
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
/// Returns true if `candidate` is `root` or a descendant of `root` in the tree.
/// Must be called before the subtree is removed from the tree.
fn is_in_subtree(tree: &ElementTree, candidate: ElementId, root: ElementId) -> bool {
    let mut cur = candidate;
    loop {
        if cur == root {
            return true;
        }
        match tree.element_parent(cur) {
            Some(p) => cur = p,
            None => return false,
        }
    }
}

/// Fetch raw bytes from a URL.
async fn fetch_bytes(url: &str) -> Result<Vec<u8>, JsValue> {
    use js_sys::{ArrayBuffer, Uint8Array};
    let window = web_sys::window().ok_or("no window")?;
    let resp: web_sys::Response = JsFuture::from(window.fetch_with_str(url))
        .await?
        .dyn_into()?;
    if !resp.ok() {
        return Err(JsValue::from_str(&format!(
            "fetch failed: {} {}",
            resp.status(),
            resp.status_text()
        )));
    }
    let buf: ArrayBuffer = JsFuture::from(resp.array_buffer()?).await?.dyn_into()?;
    Ok(Uint8Array::new(&buf).to_vec())
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
