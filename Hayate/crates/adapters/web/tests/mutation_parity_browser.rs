//! Canvas↔HTML ミューテーションパリティ（#439 / ADR-0054 RecordingPainter パターンの
//! クロスモード版）。同一の wire op ストリームを Canvas Mode（即時 `TreeSink`）と
//! HTML Mode（遅延 `HtmlDeferred`）の両 `apply_mutations` に流し、観測可能な効果が
//! 一致することを構造テストとして固定する。両モードが1本の生成 decode を共有し、
//! 差分が「即時木適用 vs 遅延 DOM enqueue」だけであることを保証し、Canvas↔HTML の
//! ドリフトをコンパイル時＋実行時の両方で塞ぐ。
//!
//! ヘッドレスブラウザ上で実行する（テスト専用エクスポートは使わず実ホスト API のみ。
//! ADR-0072）。
#![cfg(target_arch = "wasm32")]

use hayate_adapter_web::{HayateElementHtmlRenderer, HayateElementRenderer};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;
use web_sys::{HtmlCanvasElement, HtmlElement};

wasm_bindgen_test_configure!(run_in_browser);

/// wire opcode の判別子（proto/spec/opcodes.json）。テストは実ホスト API
/// `apply_mutations`（ADR-0052）のバッチ経路だけでミューテーションを駆動する。
const OP_SET_ROOT: f64 = 3.0;
const OP_CREATE: f64 = 9.0;
const OP_SET_TEXT_CONTENT: f64 = 12.0;
/// `ElementKind::TextInput` の判別子。
const ELEMENT_KIND_TEXT_INPUT: u32 = 4;

fn make_canvas(size: u32) -> HtmlCanvasElement {
    let document = web_sys::window().unwrap().document().unwrap();
    let canvas: HtmlCanvasElement = document
        .create_element("canvas")
        .unwrap()
        .dyn_into()
        .unwrap();
    canvas.set_width(size);
    canvas.set_height(size);
    document.body().unwrap().append_child(&canvas).unwrap();
    canvas
}

fn make_container() -> HtmlElement {
    let document = web_sys::window().unwrap().document().unwrap();
    let div: HtmlElement = document
        .create_element("div")
        .unwrap()
        .dyn_into()
        .unwrap();
    document.body().unwrap().append_child(&div).unwrap();
    div
}

/// 文字列テーブル `texts` を JS 配列に詰める小さなヘルパ。
fn texts(values: &[&str]) -> js_sys::Array {
    let arr = js_sys::Array::new();
    for v in values {
        arr.push(&JsValue::from_str(v));
    }
    arr
}

// 同一 op ストリーム（CREATE text-input → SET_ROOT → SET_TEXT_CONTENT "hi"）を
// Canvas と HTML の両 apply_mutations へ流すと、編集可能テキスト内容の読み戻しが
// 一致する。1本の生成 decode を両 sink が消費している証左。
#[wasm_bindgen_test]
async fn canvas_and_html_sinks_agree_on_text_content_for_one_op_stream() {
    let canvas = make_canvas(200);
    let mut canvas_renderer = HayateElementRenderer::init(canvas)
        .await
        .expect("canvas renderer init");

    let container = make_container();
    let mut html_renderer =
        HayateElementHtmlRenderer::new(container).expect("html renderer init");

    let ops = [
        OP_CREATE,
        1.0,
        ELEMENT_KIND_TEXT_INPUT as f64,
        OP_SET_ROOT,
        1.0,
        OP_SET_TEXT_CONTENT,
        1.0,
        0.0,
    ];

    canvas_renderer
        .apply_mutations(&ops, &[], texts(&["hi"]))
        .expect("canvas apply_mutations");
    html_renderer
        .apply_mutations(&ops, &[], texts(&["hi"]))
        .expect("html apply_mutations");

    // HTML Mode は唯一のフラッシュ境界 render() で DOM を実体化する（ADR-0030）。
    canvas_renderer.render(0.0).expect("canvas render");
    html_renderer.render(0.0).expect("html render");

    assert_eq!(
        canvas_renderer.element_get_text_content(1.0),
        "hi",
        "Canvas Mode applies SET_TEXT_CONTENT through TreeSink"
    );
    assert_eq!(
        html_renderer.element_get_text_content(1.0),
        canvas_renderer.element_get_text_content(1.0),
        "HtmlDeferred and TreeSink must agree on the effect of the same op stream"
    );
}
