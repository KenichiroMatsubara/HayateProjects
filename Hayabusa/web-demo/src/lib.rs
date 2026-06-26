//! Hayabusa → hayate-adapter-web present 統合（ADR-0045 in-process projection）のデモ。
//!
//! 経路（wire を介さない・同一プロセス Rust。Tsubame は不在）:
//!
//! ```text
//! Hayabusa Template IR + reactive runtime（Signal/Memo/Effect）
//!   → instantiate（effect が ElementSink へ mutation を積む）
//!     → HayabusaApp（DeliverySink）::handle(deliveries, renderer.tree_mut())
//!       → mutation を adapter-web の ElementTree へ drain（初期構築 + click 由来 patch）
//!         → HayateElementRenderer::render()（tree → layout → SceneGraph → tiny-skia → canvas）
//! ```
//!
//! 入力もライブの実経路で通す。canvas renderer は `init()` 内で自前のポインタ listener を
//! canvas へ attach する（production の todo デモと同じ経路）。本デモは `requestAnimationFrame`
//! ループで毎フレーム:
//!
//! ```text
//! render(ts)                         // buffered pointer を排出 → hit-test → Click を tree へ dispatch
//!   → tree.poll_deliveries()         // その Click delivery を取り出し
//!     → HayabusaApp::handle(d, tree) // listener→ElId→handler(+1)→signal 更新→Memo→Effect→text patch
//! ```
//!
//! を回す。AppHost::tick と同型（ただし tree は renderer が所有）。クリックすると数が増える。

use std::cell::RefCell;
use std::rc::Rc;

use hayabusa::prelude::{
    instantiate, Binding, Display, ElementKind, Expr, FlexDirection, Handler, HayabusaApp, Length,
    RecordingSink, Rgba, Runtime, Scope, StyleProp, TemplateNode, Value,
};
use hayate_adapter_web::HayateElementRenderer;
use hayate_app_host::DeliverySink;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HtmlCanvasElement;

/// core の既定 font-family（`hayate_core::DEFAULT_FONT_FAMILY`）。Hayabusa の style 語彙は
/// font-family を持たないため、テキストはこのファミリで shape される。同梱の Noto Sans JP を
/// この名前で登録すれば Latin（"+1"・数字）も日本語（"隼…"）も CPU バックエンドで描画できる
/// （ネットワーク fetch 不要 = headless 安定）。
const DEFAULT_FONT_FAMILY: &str = "Noto Sans";

/// リポジトリ同梱フォント。`include_bytes!` で wasm に焼き込み、起動時に登録する。
const NOTO_SANS_JP: &[u8] =
    include_bytes!("../../../Hayate/crates/core/assets/fonts/NotoSansJP.ttf");

/// 0..255 の sRGB を Hayabusa の 0..1 正規化 `Rgba` へ。
fn rgb(r: u8, g: u8, b: u8) -> Rgba {
    Rgba {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    }
}

/// canvas を受け取り、Hayabusa が組んだカウンタコンポーネントを present する。
/// 起動時に click を 3 回合成し、reactive 経路で count が 3 まで上がった状態を描画する。
#[wasm_bindgen]
pub async fn boot(canvas: HtmlCanvasElement) -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    // ── 1. Hayabusa：手組み Template IR + reactive runtime ───────────────────
    let rt = Runtime::new();
    let count = rt.signal(Value::number(0));

    // handler 0：+1。捕捉する signal は clone（scope 配線が count を読むため move し切らない）。
    let increment: Handler = {
        let count = count.clone();
        Box::new(move |_: Value| {
            count.update(|v| Value::number(v.as_number().unwrap() + 1.0));
        })
    };

    // <view column> ├ <text 隼…> ├ <text {count}> └ <button>└<text +1>
    // button のラベルは子 `text` 要素にする（text 要素だけがグリフを描く・RN 語彙）。
    let template = TemplateNode::new(ElementKind::View)
        .style(vec![
            StyleProp::Display(Display::Flex),
            StyleProp::FlexDirection(FlexDirection::Column),
            StyleProp::Gap(Length::Px(14.0)),
            StyleProp::Padding(Length::Px(28.0)),
            StyleProp::Width(Length::Percent(100.0)),
            StyleProp::Height(Length::Percent(100.0)),
            StyleProp::BackgroundColor(rgb(0xff, 0xff, 0xff)),
        ])
        .child(
            TemplateNode::new(ElementKind::Text)
                .text("隼 を Web に描画")
                .style(vec![
                    StyleProp::FontSize(20.0),
                    StyleProp::TextColor(rgb(0x0f, 0x17, 0x2a)),
                ]),
        )
        .child(
            TemplateNode::new(ElementKind::Text)
                .bind_text(Expr::var("count"))
                .style(vec![
                    StyleProp::FontSize(64.0),
                    StyleProp::TextColor(rgb(0x25, 0x63, 0xeb)),
                ]),
        )
        .child(
            TemplateNode::new(ElementKind::Button)
                .on_click(0)
                .style(vec![
                    StyleProp::BackgroundColor(rgb(0x25, 0x63, 0xeb)),
                    StyleProp::Padding(Length::Px(10.0)),
                ])
                .child(
                    TemplateNode::new(ElementKind::Text).text("+1").style(vec![
                        StyleProp::FontSize(18.0),
                        StyleProp::TextColor(rgb(0xff, 0xff, 0xff)),
                    ]),
                ),
        );

    let scope = Scope::new().with("count", Binding::Signal(count.clone()));
    let handlers: Vec<Handler> = vec![increment];
    let sink = Rc::new(RefCell::new(RecordingSink::new()));
    let instance = instantiate(&rt, &template, &scope, handlers, sink);
    let mut app = HayabusaApp::new(instance);

    // ── 2. adapter-web：canvas 上に CPU(tiny-skia) present レンダラ ───────────
    // init() が canvas へポインタ listener を attach する（hit-test → Click は render() で発火）。
    let mut renderer = HayateElementRenderer::init(canvas).await?;
    renderer.register_font_bytes(DEFAULT_FONT_FAMILY, NOTO_SANS_JP);
    renderer.set_background_color(0.043, 0.063, 0.125); // #0b1020

    // ── 3. in-process projection：初期構築 mutation を借用ツリーへ drain（root 設定 + listener 登録）─
    DeliverySink::handle(&mut app, &[], renderer.tree_mut());

    // ── 4. ライブ raf ループ：render → poll_deliveries → handle ───────────────
    let state = Rc::new(RefCell::new((renderer, app)));
    let raf: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::new(RefCell::new(None));
    let raf_kick = raf.clone();
    *raf.borrow_mut() = Some(Closure::wrap(Box::new(move |ts: f64| {
        {
            let mut s = state.borrow_mut();
            let (renderer, app) = &mut *s;
            // render() が buffered pointer を排出し、hit-test 成立で Click を tree へ dispatch する。
            let _ = renderer.render(ts);
            // その Click delivery を取り出し、Hayabusa の handler 経由で reactive patch を起こす
            // （適用先は同じ借用ツリー。次フレームの render で新しい text が present される）。
            let deliveries = renderer.tree_mut().poll_deliveries();
            if !deliveries.is_empty() {
                DeliverySink::handle(app, &deliveries, renderer.tree_mut());
            }
        }
        request_animation_frame(raf_kick.borrow().as_ref().unwrap());
    }) as Box<dyn FnMut(f64)>));
    request_animation_frame(raf.borrow().as_ref().unwrap());
    // raf チェーンが自身を保持し続けるよう Closure をリークさせる（ループは恒久）。
    std::mem::forget(raf);

    Ok(())
}

/// 次フレームをスケジュールする小さなヘルパ。
fn request_animation_frame(f: &Closure<dyn FnMut(f64)>) {
    web_sys::window()
        .expect("no window")
        .request_animation_frame(f.as_ref().unchecked_ref())
        .expect("requestAnimationFrame failed");
}
