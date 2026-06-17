//! End-to-end verification of the self-wired canvas pointer path (ADR-0092, #211).
//!
//! Runs in a headless browser via `wasm-pack test --headless --firefox`, built
//! with `--no-default-features --features backend-null` (no WebGPU / EditContext).
//! A real `pointermove` is dispatched on the canvas; the adapter's self-attached
//! listener must transform + buffer it, `render()` drains it into Core, and
//! `poll_events()` must surface a `HoverEnter` delivery — exercising the whole
//! DOM-event → adapter → Core → poll chain without any test-only export (ADR-0072).
#![cfg(target_arch = "wasm32")]

use hayate_adapter_web::HayateElementRenderer;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;
use web_sys::HtmlCanvasElement;

wasm_bindgen_test_configure!(run_in_browser);

/// `PointerKind` wire discriminants (crates/core/src/element/pointer.rs).
const POINTER_KIND_MOUSE: u32 = 0;
const POINTER_KIND_TOUCH: u32 = 1;
/// Generated event-kind discriminant for `HoverEnter` (proto/spec/event_kinds.json).
const HOVER_ENTER_KIND: f64 = 10.0;
/// Generated event-kind discriminant for `HoverLeave` (proto/spec/event_kinds.json).
const HOVER_LEAVE_KIND: f64 = 11.0;
/// Generated event-kind discriminant for `Scroll` (proto/spec/event_kinds.json).
const SCROLL_KIND: f64 = 7.0;
/// `ElementKind::View` discriminant (crates/core/src/element/kind.rs).
const ELEMENT_KIND_VIEW: u32 = 0;
/// `ElementKind::ScrollView` discriminant (crates/core/src/element/kind.rs).
const ELEMENT_KIND_SCROLLVIEW: u32 = 5;
/// style_packet tags: width=5, height=6; unit 0 = Px (crates/adapters/web/src/style_packet.rs).
const TAG_WIDTH: f32 = 5.0;
const TAG_HEIGHT: f32 = 6.0;

fn make_canvas(size: u32) -> HtmlCanvasElement {
    let document = web_sys::window().unwrap().document().unwrap();
    let canvas: HtmlCanvasElement = document
        .create_element("canvas")
        .unwrap()
        .dyn_into()
        .unwrap();
    canvas.set_width(size);
    canvas.set_height(size);
    let style = canvas.style();
    style.set_property("width", &format!("{size}px")).unwrap();
    style.set_property("height", &format!("{size}px")).unwrap();
    document.body().unwrap().append_child(&canvas).unwrap();
    canvas
}

/// Dispatch a genuine `PointerEvent` of `kind` at viewport `(client_x, client_y)`.
fn dispatch_pointer_event(canvas: &HtmlCanvasElement, kind: &str, client_x: f64, client_y: f64) {
    let window = web_sys::window().unwrap();
    let ctor = js_sys::Reflect::get(&window, &JsValue::from_str("PointerEvent")).unwrap();
    let ctor: js_sys::Function = ctor.dyn_into().unwrap();

    let init = js_sys::Object::new();
    js_sys::Reflect::set(&init, &"clientX".into(), &JsValue::from_f64(client_x)).unwrap();
    js_sys::Reflect::set(&init, &"clientY".into(), &JsValue::from_f64(client_y)).unwrap();
    js_sys::Reflect::set(&init, &"bubbles".into(), &JsValue::TRUE).unwrap();
    js_sys::Reflect::set(&init, &"pointerType".into(), &"mouse".into()).unwrap();
    // Real mouse events are always primary; the adapter ignores non-primary
    // pointers (#350), so the synthetic event must say so too.
    js_sys::Reflect::set(&init, &"isPrimary".into(), &JsValue::TRUE).unwrap();

    let args = js_sys::Array::of2(&JsValue::from_str(kind), &init);
    let event: web_sys::Event = js_sys::Reflect::construct(&ctor, &args)
        .unwrap()
        .dyn_into()
        .unwrap();
    canvas.dispatch_event(&event).unwrap();
}

fn dispatch_pointer_move(canvas: &HtmlCanvasElement, client_x: f64, client_y: f64) {
    dispatch_pointer_event(canvas, "pointermove", client_x, client_y);
}

/// Dispatch a primary `touch` PointerEvent (the drag→scroll path, #350). Sets
/// `pointerType: "touch"`, `isPrimary: true` and a `pointerId` so the adapter's
/// primary-pointer filter and scroll gesture engage.
fn dispatch_touch_event(canvas: &HtmlCanvasElement, kind: &str, client_x: f64, client_y: f64) {
    let window = web_sys::window().unwrap();
    let ctor = js_sys::Reflect::get(&window, &JsValue::from_str("PointerEvent")).unwrap();
    let ctor: js_sys::Function = ctor.dyn_into().unwrap();

    let init = js_sys::Object::new();
    js_sys::Reflect::set(&init, &"clientX".into(), &JsValue::from_f64(client_x)).unwrap();
    js_sys::Reflect::set(&init, &"clientY".into(), &JsValue::from_f64(client_y)).unwrap();
    js_sys::Reflect::set(&init, &"bubbles".into(), &JsValue::TRUE).unwrap();
    js_sys::Reflect::set(&init, &"pointerType".into(), &"touch".into()).unwrap();
    js_sys::Reflect::set(&init, &"isPrimary".into(), &JsValue::TRUE).unwrap();
    js_sys::Reflect::set(&init, &"pointerId".into(), &JsValue::from_f64(1.0)).unwrap();

    let args = js_sys::Array::of2(&JsValue::from_str(kind), &init);
    let event: web_sys::Event = js_sys::Reflect::construct(&ctor, &args)
        .unwrap()
        .dyn_into()
        .unwrap();
    canvas.dispatch_event(&event).unwrap();
}

/// True if `rows` (delivery `[listener_id, kind, ...]` tuples from `poll_events`)
/// contains a delivery to `listener_id` of the given event `kind`.
fn has_delivery(rows: &js_sys::Array, listener_id: f64, kind: f64) -> bool {
    (0..rows.length()).any(|i| {
        let row = js_sys::Array::from(&rows.get(i));
        row.get(0).as_f64() == Some(listener_id) && row.get(1).as_f64() == Some(kind)
    })
}

#[wasm_bindgen_test]
async fn dispatched_pointermove_delivers_hover_enter() {
    let canvas = make_canvas(200);
    let mut renderer = HayateElementRenderer::init(canvas.clone())
        .await
        .expect("renderer init");

    // A single root View filling the surface, with a HoverEnter listener.
    renderer.element_create(1.0, ELEMENT_KIND_VIEW).unwrap();
    // TAG_WIDTH=5 / TAG_HEIGHT=6, value 200, unit 0 (Px).
    renderer
        .element_set_style(1.0, &[5.0, 200.0, 0.0, 6.0, 200.0, 0.0])
        .unwrap();
    renderer.set_root(1.0);
    let listener_id = renderer
        .register_listener(1.0, HOVER_ENTER_KIND as u32)
        .unwrap();

    // First frame lays out the tree so hit-testing has bounds.
    renderer.render(0.0).unwrap();

    // Move the pointer a few CSS px into the surface — well inside the 200px
    // root for any device-pixel-ratio the headless browser reports.
    let rect = canvas.get_bounding_client_rect();
    dispatch_pointer_move(&canvas, rect.left() + 10.0, rect.top() + 10.0);

    // Next frame drains the buffered move into Core, producing HoverEnter.
    renderer.render(16.0).unwrap();

    let rows = renderer.poll_events();
    assert!(
        has_delivery(&rows, listener_id, HOVER_ENTER_KIND),
        "expected a HoverEnter delivery for the self-wired pointermove"
    );
}

#[wasm_bindgen_test]
async fn touch_drag_scrolls_the_scroll_view_and_fires_scroll() {
    // A ScrollView the size of the surface with a child taller than it, so
    // there is room to scroll vertically (content 600 vs viewport 200).
    let canvas = make_canvas(200);
    let mut renderer = HayateElementRenderer::init(canvas.clone())
        .await
        .expect("renderer init");

    renderer.element_create(1.0, ELEMENT_KIND_SCROLLVIEW).unwrap();
    renderer
        .element_set_style(1.0, &[TAG_WIDTH, 200.0, 0.0, TAG_HEIGHT, 200.0, 0.0])
        .unwrap();
    renderer.element_create(2.0, ELEMENT_KIND_VIEW).unwrap();
    renderer
        .element_set_style(2.0, &[TAG_WIDTH, 200.0, 0.0, TAG_HEIGHT, 600.0, 0.0])
        .unwrap();
    renderer.element_append_child(1.0, 2.0);
    renderer.set_root(1.0);
    let scroll_listener = renderer.register_listener(1.0, SCROLL_KIND as u32).unwrap();

    // Lay out so hit-testing and content-size have geometry.
    renderer.render(0.0).unwrap();

    let rect = canvas.get_bounding_client_rect();
    let (ox, oy) = (rect.left(), rect.top());
    // Press, drag upward past the slop, then keep dragging: content follows the
    // finger so the vertical offset grows. Two moves are needed — the first
    // consumes the slop dead-zone (takeover), the second applies the delta.
    dispatch_touch_event(&canvas, "pointerdown", ox + 100.0, oy + 150.0);
    dispatch_touch_event(&canvas, "pointermove", ox + 100.0, oy + 100.0); // crosses slop
    dispatch_touch_event(&canvas, "pointermove", ox + 100.0, oy + 30.0); // scroll by ~70
    dispatch_touch_event(&canvas, "pointerup", ox + 100.0, oy + 30.0);

    // One frame drains the whole gesture in arrival order.
    renderer.render(16.0).unwrap();

    let offset = renderer.element_get_scroll_offset(1.0);
    assert!(
        offset[1] > 0.0,
        "touch drag should scroll the view down (offset.y = {})",
        offset[1]
    );
    assert!(
        has_delivery(&renderer.poll_events(), scroll_listener, SCROLL_KIND),
        "touch-driven scroll must fire Event::Scroll"
    );
}

/// Build a 200×200 ScrollView with a 200×600 child so there is a 400px vertical
/// scroll range, register a `Scroll` listener on it, lay out once, and return the
/// renderer plus the canvas' client origin and the listener id. Shared by the
/// momentum e2e tests (#351).
async fn scrollable_renderer(canvas: &HtmlCanvasElement) -> (HayateElementRenderer, f64, f64, f64) {
    let mut renderer = HayateElementRenderer::init(canvas.clone())
        .await
        .expect("renderer init");

    renderer.element_create(1.0, ELEMENT_KIND_SCROLLVIEW).unwrap();
    renderer
        .element_set_style(1.0, &[TAG_WIDTH, 200.0, 0.0, TAG_HEIGHT, 200.0, 0.0])
        .unwrap();
    renderer.element_create(2.0, ELEMENT_KIND_VIEW).unwrap();
    renderer
        .element_set_style(2.0, &[TAG_WIDTH, 200.0, 0.0, TAG_HEIGHT, 600.0, 0.0])
        .unwrap();
    renderer.element_append_child(1.0, 2.0);
    renderer.set_root(1.0);
    let scroll_listener = renderer.register_listener(1.0, SCROLL_KIND as u32).unwrap();

    renderer.render(0.0).unwrap();

    let rect = canvas.get_bounding_client_rect();
    (renderer, rect.left(), rect.top(), scroll_listener)
}

fn scroll_offset_y(renderer: &HayateElementRenderer) -> f32 {
    renderer.element_get_scroll_offset(1.0)[1]
}

#[wasm_bindgen_test]
async fn flick_coasts_then_bounces_at_the_edge_and_springs_back() {
    // Drive a real flick: one move per rAF frame so the velocity tracker sees
    // distinct timestamps, then let the released fling coast on its own frames.
    let canvas = make_canvas(200);
    let (mut renderer, ox, oy, scroll_listener) = scrollable_renderer(&canvas).await;

    // Finger climbs 150 → 60 across consecutive frames (each move > slop apart).
    dispatch_touch_event(&canvas, "pointerdown", ox + 100.0, oy + 150.0);
    renderer.render(16.0).unwrap();
    dispatch_touch_event(&canvas, "pointermove", ox + 100.0, oy + 120.0); // crosses slop
    renderer.render(32.0).unwrap();
    dispatch_touch_event(&canvas, "pointermove", ox + 100.0, oy + 90.0);
    renderer.render(48.0).unwrap();
    dispatch_touch_event(&canvas, "pointermove", ox + 100.0, oy + 60.0);
    renderer.render(64.0).unwrap();
    dispatch_touch_event(&canvas, "pointerup", ox + 100.0, oy + 60.0);
    let _ = renderer.poll_events(); // discard drag-phase Scroll deliveries

    // Release frame launches momentum from the sampled fling.
    renderer.render(80.0).unwrap();
    let offset_at_release = scroll_offset_y(&renderer);
    let _ = renderer.poll_events();

    // A pure momentum frame (no pointer input) must keep scrolling and still fire
    // Event::Scroll, exactly like a finger drag does.
    renderer.render(96.0).unwrap();
    let offset_after_momentum = scroll_offset_y(&renderer);
    assert!(
        offset_after_momentum > offset_at_release,
        "momentum should keep scrolling after the finger lifts ({offset_after_momentum} !> {offset_at_release})"
    );
    assert!(
        has_delivery(&renderer.poll_events(), scroll_listener, SCROLL_KIND),
        "momentum scrolling must fire Event::Scroll like a finger drag"
    );

    // Coast, bounce, settle: this strong fling overruns the 400px range, bounces
    // past the bottom edge into overscroll, then spring-back returns it to rest at
    // the edge. Track the peak offset across the whole animation.
    let mut peak = offset_after_momentum;
    let mut t = 112.0;
    for _ in 0..400 {
        renderer.render(t).unwrap();
        let _ = renderer.poll_events();
        peak = peak.max(scroll_offset_y(&renderer));
        t += 16.0;
    }
    assert!(
        peak > 400.0,
        "inertia reaching the edge must bounce past it into overscroll (peak {peak})"
    );
    let final_offset = scroll_offset_y(&renderer);
    assert!(
        (final_offset - 400.0).abs() < 1.0,
        "after the bounce, spring-back settles at the bottom edge (max 400, got {final_offset})"
    );
}

#[wasm_bindgen_test]
async fn dragging_past_an_edge_overscrolls_with_resistance_then_springs_back() {
    // At the top edge, dragging the content further down pulls it into overscroll
    // (negative offset) with rubber-band resistance; releasing springs it home.
    let canvas = make_canvas(200);
    let (mut renderer, ox, oy, _scroll_listener) = scrollable_renderer(&canvas).await;

    // Press near the top, cross the slop (takeover, no delta), then drag the
    // finger ~100px further DOWN — content follows below its top edge, i.e. the
    // vertical offset goes negative (overscroll past the top).
    dispatch_touch_event(&canvas, "pointerdown", ox + 100.0, oy + 40.0);
    renderer.render(16.0).unwrap();
    dispatch_touch_event(&canvas, "pointermove", ox + 100.0, oy + 60.0); // crosses slop
    renderer.render(32.0).unwrap();
    dispatch_touch_event(&canvas, "pointermove", ox + 100.0, oy + 160.0); // 100px further down
    renderer.render(48.0).unwrap();

    let overscrolled = scroll_offset_y(&renderer);
    assert!(
        overscrolled < 0.0,
        "dragging past the top edge must overscroll (offset.y = {overscrolled})"
    );
    assert!(
        overscrolled > -100.0,
        "overscroll must resist — the content lags the 100px finger pull \
         (offset.y = {overscrolled})"
    );

    // Release in overscroll: spring-back must ease the offset home to the edge (0).
    dispatch_touch_event(&canvas, "pointerup", ox + 100.0, oy + 160.0);
    let mut t = 64.0;
    for _ in 0..200 {
        renderer.render(t).unwrap();
        let _ = renderer.poll_events();
        t += 16.0;
    }
    let settled = scroll_offset_y(&renderer);
    assert!(
        settled.abs() < 1.0,
        "spring-back must return the overscrolled edge home to 0 (offset.y = {settled})"
    );
}

#[wasm_bindgen_test]
async fn a_press_during_momentum_interrupts_it_so_the_content_is_grabbable() {
    let canvas = make_canvas(200);
    let (mut renderer, ox, oy, _scroll_listener) = scrollable_renderer(&canvas).await;

    // Flick upward to get a fling coasting.
    dispatch_touch_event(&canvas, "pointerdown", ox + 100.0, oy + 150.0);
    renderer.render(16.0).unwrap();
    dispatch_touch_event(&canvas, "pointermove", ox + 100.0, oy + 120.0);
    renderer.render(32.0).unwrap();
    dispatch_touch_event(&canvas, "pointermove", ox + 100.0, oy + 90.0);
    renderer.render(48.0).unwrap();
    dispatch_touch_event(&canvas, "pointerup", ox + 100.0, oy + 90.0);
    let _ = renderer.poll_events();

    renderer.render(64.0).unwrap(); // launch
    let offset_at_release = scroll_offset_y(&renderer);
    renderer.render(80.0).unwrap(); // coast one frame
    let offset_coasting = scroll_offset_y(&renderer);
    assert!(
        offset_coasting > offset_at_release,
        "precondition: momentum must be coasting before the interrupting press",
    );

    // Press again mid-coast: the down must interrupt the fling. The drain
    // processes the press (momentum → None) before the frame's momentum step, so
    // the offset stops dead under the finger.
    dispatch_touch_event(&canvas, "pointerdown", ox + 100.0, oy + 100.0);
    renderer.render(96.0).unwrap();
    let frozen = scroll_offset_y(&renderer);

    // Subsequent frames with no further input must not move — the fling is gone.
    renderer.render(112.0).unwrap();
    renderer.render(128.0).unwrap();
    let after = scroll_offset_y(&renderer);
    assert_eq!(
        after, frozen,
        "a press during momentum must interrupt it so the content stays grabbable",
    );
}

#[wasm_bindgen_test]
async fn pointer_type_is_forwarded_to_core_as_pointer_kind() {
    // The Platform Adapter must map `PointerEvent.pointerType` to a core
    // `PointerKind` and forward it through the self-wired pointer path, so Core
    // retains `last_pointer_kind` per interaction (#357). Observed end-to-end via
    // the renderer accessor — no test-only export (ADR-0072).
    let canvas = make_canvas(200);
    let mut renderer = HayateElementRenderer::init(canvas.clone())
        .await
        .expect("renderer init");

    renderer.element_create(1.0, ELEMENT_KIND_VIEW).unwrap();
    renderer
        .element_set_style(1.0, &[TAG_WIDTH, 200.0, 0.0, TAG_HEIGHT, 200.0, 0.0])
        .unwrap();
    renderer.set_root(1.0);
    renderer.render(0.0).unwrap();

    // Before any pointer event the kind defaults to mouse.
    assert_eq!(renderer.last_pointer_kind(), POINTER_KIND_MOUSE);

    let rect = canvas.get_bounding_client_rect();
    let (ox, oy) = (rect.left(), rect.top());

    // A genuine touch press forwards PointerKind::Touch to Core.
    dispatch_touch_event(&canvas, "pointerdown", ox + 50.0, oy + 50.0);
    renderer.render(16.0).unwrap();
    assert_eq!(
        renderer.last_pointer_kind(),
        POINTER_KIND_TOUCH,
        "a touch pointerdown must set Core's last_pointer_kind to Touch"
    );

    // A mouse move then follows the live device (hybrid follow-through, not
    // latched at the first interaction).
    dispatch_pointer_move(&canvas, ox + 80.0, oy + 80.0);
    renderer.render(32.0).unwrap();
    assert_eq!(
        renderer.last_pointer_kind(),
        POINTER_KIND_MOUSE,
        "a mouse pointermove must update last_pointer_kind back to Mouse"
    );
}

#[wasm_bindgen_test]
async fn dispatched_pointerleave_delivers_hover_leave() {
    let canvas = make_canvas(200);
    let mut renderer = HayateElementRenderer::init(canvas.clone())
        .await
        .expect("renderer init");

    renderer.element_create(1.0, ELEMENT_KIND_VIEW).unwrap();
    renderer
        .element_set_style(1.0, &[5.0, 200.0, 0.0, 6.0, 200.0, 0.0])
        .unwrap();
    renderer.set_root(1.0);
    let enter_listener = renderer
        .register_listener(1.0, HOVER_ENTER_KIND as u32)
        .unwrap();
    let leave_listener = renderer
        .register_listener(1.0, HOVER_LEAVE_KIND as u32)
        .unwrap();

    renderer.render(0.0).unwrap();

    // Move into the surface: the self-wired `pointermove` produces HoverEnter.
    let rect = canvas.get_bounding_client_rect();
    dispatch_pointer_move(&canvas, rect.left() + 10.0, rect.top() + 10.0);
    renderer.render(16.0).unwrap();
    assert!(
        has_delivery(&renderer.poll_events(), enter_listener, HOVER_ENTER_KIND),
        "precondition: pointermove should HoverEnter the root"
    );

    // Leave the surface: the self-wired `pointerleave` must clear hover and
    // deliver HoverLeave for the previously-hovered root.
    dispatch_pointer_event(&canvas, "pointerleave", rect.left() + 10.0, rect.top() + 10.0);
    renderer.render(32.0).unwrap();
    assert!(
        has_delivery(&renderer.poll_events(), leave_listener, HOVER_LEAVE_KIND),
        "expected a HoverLeave delivery for the self-wired pointerleave"
    );
}
