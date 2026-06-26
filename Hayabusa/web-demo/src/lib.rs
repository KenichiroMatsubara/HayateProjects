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
//! 入力も実経路で通す：canvas 上に合成した Click を `dispatch_event` → `poll_deliveries` で
//! delivery にし、`HayabusaApp` の handler（`+1`）が signal を更新、Memo→Effect が text を
//! patch して再 present する。AppHost::tick と同型のループ。

use std::cell::RefCell;
use std::rc::Rc;

use hayabusa::prelude::{
    instantiate, Binding, Display, ElementKind, Expr, FlexDirection, Handler, HayabusaApp, Length,
    RecordingSink, Rgba, Runtime, Scope, StyleProp, TemplateNode, Value,
};
use hayate_adapter_web::HayateElementRenderer;
use hayate_app_host::DeliverySink;
use hayate_core::{DocumentEventKind, ElementId, Event};
use wasm_bindgen::prelude::*;
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

    // click ターゲット（= button）の ElId を mount 前に控えておく。
    let click_targets = instance.click_target_ids();
    let mut app = HayabusaApp::new(instance);

    // ── 2. adapter-web：canvas 上に CPU(tiny-skia) present レンダラ ───────────
    let mut renderer = HayateElementRenderer::init(canvas).await?;
    renderer.register_font_bytes(DEFAULT_FONT_FAMILY, NOTO_SANS_JP);
    renderer.set_background_color(0.043, 0.063, 0.125); // #0b1020

    // ── 3. in-process projection：初期構築 mutation を借用ツリーへ drain（root 設定込み）─
    DeliverySink::handle(&mut app, &[], renderer.tree_mut());

    // ── 4. 実入力経路で +1 を 3 回：合成 Click → delivery → handler → reactive → patch ─
    if let Some(&button) = click_targets.first() {
        for _ in 0..3 {
            let tree = renderer.tree_mut();
            tree.dispatch_event(
                DocumentEventKind::Click,
                Event::Click {
                    target_id: ElementId::from_u64(button.0),
                    x: 0.0,
                    y: 0.0,
                },
            );
            let deliveries = tree.poll_deliveries();
            DeliverySink::handle(&mut app, &deliveries, renderer.tree_mut());
        }
    }

    // ── 5. present：tree → layout → SceneGraph → tiny-skia → canvas ───────────
    for frame in 0..3 {
        renderer.render(frame as f64 * 16.0)?;
    }

    // 将来のフレーム駆動・ライブ入力配線の足場として renderer を保持（drop 回避）。
    Box::leak(Box::new(renderer));
    Ok(())
}
