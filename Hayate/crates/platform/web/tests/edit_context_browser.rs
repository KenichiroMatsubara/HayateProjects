//! Canvas IME（EditContext）経路の preedit / commit / bounds 振る舞いの E2E 検証（ADR-0069）。
//!
//! ヘッドレスブラウザ上で実行する。アダプタが自前で配線する EditContext（`hayate-adapter-web`
//! 内で完結）に対し、core の IME シーム（`on_composition_*`）を実ホスト API 経由で駆動し、
//! preedit がフォーカス中 text-input に表示され、`compositionend` で確定し、フォーカスが
//! IME 候補窓 bounds を生むことを確認する。テスト専用エクスポートは使わない（ADR-0072）。
//!
//! EditContext の DOM 配線そのもの（`textupdate` → reflect → バッファ → ドレイン）は
//! `pointer_input` の web-sys 配線と同様「薄くテスト対象外」で、UTF-16→UTF-8 変換と候補窓 rect は
//! `edit_context.rs` の純粋ユニットが、可視性ゲート（#392）は `ime_bridge.rs` ユニットが担う。
//! ここではアダプタの公開 IME シームを実ブラウザで通し、編集の意味（preedit/commit/bounds）が
//! 保存されていることを契約として固定する。
#![cfg(target_arch = "wasm32")]

use hayate_adapter_web::HayateElementRenderer;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;
use web_sys::HtmlCanvasElement;

wasm_bindgen_test_configure!(run_in_browser);

/// `ElementKind::TextInput` の判別値。
const ELEMENT_KIND_TEXT_INPUT: u32 = 4;
/// スタイルパケットのタグ: WIDTH=5, HEIGHT=6, 単位 Px=0。
const TAG_WIDTH: f32 = 5.0;
const TAG_HEIGHT: f32 = 6.0;
const UNIT_PX: f32 = 0.0;
/// wire opcode 判別子（proto/spec/opcodes.json）。実ホスト API `apply_mutations`（ADR-0052）の
/// バッチ経路だけでミューテーションを駆動する（ADR-0072）。
const OP_SET_STYLE: f64 = 4.0;
const OP_SET_TEXT_CONTENT: f64 = 12.0;

fn apply_style(r: &mut HayateElementRenderer, id: f64, packed: &[f32]) {
    let ops = [OP_SET_STYLE, id, 0.0, packed.len() as f64];
    r.apply_mutations(&ops, packed, js_sys::Array::new()).unwrap();
}

fn apply_text_content(r: &mut HayateElementRenderer, id: f64, text: &str) {
    let texts = js_sys::Array::new();
    texts.push(&JsValue::from_str(text));
    let ops = [OP_SET_TEXT_CONTENT, id, 0.0];
    r.apply_mutations(&ops, &[], texts).unwrap();
}

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
    js_sys::Reflect::set(&init, &"isPrimary".into(), &JsValue::TRUE).unwrap();

    let args = js_sys::Array::of2(&JsValue::from_str("pointerdown"), &init);
    let event: web_sys::Event = js_sys::Reflect::construct(&ctor, &args)
        .unwrap()
        .dyn_into()
        .unwrap();
    canvas.dispatch_event(&event).unwrap();
}

/// レイアウト済みの全面 text-input をフォーカスしたレンダラを用意する。
async fn focused_text_input() -> (HayateElementRenderer, HtmlCanvasElement) {
    let canvas = make_canvas(200);
    let mut renderer = HayateElementRenderer::init(canvas.clone(), None)
        .await
        .expect("renderer init");

    renderer.element_create(1.0, ELEMENT_KIND_TEXT_INPUT).unwrap();
    apply_style(
        &mut renderer,
        1.0,
        &[TAG_WIDTH, 200.0, UNIT_PX, TAG_HEIGHT, 200.0, UNIT_PX],
    );
    renderer.set_root(1.0);

    renderer.render(0.0).unwrap();
    let rect = canvas.get_bounding_client_rect();
    dispatch_pointer_down(&canvas, rect.left() + 10.0, rect.top() + 10.0);
    renderer.render(16.0).unwrap();
    assert_eq!(
        renderer.focused_element_id(),
        1.0,
        "the pointerdown should focus the text-input"
    );
    (renderer, canvas)
}

#[wasm_bindgen_test]
async fn preedit_shows_while_composing_then_commits_on_composition_end() {
    // 日本語 IME の最小サイクル: 変換開始 → preedit 表示 → 確定。アダプタの公開 IME シームを
    // 通し、preedit がフォーカス中フィールドに見え、`compositionend` が確定することを固定する。
    let (mut renderer, _canvas) = focused_text_input().await;

    renderer.on_composition_start(1.0, "");
    renderer.on_composition_update(1.0, "ねこ");
    // preedit は確定済み内容と結合して表示される（content "" + preedit "ねこ"）。
    assert_eq!(
        renderer.element_get_text_content(1.0),
        "ねこ",
        "preedit must be visible in the focused field while composing"
    );

    renderer.on_composition_end(1.0, "猫");
    assert_eq!(
        renderer.element_get_text_content(1.0),
        "猫",
        "compositionend commits the conversion result and clears the preedit"
    );
}

#[wasm_bindgen_test]
async fn formatted_preedit_clauses_underline_without_changing_the_committed_text() {
    // `textformatupdate` の文節フォーマット範囲（ADR-0102）を伴う preedit 更新は、Canvas が
    // 文節下線を描くために節範囲を運ぶが、確定までは内容を変えない。確定済みテキストへ追記しても
    // preedit は内容と結合表示される。
    let (mut renderer, _canvas) = focused_text_input().await;
    apply_text_content(&mut renderer, 1.0, "あ");
    renderer.on_text_input(1.0, "");

    renderer.on_composition_start(1.0, "");
    // フラットな `[start, end, weight, …]`（バイトオフセット、weight=1 は太線=アクティブ節）。
    renderer.on_composition_update_formatted(1.0, "ねこ", &[0, 3, 1, 3, 6, 0]);
    assert_eq!(
        renderer.element_get_text_content(1.0),
        "あねこ",
        "the formatted preedit appends to the committed text for display"
    );

    renderer.on_composition_end(1.0, "猫");
    assert_eq!(
        renderer.element_get_text_content(1.0),
        "あ猫",
        "commit appends the conversion result after the existing content"
    );
}

#[wasm_bindgen_test]
async fn focused_text_input_produces_a_non_degenerate_ime_caret_bound() {
    // フォーカス中の text-input は IME 候補窓を置くためのキャレット文字境界を生む（ADR-0069）。
    // `drive_ime` がレイアウト後に bounds を同期し、`ime_character_bounds` が露出する。
    let (mut renderer, _canvas) = focused_text_input().await;
    apply_text_content(&mut renderer, 1.0, "hi");
    // レイアウト + drive_ime を一度回し、キャレット境界を同期させる。
    renderer.render(32.0).unwrap();

    let bounds = renderer.ime_character_bounds();
    assert!(
        bounds[2] > 0.0 && bounds[3] > 0.0,
        "a focused, laid-out caret must yield a non-zero width/height bound, got {bounds:?}"
    );
}
