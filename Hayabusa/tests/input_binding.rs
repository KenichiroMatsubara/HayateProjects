//! `text-input` の value 束縛 ＋ `on:input`（ADR-0007）の instantiate 経路を、`RecordingSink`
//! 上で実証する。`RecordingSink` はガード無しで全 op を記録するので（差分・非組成中ガードは
//! host=core 側・ADR-0007）、ここでは「読み（`on:input` → signal）」と「書き（signal →
//! `set_value` 発行）」の**配線**そのものを観測する。ガード挙動は core の value_guard_tests と
//! tests/app_host.rs（実 EditState）で検証する。

use hayabusa::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

const INPUT: ElId = ElId(1);

/// `<view><text-input value={draft} on:input={edit}/></view>` を組み立てて instantiate する。
/// `edit` は commit 済みテキスト（payload）を `draft` に書く。
fn build_field() -> (Signal, Rc<RefCell<RecordingSink>>, Instance<RecordingSink>) {
    let rt = Runtime::new();
    let draft = rt.signal(Value::string(""));

    let template = TemplateNode::new(ElementKind::View).child(
        TemplateNode::new(ElementKind::TextInput)
            .bind_value(Expr::var("draft"))
            .on_input(0),
    );

    let scope = Scope::new().with("draft", Binding::Signal(draft.clone()));

    let sink_draft = draft.clone();
    let handlers: Vec<Handler> = vec![Box::new(move |payload: Value| sink_draft.set(payload))];

    let sink = Rc::new(RefCell::new(RecordingSink::new()));
    let app = instantiate(&rt, &template, &scope, handlers, sink.clone());
    (draft, sink, app)
}

#[test]
fn value_binding_emits_initial_set_value() {
    let (_draft, sink, _app) = build_field();
    // value 束縛 Effect の初回実行で、空文字の programmatic set が出る。
    assert_eq!(sink.borrow().value_mutations(), vec![(INPUT, "".to_string())]);
}

#[test]
fn on_input_updates_the_signal_read_path() {
    let (draft, _sink, app) = build_field();

    assert!(app.input(INPUT, "hello"), "text-input has an on:input handler");
    // 「読み・主」：commit 済みテキストが signal に入る。
    assert_eq!(draft.get(), Value::string("hello"));
}

#[test]
fn signal_change_reissues_set_value_write_path() {
    let (_draft, sink, app) = build_field();
    sink.borrow_mut().clear_log();

    // input 確定 → handler が signal を更新 → value 束縛 Effect が set_value を再発行。
    app.input(INPUT, "hi");

    assert_eq!(sink.borrow().value_mutations(), vec![(INPUT, "hi".to_string())]);
    // set_value 以外の構造 mutation は出ない（fine-grained）。
    assert_eq!(sink.borrow().log().len(), 1);
}

#[test]
fn programmatic_clear_emits_set_value() {
    let (draft, sink, _app) = build_field();
    // 先に値を入れてからクリア（submit 後のフォームクリアに相当）。
    draft.set(Value::string("abc"));
    sink.borrow_mut().clear_log();

    draft.set(Value::string(""));

    assert_eq!(sink.borrow().value_mutations(), vec![(INPUT, "".to_string())]);
}

#[test]
fn input_on_non_handler_element_is_a_no_op() {
    let (_draft, _sink, app) = build_field();
    assert!(!app.input(ElId(0), "x"), "the view has no on:input handler");
}
