//! Canvas キーマップ → EditIntent 経路の E2E 検証（ADR-0103）。
//!
//! ヘッドレスブラウザ上で実行する。実際の `pointerdown` でレイアウト済みの
//! text-input をフォーカスし、`on_key_down` の矢印キーが wasm 境界を越えて
//! アダプタのキーマップで `EditIntent` に変換され、core の `apply_edit_intent`
//! を駆動する。後続の `on_text_input` が移動後のキャレット位置に挿入するため
//! 観測できる。テスト専用エクスポートは使わず、実ホスト API のみで検証する
//! （ADR-0072）。
#![cfg(target_arch = "wasm32")]

use hayate_adapter_web::HayateElementRenderer;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::*;
use web_sys::HtmlCanvasElement;

wasm_bindgen_test_configure!(run_in_browser);

/// `ElementKind::TextInput` の判別値。
const ELEMENT_KIND_TEXT_INPUT: u32 = 4;
/// `MODIFIER_CTRL`。Win/Linux の主修飾キーで、アダプタのキーマップでは
/// クリップボードや全選択の intent に対応する。
const MOD_CTRL: u32 = 2;
/// スタイルパケットのタグ: WIDTH=5, HEIGHT=6, 単位 Px=0。
const TAG_WIDTH: f32 = 5.0;
const TAG_HEIGHT: f32 = 6.0;
const UNIT_PX: f32 = 0.0;
/// 撤去した命令的セッター（#439）の代わりに使う wire opcode の判別子
/// （proto/spec/opcodes.json）。テストは実ホスト API `apply_mutations`（ADR-0052）の
/// バッチ経路だけでミューテーションを駆動する（ADR-0072: テスト専用エクスポート無し）。
const OP_SET_STYLE: f64 = 4.0;
const OP_SET_TEXT_CONTENT: f64 = 12.0;
const OP_SET_MULTILINE: f64 = 18.0;

/// 1 要素のスタイルを `apply_mutations` で適用する（`OP_SET_STYLE` 1 件）。
fn apply_style(r: &mut HayateElementRenderer, id: f64, packed: &[f32]) {
    let ops = [OP_SET_STYLE, id, 0.0, packed.len() as f64];
    r.apply_mutations(&ops, packed, js_sys::Array::new(), &[])
        .unwrap();
}

/// 編集可能テキスト内容を `apply_mutations` で設定する（`OP_SET_TEXT_CONTENT` 1 件）。
fn apply_text_content(r: &mut HayateElementRenderer, id: f64, text: &str) {
    let texts = js_sys::Array::new();
    texts.push(&JsValue::from_str(text));
    let ops = [OP_SET_TEXT_CONTENT, id, 0.0];
    r.apply_mutations(&ops, &[], texts, &[]).unwrap();
}

/// 複数行フラグを `apply_mutations` で設定する（`OP_SET_MULTILINE` 1 件）。
fn apply_multiline(r: &mut HayateElementRenderer, id: f64, multiline: bool) {
    let ops = [OP_SET_MULTILINE, id, if multiline { 1.0 } else { 0.0 }];
    r.apply_mutations(&ops, &[], js_sys::Array::new(), &[])
        .unwrap();
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
    // アダプタの pointerdown リスナーは非プライマリポインタを無視し
    // （`pe.is_primary()`）、`PointerEventInit.isPrimary` の既定は false。
    // これがないと合成押下が捨てられ input がフォーカスされない。
    js_sys::Reflect::set(&init, &"isPrimary".into(), &JsValue::TRUE).unwrap();

    let args = js_sys::Array::of2(&JsValue::from_str("pointerdown"), &init);
    let event: web_sys::Event = js_sys::Reflect::construct(&ctor, &args)
        .unwrap()
        .dyn_into()
        .unwrap();
    canvas.dispatch_event(&event).unwrap();
}

/// `navigator.clipboard.readText` を `text` に解決するスタブで置き換える。
/// ヘッドレス環境では実クリップボード読み取り権限が拒否されるため、
/// 非同期の Ctrl/Cmd+V 読み取りを決定的にする。アダプタが参照する生の
/// `Clipboard` オブジェクトを差し替えるので、アダプタ側にテスト専用エクスポートは要らない。
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
    // アダプタが後で呼ぶので、クロージャをページ寿命まで生かす。
    stub.forget();
}

/// マイクロタスクキューに譲り、`spawn_local` した future（クリップボード読み取り）が
/// 結果検査の前に完了できるようにする。
async fn flush_microtasks() {
    for _ in 0..3 {
        let _ = JsFuture::from(js_sys::Promise::resolve(&JsValue::UNDEFINED)).await;
    }
}

#[wasm_bindgen_test]
async fn arrow_key_moves_the_caret_through_the_canvas_keymap() {
    let canvas = make_canvas(200);
    let mut renderer = HayateElementRenderer::init(canvas.clone(), None)
        .await
        .expect("renderer init");

    // 画面いっぱいの text-input に "hello"（キャレットは末尾）。
    renderer
        .element_create(1.0, ELEMENT_KIND_TEXT_INPUT)
        .unwrap();
    apply_style(
        &mut renderer,
        1.0,
        &[TAG_WIDTH, 200.0, UNIT_PX, TAG_HEIGHT, 200.0, UNIT_PX],
    );
    renderer.set_root(1.0);
    apply_text_content(&mut renderer, 1.0, "hello");

    // レイアウト後、実 pointerdown を render で処理して input をフォーカスする。
    renderer.render(0.0).unwrap();
    let rect = canvas.get_bounding_client_rect();
    dispatch_pointer_down(&canvas, rect.left() + 10.0, rect.top() + 10.0);
    renderer.render(16.0).unwrap();
    assert_eq!(
        renderer.focused_element_id(),
        1.0,
        "the pointerdown should focus the text-input"
    );

    // キャレットを先頭まで動かす。各 ArrowLeft は wasm 境界を越えて
    // Move/Grapheme/Backward intent に変換され、キャレットを左へ1歩
    // （0でクランプ）。その後の入力はキャレット位置に挿入される。
    for _ in 0..10 {
        renderer.on_key_down("ArrowLeft", 0);
    }
    renderer.on_text_input(1.0, "X");
    assert_eq!(
        renderer.element_get_text_content(1.0),
        "Xhello",
        "ArrowLeft moved the caret to the start, so typing inserts there"
    );

    // 末尾まで戻して入力すると末尾に挿入され、ArrowRight もキーマップを経由することを示す。
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
    // 複数行 text-input では Enter を末尾追加ではなくキャレット位置への
    // 改行挿入として扱う。キャレットを先頭へ動かしてから Enter。
    let canvas = make_canvas(200);
    let mut renderer = HayateElementRenderer::init(canvas.clone(), None)
        .await
        .expect("renderer init");

    renderer
        .element_create(1.0, ELEMENT_KIND_TEXT_INPUT)
        .unwrap();
    apply_style(
        &mut renderer,
        1.0,
        &[TAG_WIDTH, 200.0, UNIT_PX, TAG_HEIGHT, 200.0, UNIT_PX],
    );
    renderer.set_root(1.0);
    apply_multiline(&mut renderer, 1.0, true);
    apply_text_content(&mut renderer, 1.0, "ab");

    renderer.render(0.0).unwrap();
    let rect = canvas.get_bounding_client_rect();
    dispatch_pointer_down(&canvas, rect.left() + 10.0, rect.top() + 10.0);
    renderer.render(16.0).unwrap();
    assert_eq!(renderer.focused_element_id(), 1.0);

    // 押下はクリック位置にキャレットを置く（ADR-0097）ので末尾とは限らない。
    // まず末尾へクランプし（末尾より先の ArrowRight は no-op）、1歩左へ動かして
    // 'a' と 'b' の間に置く。
    renderer.on_key_down("ArrowRight", 0);
    renderer.on_key_down("ArrowRight", 0);
    renderer.on_key_down("ArrowLeft", 0); // キャレットを 'a' と 'b' の間へ
    renderer.on_key_down("Enter", 0);
    assert_eq!(
        renderer.element_get_text_content(1.0),
        "a\nb",
        "Enter inserts the newline at the caret, not at the end",
    );
}

#[wasm_bindgen_test]
async fn enter_in_a_single_line_field_does_not_insert_a_newline() {
    // 既定（単一行）の text-input は Enter でテキストを変えない。
    // このキーはアプリの送信シグナルであり改行ではない。
    let canvas = make_canvas(200);
    let mut renderer = HayateElementRenderer::init(canvas.clone(), None)
        .await
        .expect("renderer init");

    renderer
        .element_create(1.0, ELEMENT_KIND_TEXT_INPUT)
        .unwrap();
    apply_style(
        &mut renderer,
        1.0,
        &[TAG_WIDTH, 200.0, UNIT_PX, TAG_HEIGHT, 200.0, UNIT_PX],
    );
    renderer.set_root(1.0);
    apply_text_content(&mut renderer, 1.0, "ab");

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
    let mut renderer = HayateElementRenderer::init(canvas.clone(), None)
        .await
        .expect("renderer init");

    renderer
        .element_create(1.0, ELEMENT_KIND_TEXT_INPUT)
        .unwrap();
    apply_style(
        &mut renderer,
        1.0,
        &[TAG_WIDTH, 200.0, UNIT_PX, TAG_HEIGHT, 200.0, UNIT_PX],
    );
    renderer.set_root(1.0);
    apply_text_content(&mut renderer, 1.0, "hello");

    renderer.render(0.0).unwrap();
    let rect = canvas.get_bounding_client_rect();
    dispatch_pointer_down(&canvas, rect.left() + 10.0, rect.top() + 10.0);
    renderer.render(16.0).unwrap();
    assert_eq!(renderer.focused_element_id(), 1.0);

    // Home は Move/LineBoundary/Backward intent に変換され、フィールド先頭へ
    // ジャンプする。入力はそこに挿入される。
    renderer.on_key_down("Home", 0);
    renderer.on_text_input(1.0, "X");
    assert_eq!(
        renderer.element_get_text_content(1.0),
        "Xhello",
        "Home jumped the caret to the field start"
    );

    // End は Move/LineBoundary/Forward に変換され、フィールド末尾へ戻る。
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
    let mut renderer = HayateElementRenderer::init(canvas.clone(), None)
        .await
        .expect("renderer init");

    renderer
        .element_create(1.0, ELEMENT_KIND_TEXT_INPUT)
        .unwrap();
    apply_style(
        &mut renderer,
        1.0,
        &[TAG_WIDTH, 200.0, UNIT_PX, TAG_HEIGHT, 200.0, UNIT_PX],
    );
    renderer.set_root(1.0);
    apply_text_content(&mut renderer, 1.0, "hello");

    renderer.render(0.0).unwrap();
    let rect = canvas.get_bounding_client_rect();
    dispatch_pointer_down(&canvas, rect.left() + 10.0, rect.top() + 10.0);
    renderer.render(16.0).unwrap();
    assert_eq!(
        renderer.focused_element_id(),
        1.0,
        "the pointerdown should focus the input"
    );

    // キャレットを末尾へ動かし、Backspace で末尾の 'o' を削除する。
    // キーは wasm 境界を越えて Delete/Backward に変換され内容を編集する。
    for _ in 0..10 {
        renderer.on_key_down("ArrowRight", 0);
    }
    renderer.on_key_down("Backspace", 0);
    assert_eq!(
        renderer.element_get_text_content(1.0),
        "hell",
        "Backspace removed the char before the caret"
    );

    // キャレットを先頭へ動かし、Delete で先頭の 'h' を削除する。
    // 前方削除もキーマップを経由することを示す。
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
async fn ctrl_delete_keys_remove_words_through_the_canvas_keymap() {
    // Ctrl+Backspace / Ctrl+Delete は wasm 境界を越え、アダプタのキーマップで
    // Delete/Word intent に変換され、単語まるごとを削除する。
    let canvas = make_canvas(200);
    let mut renderer = HayateElementRenderer::init(canvas.clone(), None)
        .await
        .expect("renderer init");

    renderer
        .element_create(1.0, ELEMENT_KIND_TEXT_INPUT)
        .unwrap();
    apply_style(
        &mut renderer,
        1.0,
        &[TAG_WIDTH, 200.0, UNIT_PX, TAG_HEIGHT, 200.0, UNIT_PX],
    );
    renderer.set_root(1.0);
    apply_text_content(&mut renderer, 1.0, "hello world");

    renderer.render(0.0).unwrap();
    let rect = canvas.get_bounding_client_rect();
    dispatch_pointer_down(&canvas, rect.left() + 10.0, rect.top() + 10.0);
    renderer.render(16.0).unwrap();
    assert_eq!(
        renderer.focused_element_id(),
        1.0,
        "the pointerdown should focus the input"
    );

    // キャレットを末尾へ動かし、Ctrl+Backspace で末尾の単語を削除する。
    for _ in 0..20 {
        renderer.on_key_down("ArrowRight", 0);
    }
    renderer.on_key_down("Backspace", MOD_CTRL);
    assert_eq!(
        renderer.element_get_text_content(1.0),
        "hello ",
        "Ctrl+Backspace removed the word before the caret"
    );

    // キャレットを先頭へ動かし、Ctrl+Delete で先頭の単語を削除する。
    for _ in 0..20 {
        renderer.on_key_down("ArrowLeft", 0);
    }
    renderer.on_key_down("Delete", MOD_CTRL);
    assert_eq!(
        renderer.element_get_text_content(1.0),
        " ",
        "Ctrl+Delete removed the word after the caret"
    );
}

#[wasm_bindgen_test]
async fn ctrl_v_pastes_clipboard_text_through_the_canvas_async_read() {
    // ブラウザのクリップボード読み取りは非同期なので、Canvas Mode は core の
    // 同期 `Clipboard::read_text` では処理できない（ADR-0097）。Ctrl/Cmd+V は
    // `navigator.clipboard.readText()` を起動し、解決したテキストを次の render で
    // `element_paste` に戻す。読み取りをスタブすればこの経路全体が観測でき、
    // 空のフォーカス中フィールドにクリップボードのテキストが入る。
    let canvas = make_canvas(200);
    let mut renderer = HayateElementRenderer::init(canvas.clone(), None)
        .await
        .expect("renderer init");

    renderer
        .element_create(1.0, ELEMENT_KIND_TEXT_INPUT)
        .unwrap();
    apply_style(
        &mut renderer,
        1.0,
        &[TAG_WIDTH, 200.0, UNIT_PX, TAG_HEIGHT, 200.0, UNIT_PX],
    );
    renderer.set_root(1.0);

    // レイアウト後、実 pointerdown で（空の）input をフォーカスする。
    renderer.render(0.0).unwrap();
    let rect = canvas.get_bounding_client_rect();
    dispatch_pointer_down(&canvas, rect.left() + 10.0, rect.top() + 10.0);
    renderer.render(16.0).unwrap();
    assert_eq!(
        renderer.focused_element_id(),
        1.0,
        "pointerdown focuses the input"
    );

    stub_clipboard_read_text("PASTED");

    // Ctrl+V はアダプタのキーマップで Paste intent に対応し、同期シームを通さず
    // 非同期読み取りを開始する。
    renderer.on_key_down("v", MOD_CTRL);
    // 起動した読み取りを解決させ、render がそれをフィールドへ流し込む。
    flush_microtasks().await;
    renderer.render(32.0).unwrap();

    assert_eq!(
        renderer.element_get_text_content(1.0),
        "PASTED",
        "Ctrl+V pasted the clipboard text into the focused field via the async read",
    );
}

#[wasm_bindgen_test]
async fn public_edit_intent_wire_reports_all_outcomes_and_protocol_errors() {
    let canvas = make_canvas(100);
    let mut renderer = HayateElementRenderer::init(canvas, None).await.unwrap();
    renderer
        .element_create(1.0, ELEMENT_KIND_TEXT_INPUT)
        .unwrap();
    renderer.set_root(1.0);
    apply_text_content(&mut renderer, 1.0, "ab");

    assert_eq!(renderer.dispatch_edit_intent(1.0, &[4.0]).unwrap(), 0);
    assert_eq!(renderer.dispatch_edit_intent(999.0, &[4.0]).unwrap(), 1);
    assert_eq!(renderer.dispatch_edit_intent(1.0, &[3.0]).unwrap(), 1);

    renderer.on_composition_start(1.0, "x");
    assert_eq!(renderer.dispatch_edit_intent(1.0, &[4.0]).unwrap(), 1);
    renderer.on_composition_end(1.0, "x");

    stub_clipboard_read_text("deferred");
    assert_eq!(renderer.dispatch_edit_intent(1.0, &[7.0]).unwrap(), 2);
    assert!(renderer.dispatch_edit_intent(1.0, &[99.0]).is_err());
    assert!(renderer.dispatch_edit_intent(f64::NAN, &[4.0]).is_err());
}
