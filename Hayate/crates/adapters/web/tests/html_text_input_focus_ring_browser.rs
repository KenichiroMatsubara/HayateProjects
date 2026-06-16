//! Regression: HTML Mode must keep the browser's native focus ring on text
//! inputs (#335, ADR-0102). The TextInput baseline used to set `outline: none`,
//! suppressing the `:focus-visible` ring and diverging from the DOM Renderer.
//!
//! Runs in a headless browser via `wasm-pack test --headless --firefox`, built
//! with `--no-default-features --features backend-null` (no WebGPU). The HTML
//! renderer materialises the `<input>` on `render()`; its inline `outline` must
//! no longer be forced to `none`.
#![cfg(target_arch = "wasm32")]

use hayate_adapter_web::HayateElementHtmlRenderer;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;
use web_sys::{HtmlElement, HtmlInputElement};

wasm_bindgen_test_configure!(run_in_browser);

/// `ElementKind::TextInput` discriminant (crates/core/src/element/kind.rs).
const ELEMENT_KIND_TEXT_INPUT: u32 = 4;

fn make_container() -> HtmlElement {
    let document = web_sys::window().unwrap().document().unwrap();
    let container: HtmlElement = document
        .create_element("div")
        .unwrap()
        .dyn_into()
        .unwrap();
    document.body().unwrap().append_child(&container).unwrap();
    container
}

#[wasm_bindgen_test]
fn text_input_baseline_does_not_suppress_outline() {
    let container = make_container();
    let mut renderer = HayateElementHtmlRenderer::new(container.clone()).unwrap();

    // The first created element auto-roots and mounts onto the container.
    renderer.element_create(1.0, ELEMENT_KIND_TEXT_INPUT).unwrap();
    renderer.render(0.0).unwrap();

    let input: HtmlInputElement = container
        .query_selector("input")
        .unwrap()
        .expect("text input should be materialised on render")
        .dyn_into()
        .unwrap();

    let outline = input.style().get_property_value("outline").unwrap();
    assert_ne!(
        outline, "none",
        "TextInput baseline must not force `outline: none` — the native focus ring must show"
    );
}
