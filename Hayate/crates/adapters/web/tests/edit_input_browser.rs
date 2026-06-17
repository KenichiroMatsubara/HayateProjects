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
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;
use web_sys::HtmlCanvasElement;

wasm_bindgen_test_configure!(run_in_browser);

/// `ElementKind::TextInput` discriminant (crates/core/src/element/kind.rs).
const ELEMENT_KIND_TEXT_INPUT: u32 = 4;
/// `MODIFIER_CTRL` (proto/spec wire contract) — the primary modifier on
/// Win/Linux, which the adapter keymap maps to clipboard / select-all intents.
const MOD_CTRL: u32 = 2;
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

/// Shadow `navigator.clipboard.readText` with a stub that resolves to `text`, so
/// the async Ctrl/Cmd+V read is deterministic in headless automation (where the
/// real clipboard-read permission is denied). This patches the live `Clipboard`
/// object the adapter reads through — no test-only export in the adapter itself.
fn stub_clipboard_read_text(text: &'static str) {
    let clipboard = web_sys::window().unwrap().navigator().clipboard();
    let stub = Closure::wrap(Box::new(move || -> js_sys::Promise {
        js_sys::Promise::resolve(&JsValue::from_str(text))
    }) as Box<dyn FnMut() -> js_sys::Promise>);
    js_sys::Reflect::set(
        clipboard.as_ref(),
        &JsValue::from_str("readText"),
        stub.as_ref().unchecked_ref(),
    )
    .unwrap();
    // Keep the closure alive for the page's lifetime — the adapter calls it later.
    stub.forget();
}

/// Yield to the microtask queue so a `spawn_local`'d future (the clipboard read)
/// gets a chance to run to completion before the test inspects the result.
async fn flush_microtasks() {
    for _ in 0..3 {
        let _ = JsFuture::from(js_sys::Promise::resolve(&JsValue::UNDEFINED)).await;
    }
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

#[wasm_bindgen_test]
async fn enter_in_a_multiline_field_inserts_a_newline_at_the_caret() {
    // #362: a multi-line text-input treats Enter as a newline inserted at the
    // caret, not appended to the end. Drive the caret to the start, then Enter.
    let canvas = make_canvas(200);
    let mut renderer = HayateElementRenderer::init(canvas.clone())
        .await
        .expect("renderer init");

    renderer.element_create(1.0, ELEMENT_KIND_TEXT_INPUT).unwrap();
    renderer
        .element_set_style(
            1.0,
            &[TAG_WIDTH, 200.0, UNIT_PX, TAG_HEIGHT, 200.0, UNIT_PX],
        )
        .unwrap();
    renderer.set_root(1.0);
    renderer.element_set_multiline(1.0, true);
    renderer.element_set_text_content(1.0, "ab");

    renderer.render(0.0).unwrap();
    let rect = canvas.get_bounding_client_rect();
    dispatch_pointer_down(&canvas, rect.left() + 10.0, rect.top() + 10.0);
    renderer.render(16.0).unwrap();
    assert_eq!(renderer.focused_element_id(), 1.0);

    renderer.on_key_down("ArrowLeft", 0); // caret between 'a' and 'b'
    renderer.on_key_down("Enter", 0);
    assert_eq!(
        renderer.element_get_text_content(1.0),
        "a\nb",
        "Enter inserts the newline at the caret, not at the end",
    );
}

#[wasm_bindgen_test]
async fn enter_in_a_single_line_field_does_not_insert_a_newline() {
    // #362: the default (single-line) text-input leaves its text untouched on
    // Enter — the key is the app's submit signal, not a newline.
    let canvas = make_canvas(200);
    let mut renderer = HayateElementRenderer::init(canvas.clone())
        .await
        .expect("renderer init");

    renderer.element_create(1.0, ELEMENT_KIND_TEXT_INPUT).unwrap();
    renderer
        .element_set_style(
            1.0,
            &[TAG_WIDTH, 200.0, UNIT_PX, TAG_HEIGHT, 200.0, UNIT_PX],
        )
        .unwrap();
    renderer.set_root(1.0);
    renderer.element_set_text_content(1.0, "ab");

    renderer.render(0.0).unwrap();
    let rect = canvas.get_bounding_client_rect();
    dispatch_pointer_down(&canvas, rect.left() + 10.0, rect.top() + 10.0);
    renderer.render(16.0).unwrap();
    assert_eq!(renderer.focused_element_id(), 1.0);

    renderer.on_key_down("Enter", 0);
    assert_eq!(
        renderer.element_get_text_content(1.0),
        "ab",
        "a single-line field inserts no newline on Enter",
    );
}

#[wasm_bindgen_test]
async fn home_and_end_jump_to_the_field_boundaries_through_the_canvas_keymap() {
    let canvas = make_canvas(200);
    let mut renderer = HayateElementRenderer::init(canvas.clone())
        .await
        .expect("renderer init");

    renderer.element_create(1.0, ELEMENT_KIND_TEXT_INPUT).unwrap();
    renderer
        .element_set_style(
            1.0,
            &[TAG_WIDTH, 200.0, UNIT_PX, TAG_HEIGHT, 200.0, UNIT_PX],
        )
        .unwrap();
    renderer.set_root(1.0);
    renderer.element_set_text_content(1.0, "hello");

    renderer.render(0.0).unwrap();
    let rect = canvas.get_bounding_client_rect();
    dispatch_pointer_down(&canvas, rect.left() + 10.0, rect.top() + 10.0);
    renderer.render(16.0).unwrap();
    assert_eq!(renderer.focused_element_id(), 1.0);

    // Home maps to a Move/LineBoundary/Backward intent and jumps to the field
    // start; typing then inserts there.
    renderer.on_key_down("Home", 0);
    renderer.on_text_input(1.0, "X");
    assert_eq!(
        renderer.element_get_text_content(1.0),
        "Xhello",
        "Home jumped the caret to the field start"
    );

    // End maps to Move/LineBoundary/Forward and jumps back to the field end.
    renderer.on_key_down("End", 0);
    renderer.on_text_input(1.0, "Z");
    assert_eq!(
        renderer.element_get_text_content(1.0),
        "XhelloZ",
        "End jumped the caret to the field end"
    );
}

#[wasm_bindgen_test]
async fn delete_keys_remove_chars_through_the_canvas_keymap() {
    let canvas = make_canvas(200);
    let mut renderer = HayateElementRenderer::init(canvas.clone())
        .await
        .expect("renderer init");

    renderer.element_create(1.0, ELEMENT_KIND_TEXT_INPUT).unwrap();
    renderer
        .element_set_style(
            1.0,
            &[TAG_WIDTH, 200.0, UNIT_PX, TAG_HEIGHT, 200.0, UNIT_PX],
        )
        .unwrap();
    renderer.set_root(1.0);
    renderer.element_set_text_content(1.0, "hello");

    renderer.render(0.0).unwrap();
    let rect = canvas.get_bounding_client_rect();
    dispatch_pointer_down(&canvas, rect.left() + 10.0, rect.top() + 10.0);
    renderer.render(16.0).unwrap();
    assert_eq!(renderer.focused_element_id(), 1.0, "the pointerdown should focus the input");

    // Drive the caret to the end, then Backspace removes the trailing 'o' — the
    // key crosses the wasm boundary, maps to Delete/Backward, and edits content.
    for _ in 0..10 {
        renderer.on_key_down("ArrowRight", 0);
    }
    renderer.on_key_down("Backspace", 0);
    assert_eq!(
        renderer.element_get_text_content(1.0),
        "hell",
        "Backspace removed the char before the caret"
    );

    // Drive the caret to the start, then Delete removes the leading 'h' — proving
    // forward delete also routes through the keymap.
    for _ in 0..10 {
        renderer.on_key_down("ArrowLeft", 0);
    }
    renderer.on_key_down("Delete", 0);
    assert_eq!(
        renderer.element_get_text_content(1.0),
        "ell",
        "Delete removed the char after the caret"
    );
}

#[wasm_bindgen_test]
async fn ctrl_v_pastes_clipboard_text_through_the_canvas_async_read() {
    // ADR-0097 / #361: the browser clipboard read is async, so Canvas Mode cannot
    // serve it through core's synchronous `Clipboard::read_text`. Ctrl/Cmd+V must
    // instead kick off `navigator.clipboard.readText()` and feed the resolved
    // text back through `element_paste` on the next render. With a stubbed read
    // this whole path is observable: an empty focused field ends up holding the
    // clipboard text.
    let canvas = make_canvas(200);
    let mut renderer = HayateElementRenderer::init(canvas.clone())
        .await
        .expect("renderer init");

    renderer.element_create(1.0, ELEMENT_KIND_TEXT_INPUT).unwrap();
    renderer
        .element_set_style(
            1.0,
            &[TAG_WIDTH, 200.0, UNIT_PX, TAG_HEIGHT, 200.0, UNIT_PX],
        )
        .unwrap();
    renderer.set_root(1.0);

    // Lay out, then focus the (empty) input with a genuine pointerdown.
    renderer.render(0.0).unwrap();
    let rect = canvas.get_bounding_client_rect();
    dispatch_pointer_down(&canvas, rect.left() + 10.0, rect.top() + 10.0);
    renderer.render(16.0).unwrap();
    assert_eq!(renderer.focused_element_id(), 1.0, "pointerdown focuses the input");

    stub_clipboard_read_text("PASTED");

    // Ctrl+V maps (in the adapter keymap) to a Paste intent; the adapter starts
    // the async read instead of going through the synchronous seam.
    renderer.on_key_down("v", MOD_CTRL);
    // Let the spawned read resolve, then render drains it into the field.
    flush_microtasks().await;
    renderer.render(32.0).unwrap();

    assert_eq!(
        renderer.element_get_text_content(1.0),
        "PASTED",
        "Ctrl+V pasted the clipboard text into the focused field via the async read",
    );
}
