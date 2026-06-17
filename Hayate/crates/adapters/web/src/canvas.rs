//! Canvas Mode renderer (`HayateElementRenderer`). See ADR-0077.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use crate::pointer_input::{self, PointerInput, PointerInputGuard};
use crate::resize_observer::{self, ResizeObserverGuard};
use crate::scroll_drag::{self, MoveOutcome, ScrollGesture};

use hayate_core::{
    BorderStyleValue, Color, CursorValue, DocumentEventKind, ElementId, ElementKind, ElementTree,
    Event, FontStyleValue, RenderImage, RenderImageAlphaType, RenderImageFormat, StyleProp,
    StylePropKind, TextDecorationValue,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::HtmlCanvasElement;

use crate::apply_mutations_dispatch::{
    apply_mutations_batch, unset_kind_from_u32, ApplyMutationsHost,
};
use crate::backend::{CanvasBackend, SelectedBackend};
use crate::builtin_fonts::font_url_for_renderer;
use crate::generated::encode_deliveries;
use crate::ime_bridge::{sync_ime_character_bounds, WebImeBridge};
use crate::style_packet;

use crate::shared::{element_id_from_f64, element_id_to_f64, fetch_bytes, kind_from_u32};

/// Fonts fetched asynchronously by the adapter; drained into the tree on the
/// next `poll_events()` call (single-threaded WASM — Rc<RefCell> is safe).
type FontQueue = Rc<RefCell<Vec<(String, Vec<u8>)>>>;

/// Families whose on-demand fetch failed; drained into `tree.font_fetch_failed`
/// on the next `render()` so core can re-request (or give up) — without this the
/// family stayed latched in `pending` forever (issue #343).
type FontFailureQueue = Rc<RefCell<Vec<String>>>;

/// Per-family failed-attempt count, used only to space retries with exponential
/// backoff. Core owns the *budget* (when to give up); this owns the *timing*.
type FontFetchAttempts = Rc<RefCell<HashMap<String, u32>>>;

/// Backoff before reporting a failed fetch: `BASE << (attempt - 1)`, capped.
/// A fresh GitHub Pages deploy can see jsdelivr return a transient 403/429, so
/// the first retry is quick and later ones back off (issue #343).
const FETCH_BACKOFF_BASE_MS: i32 = 400;
const FETCH_BACKOFF_MAX_MS: i32 = 5_000;

/// Resolve after `ms` via `setTimeout`, so a spawned fetch future can await a
/// backoff delay before reporting failure.
async fn backoff_sleep(ms: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        if let Some(win) = web_sys::window() {
            let cb = Closure::once_into_js(move || {
                let _ = resolve.call0(&JsValue::NULL);
            });
            let _ = win.set_timeout_with_callback_and_timeout_and_arguments_0(
                cb.unchecked_ref(),
                ms,
            );
        }
    });
    let _ = JsFuture::from(promise).await;
}

/// Web implementation of the core `Clipboard` seam (ADR-0097, #268). Copy
/// (Cmd/Ctrl+C) runs in core; core hands the selected text here, and the
/// adapter writes it via the async Clipboard API. The write is fire-and-forget:
/// it is initiated synchronously inside the user-gesture keydown that core just
/// processed, which is what the browser requires to authorize the write.
struct WebClipboard;

impl hayate_core::Clipboard for WebClipboard {
    fn write_text(&self, text: &str) {
        if let Some(clipboard) = web_sys::window().map(|w| w.navigator().clipboard()) {
            let _ = clipboard.write_text(text);
        }
    }
}

// ── Canvas Mode renderer ─────────────────────────────────────────────────

#[wasm_bindgen]
pub struct HayateElementRenderer {
    canvas: HtmlCanvasElement,
    backend: SelectedBackend,
    tree: ElementTree,
    /// wgpu surface clear colour. Decoupled from `render(timestamp_ms)` because
    /// the WIT `render` signature no longer carries it (ADR-0032 keeps render
    /// timestamp-only); call `set_background_color` separately.
    background: [f32; 4],
    /// Fonts fetched by spawned futures; applied to the tree on next poll_events.
    font_queue: FontQueue,
    /// Families whose fetch failed; reported to core on the next `render()`.
    font_failure_queue: FontFailureQueue,
    /// Per-family failed-attempt count, for exponential retry backoff.
    font_fetch_attempts: FontFetchAttempts,
    /// IME candidate-window bounds synced each render (ADR-0069).
    ime: WebImeBridge,
    /// ResizeObserver callback queues viewport metrics for the next `render()`.
    pending_resize: Rc<RefCell<Option<resize_observer::CanvasResizeMetrics>>>,
    last_viewport: Rc<RefCell<(f32, f32)>>,
    _resize_observer: ResizeObserverGuard,
    /// Self-wired pointer listeners (ADR-0080) enqueue here; drained in arrival
    /// order at the start of `render()`.
    pending_pointer: Rc<RefCell<Vec<PointerInput>>>,
    /// Last move position applied on a drain, used to seed 1px move-coalescing
    /// across frame boundaries.
    last_pointer_move: Option<(f32, f32)>,
    /// Active touch/pen drag→scroll gesture, locked to one scroll-view between
    /// frames (ADR-0082, #350). `None` when no touch press is in flight or the
    /// press is on a non-scrollable area.
    scroll_gesture: Option<ScrollGesture>,
    /// Finger samples `(x, y, frame_ms)` recorded while the active gesture is
    /// scrolling, fed to `estimate_release_velocity` on release to launch
    /// momentum (ADR-0082 Amendment, #351). Cleared on every fresh press.
    scroll_samples: Vec<(f32, f32, f64)>,
    /// Raw (un-resisted) accumulated finger offset of the active drag, used to
    /// drive the rubber-band: the finger moves this 1:1, and the *displayed*
    /// offset is `rubber_band_offset(raw, …)` so overscroll past an edge lags the
    /// finger (#352). `None` when no drag is scrolling; seeded on the first
    /// `Scroll` and cleared on press / release / cancel.
    drag_raw: Option<(ElementId, (f32, f32))>,
    /// In-flight released scroll: the locked scroll-view and its offset-space
    /// velocity (px/ms per axis). Each frame `scroll_motion_step` integrates it —
    /// friction while inside the range, a spring while in overscroll — so a fling
    /// coasts (#351), bounces at an edge, and springs back home (#352). `None`
    /// when nothing is animating.
    scroll_motion: Option<(ElementId, (f32, f32))>,
    /// Timestamp of the previous `render()` frame, for the inter-frame `dt` the
    /// momentum integrator advances by. `None` before the first frame.
    last_frame_ms: Option<f64>,
    _pointer_input: PointerInputGuard,
}

#[wasm_bindgen]
impl HayateElementRenderer {
    pub async fn init(canvas: HtmlCanvasElement) -> Result<HayateElementRenderer, JsValue> {
        let rect = canvas.get_bounding_client_rect();
        let dpr = web_sys::window()
            .map(|w| w.device_pixel_ratio())
            .unwrap_or(1.0);
        let metrics =
            resize_observer::canvas_resize_metrics(rect.width() as f32, rect.height() as f32, dpr);
        canvas.set_width(metrics.buffer_width);
        canvas.set_height(metrics.buffer_height);

        let mut backend = SelectedBackend::init(canvas.clone()).await?;
        backend.resize(
            metrics.buffer_width,
            metrics.buffer_height,
            metrics.content_scale,
        );
        let mut tree = ElementTree::new();
        tree.set_viewport(metrics.viewport_width, metrics.viewport_height);
        // Wire the Platform Adapter clipboard so core copy (Cmd/Ctrl+C) reaches
        // the browser Clipboard API (ADR-0097, #268).
        tree.set_clipboard(Box::new(WebClipboard));

        let pending_resize = Rc::new(RefCell::new(None));
        let last_viewport = Rc::new(RefCell::new((
            metrics.viewport_width,
            metrics.viewport_height,
        )));
        let resize_guard = resize_observer::attach_resize_observer(
            &canvas,
            pending_resize.clone(),
            last_viewport.clone(),
        )?;

        let pending_pointer = Rc::new(RefCell::new(Vec::new()));
        let pointer_guard =
            pointer_input::attach_pointer_input(&canvas, pending_pointer.clone())?;

        Ok(Self {
            canvas,
            backend,
            tree,
            background: [0.0, 0.0, 0.0, 1.0],
            font_queue: Rc::new(RefCell::new(Vec::new())),
            font_failure_queue: Rc::new(RefCell::new(Vec::new())),
            font_fetch_attempts: Rc::new(RefCell::new(HashMap::new())),
            ime: WebImeBridge::default(),
            pending_resize,
            last_viewport,
            _resize_observer: resize_guard,
            pending_pointer,
            last_pointer_move: None,
            scroll_gesture: None,
            scroll_samples: Vec::new(),
            drag_raw: None,
            scroll_motion: None,
            last_frame_ms: None,
            _pointer_input: pointer_guard,
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

    pub fn element_set_selectable(&mut self, id: f64, selectable: bool) {
        self.tree
            .element_set_selectable(element_id_from_f64(id), selectable);
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

    /// Resolved style of `id` after inheritance + pseudo-state (ADR-0067).
    /// Returns `null` if `id` is unknown, otherwise a JS object with the
    /// effective `Visual` fields (camelCase keys, colors as `{r,g,b,a}`).
    pub fn element_effective_visual(&self, id: f64) -> JsValue {
        let eid = element_id_from_f64(id);
        let Some(visual) = self.tree.element_effective_visual(eid) else {
            return JsValue::NULL;
        };

        let obj = js_sys::Object::new();
        let set = |key: &str, value: JsValue| {
            js_sys::Reflect::set(&obj, &JsValue::from_str(key), &value).unwrap();
        };
        set("backgroundColor", color_to_js(visual.background_color));
        set("opacity", JsValue::from_f64(visual.opacity as f64));
        set("borderRadius", JsValue::from_f64(visual.border_radius as f64));
        set("borderWidth", JsValue::from_f64(visual.border_width as f64));
        set("borderColor", color_to_js(visual.border_color));
        set("borderStyle", border_style_to_js(visual.border_style));
        set("textColor", color_to_js(visual.text_color));
        set(
            "fontSize",
            visual
                .font_size
                .map(|v| JsValue::from_f64(v as f64))
                .unwrap_or(JsValue::NULL),
        );
        set(
            "fontWeight",
            visual
                .font_weight
                .map(|v| JsValue::from_f64(v as f64))
                .unwrap_or(JsValue::NULL),
        );
        set("fontStyle", font_style_to_js(visual.font_style));
        set("textDecoration", text_decoration_to_js(visual.text_decoration));
        set("zIndex", JsValue::from_f64(visual.z_index as f64));
        set(
            "fontFamily",
            visual
                .font_family
                .map(|f| JsValue::from_str(&f))
                .unwrap_or(JsValue::NULL),
        );
        obj.into()
    }

    pub fn set_root(&mut self, id: f64) {
        self.tree.set_root(element_id_from_f64(id));
    }

    /// Advance cursor blink, run layout, and present. `timestamp_ms` should be a
    /// monotonic clock (e.g. `performance.now()`). Mutations are applied eagerly
    /// by the `element_*` setters (ADR-0037), so `render` only drives layout.
    pub fn render(&mut self, timestamp_ms: f64) -> Result<(), JsValue> {
        let pending = self.pending_resize.borrow_mut().take();
        if let Some(metrics) = pending {
            self.apply_resize(metrics);
        }
        self.drain_pointer_inputs(timestamp_ms);
        // After draining this frame's inputs (a fresh press interrupts the
        // animation, a release launches it), advance any in-flight scroll motion
        // by the inter-frame dt so inertia, bounce and spring-back integrate on
        // the same rAF loop as layout (#351, #352).
        self.step_scroll_motion(timestamp_ms);
        self.last_frame_ms = Some(timestamp_ms);
        // Report failed fetches to core first: each marks fonts dirty so the
        // commit_layout below re-shapes, re-detects the gap, and re-emits a
        // FetchFont on the next poll_events (issue #343). A family core has given
        // up on stops re-requesting, so we drop its backoff counter too.
        let failures: Vec<String> = self.font_failure_queue.borrow_mut().drain(..).collect();
        for family in failures {
            if !self.tree.font_fetch_failed(&family) {
                self.font_fetch_attempts.borrow_mut().remove(&family);
            }
        }
        // フェッチ完了フォントを layout より前に登録することで、同フレーム内で
        // fonts_dirty → compute_layout → 正しいグリフ、が成立する。
        // （poll_events より先に render が呼ばれる raf ループでも豆腐にならない）
        let loaded: Vec<(String, Vec<u8>)> = self.font_queue.borrow_mut().drain(..).collect();
        for (family, bytes) in loaded {
            self.font_fetch_attempts.borrow_mut().remove(&family);
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
        let result = self.tree.on_pointer_move(x, y);
        apply_resolved_cursor(&self.canvas, result.resolved_cursor);
    }

    pub fn on_wheel(&mut self, x: f32, y: f32, delta_x: f32, delta_y: f32) {
        if let Some(target) = self.tree.hit_test(x, y) {
            self.tree.apply_wheel_delta(target, delta_x, delta_y);
            self.tree.on_wheel(target, delta_x, delta_y);
        }
    }

    /// Drain the self-wired pointer buffer at the start of `render()`, applying
    /// each input to the tree in arrival order with 1px move-coalescing (ADR-0080).
    fn drain_pointer_inputs(&mut self, now_ms: f64) {
        let buffered: Vec<PointerInput> = self.pending_pointer.borrow_mut().drain(..).collect();
        if buffered.is_empty() {
            return;
        }
        let inputs = pointer_input::coalesce_pointer_inputs(buffered, self.last_pointer_move);
        self.last_pointer_move = pointer_input::final_anchor(&inputs, self.last_pointer_move);
        for input in inputs {
            self.apply_pointer_input(input, now_ms);
        }
    }

    fn apply_pointer_input(&mut self, input: PointerInput, now_ms: f64) {
        match input {
            PointerInput::Down {
                x,
                y,
                modifiers,
                kind,
            } => {
                // Always send the press first so a tap still shows `:active`
                // (#213), forwarding the device so Core retains it per
                // interaction (#357). A touch/pen press over a scroll-view then
                // locks a drag→scroll gesture; if the slop is never crossed the
                // release resolves as a normal click.
                self.tree.on_pointer_down_with_kind(x, y, modifiers, kind);
                self.scroll_gesture = None;
                // A fresh press interrupts any coasting fling or spring-back so the
                // content is immediately grabbable (#351) — start from rest.
                self.scroll_motion = None;
                self.drag_raw = None;
                self.scroll_samples.clear();
                if scroll_drag::is_drag_scroll_pointer(kind) {
                    if let Some(sv) = self
                        .tree
                        .hit_test(x, y)
                        .and_then(|hit| self.nearest_scroll_view(hit))
                    {
                        self.scroll_gesture = Some(ScrollGesture::new(sv, (x, y)));
                    }
                }
            }
            PointerInput::Move { x, y, kind } => {
                if let Some(mut gesture) = self.scroll_gesture.take() {
                    match gesture.on_move((x, y), scroll_drag::SCROLL_SLOP_PX) {
                        // Still a pending tap — leave the press alive.
                        MoveOutcome::Pending => {}
                        // Slop crossed: release the press (#213) so the touch
                        // becomes a scroll and no click fires on release. Seed the
                        // velocity tracker from the takeover position.
                        MoveOutcome::StartScroll => {
                            self.tree.on_pointer_cancel();
                            self.scroll_samples.push((x, y, now_ms));
                        }
                        // Drag the locked scroll-view with the finger (1:1 inside
                        // the range, rubber-band resisted past an edge), and record
                        // the sample so a release can estimate the fling.
                        MoveOutcome::Scroll { dx, dy } => {
                            self.apply_drag_delta(gesture.scroll_view, dx, dy);
                            self.scroll_samples.push((x, y, now_ms));
                        }
                    }
                    self.scroll_gesture = Some(gesture);
                } else {
                    let result = self.tree.on_pointer_move_with_kind(x, y, kind);
                    apply_resolved_cursor(&self.canvas, result.resolved_cursor);
                }
            }
            PointerInput::Up { x, y, kind } => {
                // A touch that never crossed the slop is a tap → resolve the
                // click. One that became a scroll already had its press
                // cancelled, so swallow the up and launch the released motion —
                // momentum from the sampled fling, and/or spring-back if it was
                // let go in overscroll (#351, #352).
                match self.scroll_gesture.take() {
                    Some(gesture) if !gesture.is_tap() => {
                        self.launch_scroll_motion(gesture.scroll_view)
                    }
                    _ => self.tree.on_pointer_up_with_kind(x, y, kind),
                }
            }
            PointerInput::Leave => self.tree.on_pointer_leave(),
            PointerInput::Cancel => {
                self.scroll_gesture = None;
                self.drag_raw = None;
                self.scroll_samples.clear();
                self.tree.on_pointer_cancel();
            }
            PointerInput::Wheel {
                x,
                y,
                delta_x,
                delta_y,
            } => {
                if let Some(target) = self.tree.hit_test(x, y) {
                    self.tree.apply_wheel_delta(target, delta_x, delta_y);
                    self.tree.on_wheel(target, delta_x, delta_y);
                }
            }
        }
    }

    /// Walk up from `id` to its nearest ScrollView ancestor (inclusive), the
    /// element a touch gesture locks onto. Mirrors Core's wheel-path
    /// `nearest_scroll_view` using the public kind/parent queries so the gesture
    /// lock lives in the adapter (ADR-0082, #350).
    fn nearest_scroll_view(&self, mut id: ElementId) -> Option<ElementId> {
        loop {
            if self.tree.element_kind(id) == Some(ElementKind::ScrollView) {
                return Some(id);
            }
            id = self.tree.element_parent(id)?;
        }
    }

    /// Per-axis scroll bounds of `sv`: `(max_x, max_y, dim_x, dim_y)` where `max`
    /// is the scrollable range (`content − viewport`, floored at 0) and `dim` is
    /// the viewport extent the rubber-band overscroll asymptotes to.
    fn scroll_bounds(&self, sv: ElementId) -> (f32, f32, f32, f32) {
        let (content_w, content_h) = self.tree.element_content_size(sv);
        let (_, _, view_w, view_h) = self
            .tree
            .element_layout_rect(sv)
            .unwrap_or((0.0, 0.0, 0.0, 0.0));
        (
            (content_w - view_w).max(0.0),
            (content_h - view_h).max(0.0),
            view_w,
            view_h,
        )
    }

    /// Set the locked scroll-view's offset un-clamped (SCR-02) and, when it
    /// actually moved, fire `Event::Scroll` so parallax / lazy-load react to touch
    /// scrolling too (ADR-0082). Offsets outside `[0, max]` are intentional — the
    /// rubber-band drag and the spring-back / bounce animation both live in
    /// overscroll. Offset application and scroll-notify are distinct calls.
    fn commit_scroll_offset(&mut self, sv: ElementId, nx: f32, ny: f32) {
        let (ox, oy) = self.tree.element_get_scroll_offset(sv);
        let (dx, dy) = (nx - ox, ny - oy);
        if dx.abs() > 1e-6 || dy.abs() > 1e-6 {
            self.tree.element_set_scroll_offset(sv, nx, ny);
            self.tree.on_wheel(sv, dx, dy);
        }
    }

    /// Apply a finger-driven drag delta to the locked scroll-view through the
    /// rubber-band: the finger moves a *raw* offset 1:1, and the displayed offset
    /// is `rubber_band_offset(raw, …)`, so inside the range it tracks the finger
    /// exactly and past an edge it lags with growing resistance (#352). The raw
    /// accumulator is seeded from the current offset on the first drag frame.
    fn apply_drag_delta(&mut self, sv: ElementId, dx: f32, dy: f32) {
        let (max_x, max_y, dim_x, dim_y) = self.scroll_bounds(sv);
        let (rx, ry) = match self.drag_raw {
            Some((s, raw)) if s == sv => raw,
            _ => self.tree.element_get_scroll_offset(sv),
        };
        let (rx, ry) = (rx + dx, ry + dy);
        self.drag_raw = Some((sv, (rx, ry)));
        let nx = scroll_drag::rubber_band_offset(rx, max_x, dim_x);
        let ny = scroll_drag::rubber_band_offset(ry, max_y, dim_y);
        self.commit_scroll_offset(sv, nx, ny);
    }

    /// On release of a scroll gesture, hand the locked scroll-view its released
    /// motion: the fling velocity estimated from the recorded finger samples.
    /// It coasts if there is a real fling, and it also animates when the finger
    /// let go in overscroll (velocity ≈ 0) so the edge always springs back home
    /// (#351, #352). A slow release that ends inside the range leaves nothing
    /// animating.
    fn launch_scroll_motion(&mut self, sv: ElementId) {
        let (vx, vy) = scroll_drag::estimate_release_velocity(&self.scroll_samples);
        self.scroll_samples.clear();
        self.drag_raw = None;
        let (max_x, max_y, _, _) = self.scroll_bounds(sv);
        let (ox, oy) = self.tree.element_get_scroll_offset(sv);
        let out_of_bounds = ox < 0.0 || ox > max_x || oy < 0.0 || oy > max_y;
        let has_fling = vx.abs() >= scroll_drag::physics::MIN_VELOCITY
            || vy.abs() >= scroll_drag::physics::MIN_VELOCITY;
        self.scroll_motion = (has_fling || out_of_bounds).then_some((sv, (vx, vy)));
    }

    /// Advance the released scroll by one frame: `scroll_motion_step` integrates
    /// each axis (friction inside the range, spring in overscroll), the new offset
    /// is committed un-clamped (firing `Event::Scroll` like a finger drag), and
    /// the decayed velocity carries on. The animation ends once both axes rest
    /// **and** are back within `[0, max]`, so an inertial bounce keeps running
    /// through its overscroll excursion until the spring brings it home (#351,
    /// #352).
    fn step_scroll_motion(&mut self, now_ms: f64) {
        let Some((sv, (vx, vy))) = self.scroll_motion else {
            return;
        };
        // Need a real inter-frame span to integrate; carry velocity until we do.
        let dt = match self.last_frame_ms {
            Some(prev) if now_ms > prev => (now_ms - prev) as f32,
            _ => return,
        };
        let (max_x, max_y, _, _) = self.scroll_bounds(sv);
        let (ox, oy) = self.tree.element_get_scroll_offset(sv);
        let (nx, vx2) = scroll_drag::scroll_motion_step(ox, vx, max_x, dt);
        let (ny, vy2) = scroll_drag::scroll_motion_step(oy, vy, max_y, dt);
        self.commit_scroll_offset(sv, nx, ny);
        // An axis is still animating while it carries velocity or sits in
        // overscroll (a bounce mid-excursion may momentarily read zero velocity).
        let x_active = vx2 != 0.0 || nx < 0.0 || nx > max_x;
        let y_active = vy2 != 0.0 || ny < 0.0 || ny > max_y;
        self.scroll_motion = (x_active || y_active).then_some((sv, (vx2, vy2)));
    }

    pub fn on_resize(&mut self, width: f32, height: f32, scale: f32) {
        let metrics = resize_observer::canvas_resize_metrics(width, height, scale as f64);
        self.canvas.set_width(metrics.buffer_width);
        self.canvas.set_height(metrics.buffer_height);
        self.apply_resize(metrics);
    }

    fn apply_resize(&mut self, metrics: resize_observer::CanvasResizeMetrics) {
        self.tree
            .set_viewport(metrics.viewport_width, metrics.viewport_height);
        self.backend.resize(
            metrics.buffer_width,
            metrics.buffer_height,
            metrics.content_scale,
        );
        self.tree
            .on_resize(metrics.viewport_width, metrics.viewport_height);
        *self.last_viewport.borrow_mut() = (metrics.viewport_width, metrics.viewport_height);
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

    /// Current scroll offset `[x, y]` of an element (0,0 when unknown). Symmetric
    /// with `element_set_scroll_offset`; lets the host read touch-driven scroll
    /// position back (ADR-0082, #350).
    pub fn element_get_scroll_offset(&self, id: f64) -> Box<[f32]> {
        let (x, y) = self.tree.element_get_scroll_offset(element_id_from_f64(id));
        vec![x, y].into_boxed_slice()
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

    /// Whether a document-wide text selection is active (ADR-0097, #267). The
    /// host dispatches keyboard selection gestures (Ctrl/Cmd+A, Shift+Arrow) when
    /// this is true even if no element is focused (read-only Selection Region).
    pub fn has_selection(&self) -> bool {
        self.tree.selection().is_some()
    }

    /// Physical device behind the most recent pointer interaction (#357), as the
    /// `PointerKind` wire discriminant (`mouse=0`, `touch=1`, `pen=2`). Retained
    /// per interaction so the host (and later slices) can branch on it.
    pub fn last_pointer_kind(&self) -> u32 {
        self.tree.last_pointer_kind().to_u32()
    }

    /// Handle a key press on the focused element. Editing keys are mapped to an
    /// [`EditIntent`] by the adapter's OS keymap and applied through core's
    /// editing seam (ADR-0103); everything else falls through to the raw
    /// `on_key_down` path (non-editing keys and the app `KeyDown` notification).
    pub fn on_key_down(&mut self, key: &str, modifiers: u32) {
        if let Some(intent) = crate::edit_keymap::key_to_edit_intent(key, modifiers) {
            if let Some(focused) = self.tree.focused_element() {
                if self.tree.apply_edit_intent(focused, intent) {
                    return;
                }
            }
        }
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

    /// Called by JS when the IME preedit updates, carrying the EditContext
    /// `textformatupdate` clause format ranges (ADR-0102, #336) so Canvas Mode
    /// draws the per-clause conversion underlines. `formats` is the flat
    /// `[start, end, weight, …]` triple stream (byte offsets into `text`;
    /// `weight == 0` thin, non-zero thick).
    pub fn on_composition_update_formatted(&mut self, id: f64, text: &str, formats: &[u32]) {
        let clauses = hayate_core::CompositionClause::from_wire(formats);
        self.tree
            .on_composition_update_formatted(element_id_from_f64(id), text, clauses);
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
                // Renderer-aware procurement (ADR-0043, #332): on the GPU path
                // the monochrome emoji family is upgraded to the COLR build; the
                // bytes still register under `family`, so core routing is intact.
                if let Some(url) = font_url_for_renderer(&family, self.backend.kind()) {
                    let queue = self.font_queue.clone();
                    let failures = self.font_failure_queue.clone();
                    let attempts = self.font_fetch_attempts.clone();
                    let url = url.to_string();
                    wasm_bindgen_futures::spawn_local(async move {
                        match fetch_bytes(&url).await {
                            Ok(bytes) => queue.borrow_mut().push((family, bytes)),
                            Err(e) => {
                                web_sys::console::warn_1(&e);
                                // Back off, then report the failure so core can
                                // re-request (until its retry budget is spent).
                                let n = {
                                    let mut a = attempts.borrow_mut();
                                    let c = a.entry(family.clone()).or_insert(0);
                                    *c += 1;
                                    *c
                                };
                                let delay = FETCH_BACKOFF_BASE_MS
                                    .saturating_mul(1 << (n - 1).min(8))
                                    .min(FETCH_BACKOFF_MAX_MS);
                                backoff_sleep(delay).await;
                                failures.borrow_mut().push(family);
                            }
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

/// `Some(Color)` -> `{r,g,b,a}`, `None` -> `null`.
fn color_to_js(color: Option<Color>) -> JsValue {
    let Some(c) = color else {
        return JsValue::NULL;
    };
    let obj = js_sys::Object::new();
    js_sys::Reflect::set(&obj, &JsValue::from_str("r"), &JsValue::from_f64(c.r)).unwrap();
    js_sys::Reflect::set(&obj, &JsValue::from_str("g"), &JsValue::from_f64(c.g)).unwrap();
    js_sys::Reflect::set(&obj, &JsValue::from_str("b"), &JsValue::from_f64(c.b)).unwrap();
    js_sys::Reflect::set(&obj, &JsValue::from_str("a"), &JsValue::from_f64(c.a)).unwrap();
    obj.into()
}

fn font_style_to_js(value: Option<FontStyleValue>) -> JsValue {
    match value {
        Some(FontStyleValue::Normal) => JsValue::from_str("normal"),
        Some(FontStyleValue::Italic) => JsValue::from_str("italic"),
        Some(FontStyleValue::Oblique) => JsValue::from_str("oblique"),
        None => JsValue::NULL,
    }
}

fn text_decoration_to_js(value: Option<TextDecorationValue>) -> JsValue {
    match value {
        Some(TextDecorationValue::None) => JsValue::from_str("none"),
        Some(TextDecorationValue::Underline) => JsValue::from_str("underline"),
        Some(TextDecorationValue::LineThrough) => JsValue::from_str("line-through"),
        None => JsValue::NULL,
    }
}

fn border_style_to_js(value: BorderStyleValue) -> JsValue {
    match value {
        BorderStyleValue::None => JsValue::from_str("none"),
        BorderStyleValue::Solid => JsValue::from_str("solid"),
        BorderStyleValue::Dashed => JsValue::from_str("dashed"),
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

/// Drive the browser cursor from the cursor resolved under the pointer
/// (ADR-0088 / ADR-0105). Reuses the generated Hayate-CSS → browser-CSS mapper so
/// the `cursor` value list stays single-sourced, and applies it to the canvas
/// element itself — the surface the pointer is over — rather than the whole body.
fn apply_resolved_cursor(canvas: &HtmlCanvasElement, cursor: CursorValue) {
    let mut entries: Vec<(String, String)> = Vec::new();
    crate::generated::style_prop_css_entries(&StyleProp::Cursor(cursor), &mut entries);
    let Some((_, value)) = entries.into_iter().next() else {
        return;
    };
    let _ = canvas.style().set_property("cursor", &value);
}
