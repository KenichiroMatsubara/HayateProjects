//! 式パーサ → instantiate の結線（ADR-0004）。テキストで書いた純粋式が binding /
//! `:if` / `:each` の key として動くことを確かめる。

use hayabusa::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn parsed_expression_drives_a_reactive_text_binding() {
    let rt = Runtime::new();
    let count = rt.signal(Value::number(1));

    // テキストで書いた式 `"count = " + count * 2` を束縛する。
    let expr = Expr::parse("\"count = \" + count * 2").unwrap();
    let template = TemplateNode::new(ElementKind::View)
        .child(TemplateNode::new(ElementKind::Text).bind_text(expr));
    let scope = Scope::new().with("count", Binding::Signal(count.clone()));
    let sink = Rc::new(RefCell::new(RecordingSink::new()));
    let _app = instantiate(&rt, &template, &scope, vec![], sink.clone());

    let text = ElId(1);
    assert_eq!(
        sink.borrow().text_mutations(),
        vec![(text, "count = 2".into())]
    );

    // signal が変われば、式が再評価されてその text だけが patch される。
    sink.borrow_mut().clear_log();
    count.set(Value::number(5));
    assert_eq!(
        sink.borrow().text_mutations(),
        vec![(text, "count = 10".into())]
    );
}

#[test]
fn parsed_condition_drives_if() {
    let rt = Runtime::new();
    let n = rt.signal(Value::number(0));

    let template = TemplateNode::new(ElementKind::View).child(IfBlock::new(
        Expr::parse("n > 3").unwrap(),
        TemplateNode::new(ElementKind::Text).text("big"),
    ));
    let scope = Scope::new().with("n", Binding::Signal(n.clone()));
    let sink = Rc::new(RefCell::new(RecordingSink::new()));
    let _app = instantiate(&rt, &template, &scope, vec![], sink.clone());

    // 0 > 3 は false → 未 mount。
    assert!(sink.borrow().text_mutations().is_empty());

    n.set(Value::number(5)); // 5 > 3 → mount
    assert!(sink
        .borrow()
        .text_mutations()
        .iter()
        .any(|(_, t)| t == "big"));
}

#[test]
fn parsed_key_expression_drives_each() {
    let rt = Runtime::new();
    let mk = |id: i64, label: &str| {
        Value::record([
            ("id".into(), Value::number(id as f64)),
            ("label".into(), Value::string(label)),
        ])
    };
    let items = rt.signal(Value::list([mk(1, "a"), mk(2, "b")]));

    let template = TemplateNode::new(ElementKind::View).child(EachBlock::new(
        Expr::parse("items").unwrap(),
        "item",
        Expr::parse("item.id").unwrap(),
        TemplateNode::new(ElementKind::Text).bind_text(Expr::parse("item.label").unwrap()),
    ));
    let scope = Scope::new().with("items", Binding::Signal(items.clone()));
    let sink = Rc::new(RefCell::new(RecordingSink::new()));
    let _app = instantiate(&rt, &template, &scope, vec![], sink.clone());

    let texts = sink.borrow().text_mutations();
    assert_eq!(texts, vec![(ElId(1), "a".into()), (ElId(2), "b".into())]);
}
