//! static style（ADR-0010）の instantiate 経路を `RecordingSink` 上で実証する。スタイルは
//! reactive 束縛ではなく instantiate 時に **一度だけ** `set_style` で適用される（`bind_text` の
//! ような Effect は張られない）。core への写像は tests/app_host.rs（実 `ElementTree` の layout）で
//! 検証する。

use hayabusa::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

const ROOT: ElId = ElId(0);

#[test]
fn static_style_is_applied_once_at_build() {
    let rt = Runtime::new();
    let style = vec![
        StyleProp::Width(Length::Px(120.0)),
        StyleProp::Padding(Length::Px(8.0)),
        StyleProp::Display(Display::Flex),
        StyleProp::FlexDirection(FlexDirection::Column),
        StyleProp::BackgroundColor(Rgba::new(1.0, 1.0, 1.0, 1.0)),
    ];
    let template = TemplateNode::new(ElementKind::View).style(style.clone());

    let sink = Rc::new(RefCell::new(RecordingSink::new()));
    let _app = instantiate(&rt, &template, &Scope::new(), Vec::new(), sink.clone());

    assert_eq!(sink.borrow().style_mutations(), vec![(ROOT, style)]);
}

#[test]
fn no_style_emits_no_set_style() {
    let rt = Runtime::new();
    let template = TemplateNode::new(ElementKind::View).child(TemplateNode::new(ElementKind::Text));

    let sink = Rc::new(RefCell::new(RecordingSink::new()));
    let _app = instantiate(&rt, &template, &Scope::new(), Vec::new(), sink.clone());

    assert!(sink.borrow().style_mutations().is_empty());
}

#[test]
fn static_style_does_not_react_to_signals() {
    // スタイルは static：signal を変えても再適用（追加の set_style）は起きない。
    let rt = Runtime::new();
    let count = rt.signal(Value::number(0));
    let template = TemplateNode::new(ElementKind::View)
        .style(vec![StyleProp::Height(Length::Px(10.0))])
        .child(TemplateNode::new(ElementKind::Text).bind_text(Expr::var("count")));
    let scope = Scope::new().with("count", Binding::Signal(count.clone()));

    let sink = Rc::new(RefCell::new(RecordingSink::new()));
    let _app = instantiate(&rt, &template, &scope, Vec::new(), sink.clone());
    sink.borrow_mut().clear_log();

    count.set(Value::number(1)); // text は再評価されるが style は不変。

    assert!(
        sink.borrow().style_mutations().is_empty(),
        "static style must not re-apply on signal change"
    );
}
