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

/// Generated event-kind discriminant for `HoverEnter` (proto/spec/event_kinds.json).
const HOVER_ENTER_KIND: f64 = 10.0;
/// `ElementKind::View` discriminant (crates/core/src/element/kind.rs).
const ELEMENT_KIND_VIEW: u32 = 0;

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

/// Dispatch a genuine `pointermove` PointerEvent at viewport `(client_x, client_y)`.
fn dispatch_pointer_move(canvas: &HtmlCanvasElement, client_x: f64, client_y: f64) {
    let window = web_sys::window().unwrap();
    let ctor = js_sys::Reflect::get(&window, &JsValue::from_str("PointerEvent")).unwrap();
    let ctor: js_sys::Function = ctor.dyn_into().unwrap();

    let init = js_sys::Object::new();
    js_sys::Reflect::set(&init, &"clientX".into(), &JsValue::from_f64(client_x)).unwrap();
    js_sys::Reflect::set(&init, &"clientY".into(), &JsValue::from_f64(client_y)).unwrap();
    js_sys::Reflect::set(&init, &"bubbles".into(), &JsValue::TRUE).unwrap();
    js_sys::Reflect::set(&init, &"pointerType".into(), &"mouse".into()).unwrap();

    let args = js_sys::Array::of2(&JsValue::from_str("pointermove"), &init);
    let event: web_sys::Event = js_sys::Reflect::construct(&ctor, &args)
        .unwrap()
        .dyn_into()
        .unwrap();
    canvas.dispatch_event(&event).unwrap();
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
    let delivered_hover_enter = (0..rows.length()).any(|i| {
        let row = js_sys::Array::from(&rows.get(i));
        let lid = row.get(0).as_f64();
        let kind = row.get(1).as_f64();
        lid == Some(listener_id) && kind == Some(HOVER_ENTER_KIND)
    });

    assert!(
        delivered_hover_enter,
        "expected a HoverEnter delivery for the self-wired pointermove"
    );
}
