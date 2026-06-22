//! Tracer bullet（ADR-0006）：カウンタ例を instantiate → bind → fine-grained patch
//! まで通し、**increment 時にテキストノードだけが patch される**ことを実証する。
//!
//! 構成：
//!
//! ```text
//! <view>
//!   <text>{count}</text>     ← reactive 束縛（count に依存）
//!   <button on:click>+1</button>  ← 静的テキスト ＋ 副作用ハンドラ
//! </view>
//! ```

use hayabusa::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

/// カウンタの template / scope / handlers / sink を組み立てて instantiate する。
fn build_counter() -> (
    Runtime,
    Signal,
    Rc<RefCell<RecordingSink>>,
    Instance<RecordingSink>,
) {
    let rt = Runtime::new();
    let count = rt.signal(Value::number(0));

    let template = TemplateNode::new(ElementKind::View)
        .child(TemplateNode::new(ElementKind::Text).bind_text(Expr::var("count")))
        .child(
            TemplateNode::new(ElementKind::Button)
                .text("+1")
                .on_click(0),
        );

    let scope = Scope::new().with("count", Binding::Signal(count.clone()));

    let inc = count.clone();
    let handlers: Vec<Handler> = vec![Box::new(move |_| {
        inc.update(|v| Value::number(v.as_number().unwrap() + 1.0));
    })];

    let sink = Rc::new(RefCell::new(RecordingSink::new()));
    let app = instantiate(&rt, &template, &scope, handlers, sink.clone());
    (rt, count, sink, app)
}

// ElId は instantiate の作成順（深さ優先）で払い出される。
const VIEW: ElId = ElId(0);
const TEXT: ElId = ElId(1);
const BUTTON: ElId = ElId(2);

#[test]
fn instantiation_builds_the_tree_and_renders_initial_text() {
    let (_rt, _count, sink, app) = build_counter();
    let sink = sink.borrow();

    assert_eq!(app.root(), VIEW);

    // ツリーが深さ優先で作られる：各子は build 直後に親へ append される。
    // 初期 text は "0"（束縛 Effect の初回実行）、静的ボタンテキストは "+1"。
    assert_eq!(
        sink.log(),
        &[
            Mutation::Create {
                id: VIEW,
                kind: ElementKind::View
            },
            Mutation::Create {
                id: TEXT,
                kind: ElementKind::Text
            },
            Mutation::SetText {
                id: TEXT,
                text: "0".into()
            },
            Mutation::AppendChild {
                parent: VIEW,
                child: TEXT
            },
            Mutation::Create {
                id: BUTTON,
                kind: ElementKind::Button
            },
            Mutation::SetText {
                id: BUTTON,
                text: "+1".into()
            },
            Mutation::AppendChild {
                parent: VIEW,
                child: BUTTON
            },
            Mutation::SetRoot { id: VIEW },
        ]
    );
}

#[test]
fn clicking_increment_patches_only_the_text_node() {
    let (_rt, _count, sink, app) = build_counter();

    // 初期 instantiate の mutation を捨て、以降の patch だけを観測する。
    sink.borrow_mut().clear_log();

    assert!(app.click(BUTTON), "button has a click handler");

    // tracer bullet の核心：テキストノードだけが patch される。
    // ── view も button も作り直されず、子の付け替えも起きない。
    assert_eq!(
        sink.borrow().log(),
        &[Mutation::SetText {
            id: TEXT,
            text: "1".into()
        }],
        "only the text node is patched on increment"
    );
}

#[test]
fn repeated_increments_keep_patching_only_the_text_node() {
    let (_rt, count, sink, app) = build_counter();
    sink.borrow_mut().clear_log();

    for _ in 0..3 {
        app.click(BUTTON);
    }

    assert_eq!(count.get(), Value::number(3));
    // 3 回のクリック → text への 3 回の patch のみ。他要素への mutation はゼロ。
    assert_eq!(
        sink.borrow().text_mutations(),
        vec![
            (TEXT, "1".to_string()),
            (TEXT, "2".to_string()),
            (TEXT, "3".to_string()),
        ]
    );
    assert_eq!(
        sink.borrow().log().len(),
        3,
        "no create/append/set-root mutations occur after the initial build"
    );
}

#[test]
fn clicking_a_non_handler_element_is_a_no_op() {
    let (_rt, _count, sink, app) = build_counter();
    sink.borrow_mut().clear_log();

    assert!(!app.click(TEXT), "text node has no click handler");
    assert!(
        sink.borrow().log().is_empty(),
        "no mutations from a no-op click"
    );
}
