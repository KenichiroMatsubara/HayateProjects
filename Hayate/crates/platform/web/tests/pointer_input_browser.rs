//! 自前配線した canvas ポインタ経路のエンドツーエンド検証（ADR-0092）。
//!
//! `wasm-pack test --headless --firefox` でヘッドレスブラウザ上を走り、
//! `--no-default-features --features backend-null`（WebGPU / EditContext なし）でビルドする。
//! 実際の `pointermove` を canvas に dispatch し、アダプタが自前で張ったリスナが
//! 変換 + バッファし、`render()` が Core に drain し、`poll_events()` が `HoverEnter`
//! 配信を表面化する。テスト専用エクスポートなしで DOM イベント → アダプタ → Core →
//! poll の全鎖を通す（ADR-0072）。
#![cfg(target_arch = "wasm32")]

use std::cell::Cell;
use std::rc::Rc;

use hayate_adapter_web::HayateElementRenderer;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;
use web_sys::HtmlCanvasElement;

wasm_bindgen_test_configure!(run_in_browser);

/// `PointerKind` のワイヤ判別子（crates/core/src/element/pointer.rs）。
const POINTER_KIND_MOUSE: u32 = 0;
const POINTER_KIND_TOUCH: u32 = 1;
/// `HoverEnter` の生成済みイベント種別判別子（proto/spec/event_kinds.json）。
const HOVER_ENTER_KIND: f64 = 10.0;
/// `HoverLeave` の生成済みイベント種別判別子（proto/spec/event_kinds.json）。
const HOVER_LEAVE_KIND: f64 = 11.0;
/// `Scroll` の生成済みイベント種別判別子（proto/spec/event_kinds.json）。
const SCROLL_KIND: f64 = 7.0;
/// `ElementKind::View` の判別子（crates/core/src/element/kind.rs）。
const ELEMENT_KIND_VIEW: u32 = 0;
/// `ElementKind::Button` の判別子（crates/core/src/element/kind.rs）。
const ELEMENT_KIND_BUTTON: u32 = 3;
/// `ElementKind::ScrollView` の判別子（crates/core/src/element/kind.rs）。
const ELEMENT_KIND_SCROLLVIEW: u32 = 5;
/// style_packet タグ: width=5, height=6; unit 0 = Px（crates/platform/web/src/style_packet.rs）。
const TAG_WIDTH: f32 = 5.0;
const TAG_HEIGHT: f32 = 6.0;
/// `OP_SET_STYLE` の判別子（proto/spec/opcodes.json）。命令的セッターは撤去済み（#439）
/// なので、テストは実ホスト API `apply_mutations`（ADR-0052）だけでスタイルを適用する。
const OP_SET_STYLE: f64 = 4.0;

/// 1 要素のスタイルを `apply_mutations` で適用するテストヘルパ（`OP_SET_STYLE` 1 件）。
fn apply_style(r: &mut HayateElementRenderer, id: f64, packed: &[f32]) {
    let ops = [OP_SET_STYLE, id, 0.0, packed.len() as f64];
    r.apply_mutations(&ops, packed, js_sys::Array::new(), &[]).unwrap();
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

/// 本物の `PointerEvent`（種別 `kind`）をビューポート `(client_x, client_y)` に dispatch する。
fn dispatch_pointer_event(canvas: &HtmlCanvasElement, kind: &str, client_x: f64, client_y: f64) {
    let window = web_sys::window().unwrap();
    let ctor = js_sys::Reflect::get(&window, &JsValue::from_str("PointerEvent")).unwrap();
    let ctor: js_sys::Function = ctor.dyn_into().unwrap();

    let init = js_sys::Object::new();
    js_sys::Reflect::set(&init, &"clientX".into(), &JsValue::from_f64(client_x)).unwrap();
    js_sys::Reflect::set(&init, &"clientY".into(), &JsValue::from_f64(client_y)).unwrap();
    js_sys::Reflect::set(&init, &"bubbles".into(), &JsValue::TRUE).unwrap();
    js_sys::Reflect::set(&init, &"pointerType".into(), &"mouse".into()).unwrap();
    // 実マウスイベントは常に primary。アダプタは非 primary ポインタを無視するため、
    // 合成イベントも primary を名乗る必要がある。
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

/// primary な `touch` PointerEvent（ドラッグ→スクロール経路）を dispatch する。
/// `pointerType: "touch"`、`isPrimary: true`、`pointerId` を設定し、アダプタの
/// primary ポインタフィルタとスクロールジェスチャを起動させる。
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

/// 本物の cancelable な `WheelEvent` を、指定スクロール量でビューポート
/// `(client_x, client_y)` に dispatch し、呼び出し側が `defaultPrevented` を
/// 検査できるよう dispatch したイベントを返す。`dispatch_event` 自体は、リスナが
/// cancelable イベントをキャンセルすると `false` を返し、これがアダプタが
/// ブラウザのネイティブスクロールを抑止した合図になる。
fn dispatch_wheel_event(
    canvas: &HtmlCanvasElement,
    client_x: f64,
    client_y: f64,
    delta_x: f64,
    delta_y: f64,
) -> web_sys::Event {
    let window = web_sys::window().unwrap();
    let ctor = js_sys::Reflect::get(&window, &JsValue::from_str("WheelEvent")).unwrap();
    let ctor: js_sys::Function = ctor.dyn_into().unwrap();

    let init = js_sys::Object::new();
    js_sys::Reflect::set(&init, &"clientX".into(), &JsValue::from_f64(client_x)).unwrap();
    js_sys::Reflect::set(&init, &"clientY".into(), &JsValue::from_f64(client_y)).unwrap();
    js_sys::Reflect::set(&init, &"deltaX".into(), &JsValue::from_f64(delta_x)).unwrap();
    js_sys::Reflect::set(&init, &"deltaY".into(), &JsValue::from_f64(delta_y)).unwrap();
    js_sys::Reflect::set(&init, &"bubbles".into(), &JsValue::TRUE).unwrap();
    // cancelable にすることで、非 passive リスナの `preventDefault` が実際に効く。
    js_sys::Reflect::set(&init, &"cancelable".into(), &JsValue::TRUE).unwrap();

    let args = js_sys::Array::of2(&JsValue::from_str("wheel"), &init);
    let event: web_sys::Event = js_sys::Reflect::construct(&ctor, &args)
        .unwrap()
        .dyn_into()
        .unwrap();
    canvas.dispatch_event(&event).unwrap();
    event
}

/// `rows`（`poll_events` の配信タプル `[listener_id, kind, ...]`）に、`listener_id`
/// 宛て・イベント `kind` の配信が含まれていれば true。
fn has_delivery(rows: &js_sys::Array, listener_id: f64, kind: f64) -> bool {
    (0..rows.length()).any(|i| {
        let row = js_sys::Array::from(&rows.get(i));
        row.get(0).as_f64() == Some(listener_id) && row.get(1).as_f64() == Some(kind)
    })
}

#[wasm_bindgen_test]
async fn dispatched_pointermove_delivers_hover_enter() {
    let canvas = make_canvas(200);
    let mut renderer = HayateElementRenderer::init(canvas.clone(), None)
        .await
        .expect("renderer init");

    // サーフェスを埋める単一ルート View に HoverEnter リスナを付ける。
    renderer.element_create(1.0, ELEMENT_KIND_VIEW).unwrap();
    // TAG_WIDTH=5 / TAG_HEIGHT=6、値 200、unit 0（Px）。
    apply_style(&mut renderer, 1.0, &[5.0, 200.0, 0.0, 6.0, 200.0, 0.0]);
    renderer.set_root(1.0);
    let listener_id = renderer
        .register_listener(1.0, HOVER_ENTER_KIND as u32)
        .unwrap();

    // 最初のフレームでツリーをレイアウトし、ヒットテストに境界を与える。
    renderer.render(0.0).unwrap();

    // ポインタをサーフェス内へ数 CSS px 動かす。ヘッドレスブラウザが報告する
    // どの device-pixel-ratio でも 200px ルートの十分内側に入る。
    let rect = canvas.get_bounding_client_rect();
    dispatch_pointer_move(&canvas, rect.left() + 10.0, rect.top() + 10.0);

    // 次フレームでバッファした move を Core に drain し、HoverEnter を生む。
    renderer.render(16.0).unwrap();

    let rows = renderer.poll_events();
    assert!(
        has_delivery(&rows, listener_id, HOVER_ENTER_KIND),
        "expected a HoverEnter delivery for the self-wired pointermove"
    );
}

#[wasm_bindgen_test]
async fn touch_drag_scrolls_the_scroll_view_and_fires_scroll() {
    // サーフェスサイズの ScrollView に、それより高い子を持たせ、縦スクロール
    // 余地を作る（コンテンツ 600 対ビューポート 200）。
    let canvas = make_canvas(200);
    let mut renderer = HayateElementRenderer::init(canvas.clone(), None)
        .await
        .expect("renderer init");

    renderer.element_create(1.0, ELEMENT_KIND_SCROLLVIEW).unwrap();
    apply_style(&mut renderer, 1.0, &[TAG_WIDTH, 200.0, 0.0, TAG_HEIGHT, 200.0, 0.0]);
    renderer.element_create(2.0, ELEMENT_KIND_VIEW).unwrap();
    apply_style(&mut renderer, 2.0, &[TAG_WIDTH, 200.0, 0.0, TAG_HEIGHT, 600.0, 0.0]);
    renderer.element_append_child(1.0, 2.0);
    renderer.set_root(1.0);
    let scroll_listener = renderer.register_listener(1.0, SCROLL_KIND as u32).unwrap();

    // ヒットテストとコンテンツサイズに形状を与えるためレイアウトする。
    renderer.render(0.0).unwrap();

    let rect = canvas.get_bounding_client_rect();
    let (ox, oy) = (rect.left(), rect.top());
    // 押下し、slop を越えて上方へドラッグし、さらにドラッグ続行する。コンテンツが
    // 指に追従して縦オフセットが伸びる。move は 2 回必要で、1 回目が slop の
    // デッドゾーンを消費（テイクオーバー）し、2 回目が delta を適用する。
    dispatch_touch_event(&canvas, "pointerdown", ox + 100.0, oy + 150.0);
    dispatch_touch_event(&canvas, "pointermove", ox + 100.0, oy + 100.0); // slop を越える
    dispatch_touch_event(&canvas, "pointermove", ox + 100.0, oy + 30.0); // 約 70 スクロール
    dispatch_touch_event(&canvas, "pointerup", ox + 100.0, oy + 30.0);

    // 1 フレームでジェスチャ全体を到着順に drain する。
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

#[wasm_bindgen_test]
async fn wheel_over_canvas_suppresses_the_native_scroll() {
    // canvas 内の wheel は Canvas Mode が端から端まで所有する
    // （`apply_wheel_delta` + chaining、ADR-0084）。自前配線した `wheel` リスナは
    // 非 passive で `preventDefault` する必要があり、そうしないとブラウザが
    // canvas 内スクロールに重ねてページ / ネイティブのスクロール可能な祖先も
    // スクロールしてしまう（二重スクロール）。passive リスナは `preventDefault` を
    // 黙って捨て、ネイティブスクロールが生き、`defaultPrevented` も false になる。
    let canvas = make_canvas(200);
    let mut renderer = HayateElementRenderer::init(canvas.clone(), None)
        .await
        .expect("renderer init");

    // wheel が canvas 内で行き先を持てるよう、スクロール可能な view を置く。
    renderer.element_create(1.0, ELEMENT_KIND_SCROLLVIEW).unwrap();
    apply_style(&mut renderer, 1.0, &[TAG_WIDTH, 200.0, 0.0, TAG_HEIGHT, 200.0, 0.0]);
    renderer.element_create(2.0, ELEMENT_KIND_VIEW).unwrap();
    apply_style(&mut renderer, 2.0, &[TAG_WIDTH, 200.0, 0.0, TAG_HEIGHT, 600.0, 0.0]);
    renderer.element_append_child(1.0, 2.0);
    renderer.set_root(1.0);
    renderer.render(0.0).unwrap();

    let rect = canvas.get_bounding_client_rect();
    let event = dispatch_wheel_event(&canvas, rect.left() + 100.0, rect.top() + 100.0, 0.0, 40.0);

    assert!(
        event.default_prevented(),
        "a wheel over the canvas must preventDefault so the page does not \
         double-scroll alongside the in-canvas scroll"
    );

    // wheel は canvas 内スクロールも依然として駆動する（抑止対象はブラウザの
    // ネイティブスクロールであって、Canvas Mode 自身の処理ではない）。
    renderer.render(16.0).unwrap();
    assert!(
        renderer.element_get_scroll_offset(1.0)[1] > 0.0,
        "the wheel must still scroll the in-canvas scroll-view"
    );
}

/// 200×200 の ScrollView に 200×600 の子を持たせて 400px の縦スクロール域を作り、
/// `Scroll` リスナを登録し、一度レイアウトして、renderer と canvas のクライアント
/// 原点・リスナ id を返す。momentum の e2e テスト群で共有する。
async fn scrollable_renderer(canvas: &HtmlCanvasElement) -> (HayateElementRenderer, f64, f64, f64) {
    let mut renderer = HayateElementRenderer::init(canvas.clone(), None)
        .await
        .expect("renderer init");

    renderer.element_create(1.0, ELEMENT_KIND_SCROLLVIEW).unwrap();
    apply_style(&mut renderer, 1.0, &[TAG_WIDTH, 200.0, 0.0, TAG_HEIGHT, 200.0, 0.0]);
    renderer.element_create(2.0, ELEMENT_KIND_VIEW).unwrap();
    apply_style(&mut renderer, 2.0, &[TAG_WIDTH, 200.0, 0.0, TAG_HEIGHT, 600.0, 0.0]);
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
    // 実フリックを駆動する。rAF フレームごとに 1 move として速度トラッカに
    // 別個のタイムスタンプを見せ、リリース後の fling を自前フレームで惰走させる。
    let canvas = make_canvas(200);
    let (mut renderer, ox, oy, scroll_listener) = scrollable_renderer(&canvas).await;

    // 指が連続フレームで 150 → 60 へ上がる（各 move は slop 超の間隔）。
    dispatch_touch_event(&canvas, "pointerdown", ox + 100.0, oy + 150.0);
    renderer.render(16.0).unwrap();
    dispatch_touch_event(&canvas, "pointermove", ox + 100.0, oy + 120.0); // slop を越える
    renderer.render(32.0).unwrap();
    dispatch_touch_event(&canvas, "pointermove", ox + 100.0, oy + 90.0);
    renderer.render(48.0).unwrap();
    dispatch_touch_event(&canvas, "pointermove", ox + 100.0, oy + 60.0);
    renderer.render(64.0).unwrap();
    dispatch_touch_event(&canvas, "pointerup", ox + 100.0, oy + 60.0);
    let _ = renderer.poll_events(); // ドラッグ相の Scroll 配信を捨てる

    // リリースフレームでサンプルした fling から momentum を起動する。
    renderer.render(80.0).unwrap();
    let offset_at_release = scroll_offset_y(&renderer);
    let _ = renderer.poll_events();

    // 純粋な momentum フレーム（ポインタ入力なし）も、指ドラッグと全く同様に
    // スクロールを続け、Event::Scroll を発火しなければならない。
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

    // 惰走・バウンス・収束: この強い fling は 400px 域を超過し、下端を越えて
    // overscroll へバウンスし、spring-back が端へ戻して静止させる。
    // アニメ全体を通じてピークオフセットを追う。
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
    // 上端でコンテンツをさらに下へドラッグすると、ラバーバンド抵抗を伴って
    // overscroll（負オフセット）へ引き込まれ、リリースで元へ戻る。
    let canvas = make_canvas(200);
    let (mut renderer, ox, oy, _scroll_listener) = scrollable_renderer(&canvas).await;

    // 上端付近で押下し、slop を越え（テイクオーバー、delta なし）、指をさらに
    // 約 100px 下へドラッグする。コンテンツが上端より下へ追従し、縦オフセットが
    // 負（上端を越えた overscroll）になる。
    dispatch_touch_event(&canvas, "pointerdown", ox + 100.0, oy + 40.0);
    renderer.render(16.0).unwrap();
    dispatch_touch_event(&canvas, "pointermove", ox + 100.0, oy + 60.0); // slop を越える
    renderer.render(32.0).unwrap();
    dispatch_touch_event(&canvas, "pointermove", ox + 100.0, oy + 160.0); // さらに 100px 下
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

    // overscroll 中にリリース: spring-back がオフセットを端（0）へ戻す。
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

    // 上方へフリックして fling を惰走させる。
    dispatch_touch_event(&canvas, "pointerdown", ox + 100.0, oy + 150.0);
    renderer.render(16.0).unwrap();
    dispatch_touch_event(&canvas, "pointermove", ox + 100.0, oy + 120.0);
    renderer.render(32.0).unwrap();
    dispatch_touch_event(&canvas, "pointermove", ox + 100.0, oy + 90.0);
    renderer.render(48.0).unwrap();
    dispatch_touch_event(&canvas, "pointerup", ox + 100.0, oy + 90.0);
    let _ = renderer.poll_events();

    renderer.render(64.0).unwrap(); // 起動
    let offset_at_release = scroll_offset_y(&renderer);
    renderer.render(80.0).unwrap(); // 1 フレーム惰走
    let offset_coasting = scroll_offset_y(&renderer);
    assert!(
        offset_coasting > offset_at_release,
        "precondition: momentum must be coasting before the interrupting press",
    );

    // 惰走中に再び押下: down が fling を中断する。drain はフレームの momentum
    // ステップより前に押下を処理する（momentum → None）ため、オフセットは指の下で
    // 即座に止まる。
    dispatch_touch_event(&canvas, "pointerdown", ox + 100.0, oy + 100.0);
    renderer.render(96.0).unwrap();
    let frozen = scroll_offset_y(&renderer);

    // 以降の入力なしフレームは動いてはならない。fling は消えている。
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
    // Platform Adapter は `PointerEvent.pointerType` をコアの `PointerKind` に
    // マップし、自前配線のポインタ経路で転送する。これにより Core はインタラク
    // ションごとに `last_pointer_kind` を保持する。renderer のアクセサ経由で
    // エンドツーエンドに観測する（テスト専用エクスポートなし、ADR-0072）。
    let canvas = make_canvas(200);
    let mut renderer = HayateElementRenderer::init(canvas.clone(), None)
        .await
        .expect("renderer init");

    renderer.element_create(1.0, ELEMENT_KIND_VIEW).unwrap();
    apply_style(&mut renderer, 1.0, &[TAG_WIDTH, 200.0, 0.0, TAG_HEIGHT, 200.0, 0.0]);
    renderer.set_root(1.0);
    renderer.render(0.0).unwrap();

    // どのポインタイベントの前でも、種別の既定は mouse。
    assert_eq!(renderer.last_pointer_kind(), POINTER_KIND_MOUSE);

    let rect = canvas.get_bounding_client_rect();
    let (ox, oy) = (rect.left(), rect.top());

    // 本物の touch 押下は PointerKind::Touch を Core へ転送する。
    dispatch_touch_event(&canvas, "pointerdown", ox + 50.0, oy + 50.0);
    renderer.render(16.0).unwrap();
    assert_eq!(
        renderer.last_pointer_kind(),
        POINTER_KIND_TOUCH,
        "a touch pointerdown must set Core's last_pointer_kind to Touch"
    );

    // 続くマウス move は実デバイスに追従する（最初のインタラクションで固定せず、
    // ハイブリッドに追従する）。
    dispatch_pointer_move(&canvas, ox + 80.0, oy + 80.0);
    renderer.render(32.0).unwrap();
    assert_eq!(
        renderer.last_pointer_kind(),
        POINTER_KIND_MOUSE,
        "a mouse pointermove must update last_pointer_kind back to Mouse"
    );
}

#[wasm_bindgen_test]
async fn pointer_move_over_button_applies_pointer_cursor_to_the_canvas() {
    // 自前配線のポインタ経路は、解決済みカーソル（ADR-0088, ADR-0105）を
    // canvas 要素自体に適用する。明示的な `cursor` のないボタンをホバーすると
    // 要素種別の UA 既定（pointer）が出るため、アプリが個別に styling せずとも
    // Canvas が DOM の `<button>` に一致する。
    let canvas = make_canvas(200);
    let mut renderer = HayateElementRenderer::init(canvas.clone(), None)
        .await
        .expect("renderer init");

    // サーフェスを埋めるボタン、明示的な cursor スタイルなし。
    renderer.element_create(1.0, ELEMENT_KIND_BUTTON).unwrap();
    apply_style(&mut renderer, 1.0, &[TAG_WIDTH, 200.0, 0.0, TAG_HEIGHT, 200.0, 0.0]);
    renderer.set_root(1.0);
    renderer.render(0.0).unwrap();

    let rect = canvas.get_bounding_client_rect();
    dispatch_pointer_move(&canvas, rect.left() + 10.0, rect.top() + 10.0);
    renderer.render(16.0).unwrap();

    assert_eq!(
        canvas.style().get_property_value("cursor").unwrap(),
        "pointer",
        "hovering a button must apply the pointer cursor to the canvas element"
    );
}

#[wasm_bindgen_test]
async fn a_buffered_pointer_input_wakes_the_on_demand_frame_loop() {
    // 回帰（ADR-0080 / ADR-0126）: 自前配線のポインタ listener は入力をバッファした直後に
    // `request_redraw` を叩き、idle に落ちた on-demand フレームループを 1 フレーム起こさねば
    // ならない。これが無いと `render()` が呼ばれず `pending_pointer` が drain されないため、
    // idle 時のタップが捨てられる（Android Chrome でボタンが無反応になる回帰）。
    // ここでは wake コールバック（JS の `scheduleFrame` 相当）を注入し、`render()` を挟まずに
    // 本物の `pointerdown` を dispatch して、listener が wake を叩くことを直接検証する。
    let canvas = make_canvas(200);
    let mut renderer = HayateElementRenderer::init(canvas.clone(), None)
        .await
        .expect("renderer init");

    renderer.element_create(1.0, ELEMENT_KIND_VIEW).unwrap();
    apply_style(&mut renderer, 1.0, &[TAG_WIDTH, 200.0, 0.0, TAG_HEIGHT, 200.0, 0.0]);
    renderer.set_root(1.0);
    renderer.render(0.0).unwrap();

    // wake コールバックの発火回数を数える。set_request_redraw で JS の scheduleFrame を注入
    // する経路そのもの（HayateRenderer.start() が本番で叩く）。
    let count = Rc::new(Cell::new(0u32));
    let count_cb = count.clone();
    let closure = Closure::wrap(Box::new(move || {
        count_cb.set(count_cb.get() + 1);
    }) as Box<dyn FnMut()>);
    renderer.set_request_redraw(closure.as_ref().unchecked_ref::<js_sys::Function>().clone());

    let rect = canvas.get_bounding_client_rect();
    // 本物の touch pointerdown。render は呼ばない — wake だけを見る。
    dispatch_touch_event(&canvas, "pointerdown", rect.left() + 50.0, rect.top() + 50.0);
    assert!(
        count.get() >= 1,
        "a touch pointerdown must wake the on-demand frame loop via request_redraw"
    );

    // pointerup も wake する（タップの解放フレームが drain されるように）。
    let before = count.get();
    dispatch_touch_event(&canvas, "pointerup", rect.left() + 50.0, rect.top() + 50.0);
    assert!(
        count.get() > before,
        "a touch pointerup must also wake the loop so the tap's release is drained"
    );

    drop(closure);
}

#[wasm_bindgen_test]
async fn dispatched_pointerleave_delivers_hover_leave() {
    let canvas = make_canvas(200);
    let mut renderer = HayateElementRenderer::init(canvas.clone(), None)
        .await
        .expect("renderer init");

    renderer.element_create(1.0, ELEMENT_KIND_VIEW).unwrap();
    apply_style(&mut renderer, 1.0, &[5.0, 200.0, 0.0, 6.0, 200.0, 0.0]);
    renderer.set_root(1.0);
    let enter_listener = renderer
        .register_listener(1.0, HOVER_ENTER_KIND as u32)
        .unwrap();
    let leave_listener = renderer
        .register_listener(1.0, HOVER_LEAVE_KIND as u32)
        .unwrap();

    renderer.render(0.0).unwrap();

    // サーフェス内へ移動: 自前配線の `pointermove` が HoverEnter を生む。
    let rect = canvas.get_bounding_client_rect();
    dispatch_pointer_move(&canvas, rect.left() + 10.0, rect.top() + 10.0);
    renderer.render(16.0).unwrap();
    assert!(
        has_delivery(&renderer.poll_events(), enter_listener, HOVER_ENTER_KIND),
        "precondition: pointermove should HoverEnter the root"
    );

    // サーフェスから離脱: 自前配線の `pointerleave` がホバーをクリアし、
    // 直前にホバーしていたルートへ HoverLeave を配信する。
    dispatch_pointer_event(&canvas, "pointerleave", rect.left() + 10.0, rect.top() + 10.0);
    renderer.render(32.0).unwrap();
    assert!(
        has_delivery(&renderer.poll_events(), leave_listener, HOVER_LEAVE_KIND),
        "expected a HoverLeave delivery for the self-wired pointerleave"
    );
}
