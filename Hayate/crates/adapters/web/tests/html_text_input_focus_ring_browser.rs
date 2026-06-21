//! 回帰テスト: HTML Mode はテキスト入力でブラウザ標準のフォーカスリングを保つ
//! （ADR-0102）。TextInput のインライン `outline` を `none` に強制すると
//! `:focus-visible` のリングが消え、DOM Renderer と挙動が分かれる。
//!
//! `wasm-pack test --headless --firefox` でヘッドレスブラウザ上で実行
//! （WebGPU 無しの `--no-default-features --features backend-null` ビルド）。
//! HTML renderer は `render()` 時に `<input>` を生成する。
#![cfg(target_arch = "wasm32")]

use hayate_adapter_web::HayateElementHtmlRenderer;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;
use web_sys::{HtmlElement, HtmlInputElement};

wasm_bindgen_test_configure!(run_in_browser);

/// `ElementKind::TextInput` の判別子（crates/core/src/element/kind.rs）。
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

    // 最初に生成した要素は自動でルート化されコンテナにマウントされる。
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
