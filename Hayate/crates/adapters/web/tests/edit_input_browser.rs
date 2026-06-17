//! End-to-end verification of the Canvas keymap → EditIntent path (ADR-0103).
//!
//! Runs in a headless browser via `wasm-pack test --headless --firefox`, built
//! with `--no-default-features --features backend-null`. A real `pointerdown`
//! focuses a laid-out text-input, then `on_key_down` arrow presses cross the
//! wasm boundary, are mapped to an `EditIntent` by the adapter keymap, and drive
//! core's `apply_edit_intent` — observable because a following `on_text_input`
//! inserts at the moved caret. No test-only export (ADR-0072): only the real
//! host API (`element_set_text_content`, `on_key_down`, `on_text_input`,
//! `element_get_text_content`, `focused_element_id`) is used.
#![cfg(target_arch = "wasm32")]

use hayate_adapter_web::HayateElementRenderer;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;
use web_sys::HtmlCanvasElement;

wasm_bindgen_test_configure!(run_in_browser);

/// `ElementKind::TextInput` discriminant (crates/core/src/element/kind.rs).
const ELEMENT_KIND_TEXT_INPUT: u32 = 4;
/// Style-packet tags (proto/spec/style_tags.json): WIDTH=5, HEIGHT=6, unit Px=0.
const TAG_WIDTH: f32 = 5.0;
const TAG_HEIGHT: f32 = 6.0;
const UNIT_PX: f32 = 0.0;

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

fn dispatch_pointer_down(canvas: &HtmlCanvasElement, client_x: f64, client_y: f64) {
    let window = web_sys::window().unwrap();
    let ctor = js_sys::Reflect::get(&window, &JsValue::from_str("PointerEvent")).unwrap();
    let ctor: js_sys::Function = ctor.dyn_into().unwrap();

    let init = js_sys::Object::new();
    js_sys::Reflect::set(&init, &"clientX".into(), &JsValue::from_f64(client_x)).unwrap();
    js_sys::Reflect::set(&init, &"clientY".into(), &JsValue::from_f64(client_y)).unwrap();
    js_sys::Reflect::set(&init, &"bubbles".into(), &JsValue::TRUE).unwrap();
    js_sys::Reflect::set(&init, &"pointerType".into(), &"mouse".into()).unwrap();

    let args = js_sys::Array::of2(&JsValue::from_str("pointerdown"), &init);
    let event: web_sys::Event = js_sys::Reflect::construct(&ctor, &args)
        .unwrap()
        .dyn_into()
        .unwrap();
    canvas.dispatch_event(&event).unwrap();
}

#[wasm_bindgen_test]
async fn arrow_key_moves_the_caret_through_the_canvas_keymap() {
    let canvas = make_canvas(200);
    let mut renderer = HayateElementRenderer::init(canvas.clone())
        .await
        .expect("renderer init");

    // A single text-input filling the surface, carrying "hello" (caret at end).
    renderer.element_create(1.0, ELEMENT_KIND_TEXT_INPUT).unwrap();
    renderer
        .element_set_style(
            1.0,
            &[TAG_WIDTH, 200.0, UNIT_PX, TAG_HEIGHT, 200.0, UNIT_PX],
        )
        .unwrap();
    renderer.set_root(1.0);
    renderer.element_set_text_content(1.0, "hello");

    // Lay out, then focus the input with a genuine pointerdown drained on render.
    renderer.render(0.0).unwrap();
    let rect = canvas.get_bounding_client_rect();
    dispatch_pointer_down(&canvas, rect.left() + 10.0, rect.top() + 10.0);
    renderer.render(16.0).unwrap();
    assert_eq!(
        renderer.focused_element_id(),
        1.0,
        "the pointerdown should focus the text-input"
    );

    // Drive the caret to the very start: each bare ArrowLeft must cross the wasm
    // boundary, map to a Move/Grapheme/Backward intent, and step the caret left
    // (clamping at 0). Then typing inserts at the caret.
    for _ in 0..10 {
        renderer.on_key_down("ArrowLeft", 0);
    }
    renderer.on_text_input(1.0, "X");
    assert_eq!(
        renderer.element_get_text_content(1.0),
        "Xhello",
        "ArrowLeft moved the caret to the start, so typing inserts there"
    );

    // Driving back to the end and typing inserts at the tail, proving ArrowRight
    // also routes through the keymap.
    for _ in 0..10 {
        renderer.on_key_down("ArrowRight", 0);
    }
    renderer.on_text_input(1.0, "Z");
    assert_eq!(
        renderer.element_get_text_content(1.0),
        "XhelloZ",
        "ArrowRight moved the caret to the end before the second insert"
    );
}
