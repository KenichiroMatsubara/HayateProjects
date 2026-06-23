//! 実機コア統合（Hayabusa ADR-0009）：counter tracer bullet を **実際の
//! `hayate_core::ElementTree`** 上で通し、`HayateSink` 経由の fine-grained patch が
//! Element Layer に届くことを実証する。`tests/counter.rs` の `RecordingSink` 版と対をなす
//! ――同じ instantiate / bind / click 経路を、観測用ログではなく実ツリーで検証する。
//!
//! `feature = "hayate-core"` 専用（既定ビルドではコンパイルされない）。
//! 実行：`cargo test --features hayate-core --test hayate_sink`

#![cfg(feature = "hayate-core")]

use hayabusa::prelude::*;
use hayate_core::ElementId;
use std::cell::RefCell;
use std::rc::Rc;

/// counter の template / scope / handlers を `HayateSink` 上で instantiate する。
fn build_counter() -> (Signal, Rc<RefCell<HayateSink>>, Instance<HayateSink>) {
    let rt = Runtime::new();
    let count = rt.signal(Value::number(0));

    // <view><text>{count}</text><button on:click>+1</button></view>
    let template = TemplateNode::new(ElementKind::View)
        .child(TemplateNode::new(ElementKind::Text).bind_text(Expr::var("count")))
        .child(TemplateNode::new(ElementKind::Button).text("+1").on_click(0));

    let scope = Scope::new().with("count", Binding::Signal(count.clone()));

    let inc = count.clone();
    let handlers: Vec<Handler> = vec![Box::new(move |_| {
        inc.update(|v| Value::number(v.as_number().unwrap() + 1.0));
    })];

    let sink = Rc::new(RefCell::new(HayateSink::new()));
    let app = instantiate(&rt, &template, &scope, handlers, sink.clone());
    (count, sink, app)
}

// instantiate の作成順（深さ優先）で払い出される ElId。
const VIEW: ElId = ElId(0);
const TEXT: ElId = ElId(1);
const BUTTON: ElId = ElId(2);

fn core_id(id: ElId) -> ElementId {
    ElementId::from_u64(id.0)
}

#[test]
fn instantiation_builds_a_real_element_tree() {
    let (_count, sink, app) = build_counter();
    assert_eq!(app.root(), VIEW);

    let sink = sink.borrow();
    let tree = sink.tree();
    // 束縛 Effect の初回実行で text ノードに "0" が乗っている。
    assert_eq!(tree.element_get_text(core_id(TEXT)), "0");
}

#[test]
fn clicking_increment_patches_the_text_node_in_the_real_tree() {
    let (_count, sink, app) = build_counter();

    assert!(app.click(BUTTON), "button has a click handler");

    // increment 後、実 ElementTree の text ノードが "1" に patch されている。
    assert_eq!(sink.borrow().tree().element_get_text(core_id(TEXT)), "1");
}

/// 実機コア固有の意味論（host-ABI 線・ADR-0002）の記録：`hayate_core` の
/// `element_set_text` は text-like 要素（`Text` / `TextInput`）にのみ適用し、`Button` への
/// set は no-op になる（buttons はラベルを子 `text` 要素で持つ）。tracer-bullet の
/// `RecordingSink` は kind を問わず記録するため、ここに差が出る。Button ラベルを実コアで
/// 出すには子 text ノードを置く必要がある（後続テンプレ／コンパイラの責務）。
#[test]
fn button_label_set_text_is_a_no_op_in_core() {
    let (_count, sink, _app) = build_counter();
    assert_eq!(
        sink.borrow().tree().element_get_text(core_id(BUTTON)),
        "",
        "core treats a button as non-text-like; its label belongs on a child text node"
    );
}

#[test]
fn repeated_increments_keep_patching_the_same_text_node() {
    let (count, sink, app) = build_counter();

    for _ in 0..3 {
        app.click(BUTTON);
    }

    assert_eq!(count.get(), Value::number(3));
    assert_eq!(sink.borrow().tree().element_get_text(core_id(TEXT)), "3");
}

#[test]
fn clicking_a_non_handler_element_leaves_the_tree_unchanged() {
    let (_count, sink, app) = build_counter();

    assert!(!app.click(TEXT), "text node has no click handler");
    assert_eq!(sink.borrow().tree().element_get_text(core_id(TEXT)), "0");
}
