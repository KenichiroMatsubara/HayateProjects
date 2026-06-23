//! App Host 配線の統合（ADR-0117）：counter を実 `hayate_app_host::AppHost` へ mount し、
//! **App Host が tree を所有する borrowed-tree モデル**で 1 フレームの完全ループを通す。
//!
//! ```text
//! Platform Front (test)  ──tick──▶  AppHost
//!   AppHost.poll_deliveries() ──▶ HayabusaApp::handle(deliveries, &mut tree)
//!     ListenerId → ElId → Instance::click → handler → batch flush
//!       effect → buffering RecordingSink へ Mutation
//!     drain → apply_mutation(&mut tree)   ← フレーム内で借用ツリーへ出し切る
//! ```
//!
//! `tests/hayate_sink.rs` が `HayateSink`（tree 所有）で seam 単体を実証したのに対し、
//! こちらは ADR-0117 の本番経路（App Host 所有・毎フレーム drain・ListenerId ルーティング）を
//! 実 `ElementTree` 上で通す。`feature = "app-host"` 専用。
//!
//! 実行：`cargo test --features app-host --test app_host`

#![cfg(feature = "app-host")]

use hayabusa::prelude::*;
use hayate_app_host::{AppHost, HeadlessSurface};
use hayate_core::{DocumentEventKind, ElementId, Event};
use std::cell::RefCell;
use std::rc::Rc;

// instantiate の作成順（深さ優先）で払い出される ElId。
const TEXT: ElId = ElId(1);
const BUTTON: ElId = ElId(2);

fn text_eid() -> ElementId {
    ElementId::from_u64(TEXT.0)
}
fn button_eid() -> ElementId {
    ElementId::from_u64(BUTTON.0)
}

/// counter（<view><text>{count}</text><button on:click>+1</button></view>）を buffering
/// `RecordingSink` 上で instantiate し、App Host アダプタにして返す。`count` も返す。
fn build_counter_app() -> (Signal, HayabusaApp) {
    let rt = Runtime::new();
    let count = rt.signal(Value::number(0));

    let template = TemplateNode::new(ElementKind::View)
        .child(TemplateNode::new(ElementKind::Text).bind_text(Expr::var("count")))
        .child(TemplateNode::new(ElementKind::Button).text("+1").on_click(0));

    let scope = Scope::new().with("count", Binding::Signal(count.clone()));

    let inc = count.clone();
    let handlers: Vec<Handler> = vec![Box::new(move |_| {
        inc.update(|v| Value::number(v.as_number().unwrap() + 1.0));
    })];

    let sink = Rc::new(RefCell::new(RecordingSink::new()));
    let instance = instantiate(&rt, &template, &scope, handlers, sink);
    (count, HayabusaApp::new(instance))
}

/// 1 フレーム目の tick で遅延 mount が走り、初期構築 mutation が App Host 所有のツリーへ
/// 適用される（text ノードに初期値 "0" が乗る）。
#[test]
fn first_tick_mounts_and_builds_the_app_hosts_tree() {
    let (_count, app) = build_counter_app();
    let mut host = AppHost::new(HeadlessSurface, Box::new(|| {}));
    host.mount(Box::new(app));

    // delivery を積まずに最初のフレームを回す → ensure_mounted がツリーを組む。
    host.tick(0.0);

    assert_eq!(host.tree().element_get_text(text_eid()), "0");
}

/// click を App Host 経由でディスパッチ → 次フレームの handle で handler が走り、
/// 借用ツリーの text ノードが fine-grained に patch される（ADR-0117 全ループ）。
#[test]
fn click_through_app_host_patches_the_text_node() {
    let (_count, app) = build_counter_app();
    let mut host = AppHost::new(HeadlessSurface, Box::new(|| {}));
    host.mount(Box::new(app));

    // フレーム1：mount＋listener 登録。
    host.tick(0.0);
    assert_eq!(host.tree().element_get_text(text_eid()), "0");

    // ボタンへ Click を合成（listener は mount 時に登録済み）。bubble dispatch で
    // delivery が積まれ、次の tick の poll_deliveries で drain される。
    host.tree_mut().dispatch_event(
        DocumentEventKind::Click,
        Event::Click {
            target_id: button_eid(),
            x: 0.0,
            y: 0.0,
        },
    );

    // フレーム2：delivery を handler へルーティング → count 0→1 → text を "1" に patch。
    host.tick(16.0);
    assert_eq!(host.tree().element_get_text(text_eid()), "1");
}

/// 複数フレームに跨る複数 click が、同じ text ノードを patch し続ける。
#[test]
fn repeated_clicks_across_frames_keep_patching_the_text_node() {
    let (count, app) = build_counter_app();
    let mut host = AppHost::new(HeadlessSurface, Box::new(|| {}));
    host.mount(Box::new(app));
    host.tick(0.0);

    let mut ts = 16.0;
    for expected in 1..=3 {
        host.tree_mut().dispatch_event(
            DocumentEventKind::Click,
            Event::Click {
                target_id: button_eid(),
                x: 0.0,
                y: 0.0,
            },
        );
        host.tick(ts);
        ts += 16.0;
        assert_eq!(
            host.tree().element_get_text(text_eid()),
            expected.to_string()
        );
    }
    assert_eq!(count.get(), Value::number(3));
}

/// `.hybs` を build 時 codegen でコンパイルした `generated::counter` を、実 App Host へ
/// mount して click → text patch まで通す。`.hybs`（ADR-0008）→ App Host（ADR-0117）→
/// 実 `ElementTree` の全経路がデモとして繋がることの実証。
#[test]
fn generated_hybs_component_runs_through_the_app_host() {
    use hayabusa::generated::counter;

    let rt = Runtime::new();
    let sink = Rc::new(RefCell::new(RecordingSink::new()));
    let instance = counter::build(&rt, sink);
    let mut host = AppHost::new(HeadlessSurface, Box::new(|| {}));
    host.mount(Box::new(HayabusaApp::new(instance)));

    host.tick(0.0);
    assert_eq!(host.tree().element_get_text(text_eid()), "0");

    host.tree_mut().dispatch_event(
        DocumentEventKind::Click,
        Event::Click {
            target_id: button_eid(),
            x: 0.0,
            y: 0.0,
        },
    );
    host.tick(16.0);
    assert_eq!(host.tree().element_get_text(text_eid()), "1");
}

/// listener が登録されていない要素への click は何もしない（no-op）。
#[test]
fn click_on_unregistered_element_is_a_no_op() {
    let (_count, app) = build_counter_app();
    let mut host = AppHost::new(HeadlessSurface, Box::new(|| {}));
    host.mount(Box::new(app));
    host.tick(0.0);

    // text ノードには click listener を登録していないので delivery は積まれない。
    host.tree_mut().dispatch_event(
        DocumentEventKind::Click,
        Event::Click {
            target_id: text_eid(),
            x: 0.0,
            y: 0.0,
        },
    );
    host.tick(16.0);

    assert_eq!(host.tree().element_get_text(text_eid()), "0");
}
