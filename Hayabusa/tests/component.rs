//! コンポーネント合成のスライス（ADR-0004）：prop 反応性・emit ルーティング・
//! インスタンス隔離・lifecycle（on_mount / on_destroy）を検証する。

use hayabusa::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

fn count_kind(log: &[Mutation], pred: impl Fn(&Mutation) -> bool) -> usize {
    log.iter().filter(|m| pred(m)).count()
}

/// ローカル count を持つカウンタコンポーネント（prop なし）。
/// `<view><text>{count}</text><button on:click>+1</button></view>`
fn local_counter() -> Rc<Component> {
    Component::new(vec![], |cx| {
        let rt = cx.rt().clone();
        let count = rt.signal(Value::number(0));
        let c = count.clone();
        let handlers: Vec<Handler> = vec![Box::new(move |_| {
            c.update(|v| Value::number(v.as_number().unwrap() + 1.0))
        })];
        let scope = Scope::new().with("count", Binding::Signal(count));
        let template = TemplateNode::new(ElementKind::View)
            .child(TemplateNode::new(ElementKind::Text).bind_text(Expr::var("count")))
            .child(
                TemplateNode::new(ElementKind::Button)
                    .text("+1")
                    .on_click(0),
            );
        ComponentView {
            scope,
            handlers,
            template,
        }
    })
}

// ---------------------------------------------------------------------------
// prop 反応性
// ---------------------------------------------------------------------------

#[test]
fn prop_is_reactive_from_parent_scope() {
    let rt = Runtime::new();
    // 子: prop "label" を表示するだけ。
    let label_comp = Component::new(vec!["label".to_string()], |cx| {
        let scope = Scope::new().with("label", cx.prop("label"));
        let template = TemplateNode::new(ElementKind::View)
            .child(TemplateNode::new(ElementKind::Text).bind_text(Expr::var("label")));
        ComponentView {
            scope,
            handlers: vec![],
            template,
        }
    });

    // 親: signal `name` を prop label として子へ渡す。
    let name = rt.signal(Value::string("Alice"));
    let template = TemplateNode::new(ElementKind::View)
        .child(ComponentSlot::new(label_comp).prop("label", Expr::var("name")));
    let scope = Scope::new().with("name", Binding::Signal(name.clone()));
    let sink = Rc::new(RefCell::new(RecordingSink::new()));
    let _app = instantiate(&rt, &template, &scope, vec![], sink.clone());

    // 親 view=0, 子 view=1, 子 text=2。
    let child_text = ElId(2);
    assert_eq!(
        sink.borrow().text_mutations(),
        vec![(child_text, "Alice".into())]
    );

    // 親 signal を変えると子の prop binding が fine-grained に patch される。
    sink.borrow_mut().clear_log();
    name.set(Value::string("Bob"));
    assert_eq!(
        sink.borrow().text_mutations(),
        vec![(child_text, "Bob".into())]
    );
}

// ---------------------------------------------------------------------------
// emit ルーティング
// ---------------------------------------------------------------------------

#[test]
fn child_emit_routes_to_parent_handler() {
    let rt = Runtime::new();
    // 子: ボタンクリックで "bump" を emit する。
    let bumper = Component::new(vec![], |cx| {
        let emit = cx.emitter();
        let handlers: Vec<Handler> = vec![Box::new(move |_| emit.emit("bump", Value::number(1)))];
        let template = TemplateNode::new(ElementKind::View).child(
            TemplateNode::new(ElementKind::Button)
                .text("bump")
                .on_click(0),
        );
        ComponentView {
            scope: Scope::new(),
            handlers,
            template,
        }
    });

    // 親: count を表示し、子の "bump" を受けて count を payload ぶん増やす。
    let count = rt.signal(Value::number(0));
    let c = count.clone();
    let parent_handlers: Vec<Handler> = vec![Box::new(move |payload: Value| {
        let delta = payload.as_number().unwrap_or(0.0);
        c.update(|v| Value::number(v.as_number().unwrap() + delta));
    })];
    let template = TemplateNode::new(ElementKind::View)
        .child(TemplateNode::new(ElementKind::Text).bind_text(Expr::var("count")))
        .child(ComponentSlot::new(bumper).on("bump", 0));
    let scope = Scope::new().with("count", Binding::Signal(count.clone()));
    let sink = Rc::new(RefCell::new(RecordingSink::new()));
    let app = instantiate(&rt, &template, &scope, parent_handlers, sink.clone());

    // 親 view=0, 親 text=1, 子 view=2, 子 button=3。
    let parent_text = ElId(1);
    let child_button = ElId(3);

    sink.borrow_mut().clear_log();
    assert!(app.click(child_button));
    // 子の emit が親ハンドラを起動し、親の count text が patch される。
    assert_eq!(
        sink.borrow().text_mutations(),
        vec![(parent_text, "1".into())]
    );
    assert_eq!(count.get(), Value::number(1));
}

// ---------------------------------------------------------------------------
// インスタンス隔離
// ---------------------------------------------------------------------------

#[test]
fn instances_have_isolated_state() {
    let rt = Runtime::new();
    let comp = local_counter();
    let template = TemplateNode::new(ElementKind::View)
        .child(ComponentSlot::new(comp.clone()))
        .child(ComponentSlot::new(comp));
    let sink = Rc::new(RefCell::new(RecordingSink::new()));
    let app = instantiate(&rt, &template, &Scope::new(), vec![], sink.clone());

    // A: view=1 text=2 button=3 / B: view=4 text=5 button=6。
    let (a_text, a_btn) = (ElId(2), ElId(3));
    let (b_text, b_btn) = (ElId(5), ElId(6));

    sink.borrow_mut().clear_log();
    app.click(a_btn);
    app.click(a_btn);
    app.click(b_btn);

    // A は 2 回、B は 1 回 — 各インスタンスの count は独立。
    assert_eq!(
        sink.borrow().text_mutations(),
        vec![
            (a_text, "1".into()),
            (a_text, "2".into()),
            (b_text, "1".into())
        ]
    );
}

// ---------------------------------------------------------------------------
// lifecycle（on_mount / on_destroy）
// ---------------------------------------------------------------------------

#[test]
fn lifecycle_runs_on_mount_and_on_destroy() {
    let rt = Runtime::new();
    let log: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));

    let log_for_comp = log.clone();
    let comp = Component::new(vec![], move |cx| {
        let l1 = log_for_comp.clone();
        cx.on_mount(move || l1.borrow_mut().push("mount".to_string()));
        let l2 = log_for_comp.clone();
        cx.on_destroy(move || l2.borrow_mut().push("destroy".to_string()));
        ComponentView {
            scope: Scope::new(),
            handlers: vec![],
            template: TemplateNode::new(ElementKind::Text).text("alive"),
        }
    });

    // :if の body（view）の中にコンポーネントを置く。
    let show = rt.signal(Value::Bool(false));
    let body = TemplateNode::new(ElementKind::View).child(ComponentSlot::new(comp));
    let template =
        TemplateNode::new(ElementKind::View).child(IfBlock::new(Expr::var("show"), body));
    let scope = Scope::new().with("show", Binding::Signal(show.clone()));
    let sink = Rc::new(RefCell::new(RecordingSink::new()));
    let _app = instantiate(&rt, &template, &scope, vec![], sink.clone());

    assert!(log.borrow().is_empty(), "not mounted yet");

    show.set(Value::Bool(true));
    assert_eq!(*log.borrow(), vec!["mount".to_string()]);

    show.set(Value::Bool(false));
    assert_eq!(
        *log.borrow(),
        vec!["mount".to_string(), "destroy".to_string()],
        "on_destroy fires when the enclosing branch is torn down"
    );
}

// ---------------------------------------------------------------------------
// :each の中のコンポーネント
// ---------------------------------------------------------------------------

#[test]
fn components_in_each_are_disposed_per_row() {
    let rt = Runtime::new();
    let destroyed: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));

    let d = destroyed.clone();
    let comp = Component::new(vec!["id".to_string()], move |cx| {
        // 破棄時に自分の id を記録する。
        let id_val = cx.prop("id").current();
        let d2 = d.clone();
        cx.on_destroy(move || d2.borrow_mut().push(id_val.to_display_string()));
        let scope = Scope::new().with("id", cx.prop("id"));
        ComponentView {
            scope,
            handlers: vec![],
            template: TemplateNode::new(ElementKind::Text).bind_text(Expr::var("id")),
        }
    });

    let items = rt.signal(Value::list([Value::number(1), Value::number(2)]));
    // 行 body（view）の中にコンポーネントを置き、prop id に item を渡す。
    let row = TemplateNode::new(ElementKind::View)
        .child(ComponentSlot::new(comp).prop("id", Expr::var("item")));
    let template = TemplateNode::new(ElementKind::View).child(EachBlock::new(
        Expr::var("items"),
        "item",
        Expr::var("item"),
        row,
    ));
    let scope = Scope::new().with("items", Binding::Signal(items.clone()));
    let sink = Rc::new(RefCell::new(RecordingSink::new()));
    let _app = instantiate(&rt, &template, &scope, vec![], sink.clone());

    // id=1 を消すと、その行のコンポーネントだけ on_destroy が走る。
    sink.borrow_mut().clear_log();
    rt.batch(|| items.set(Value::list([Value::number(2)])));

    assert_eq!(*destroyed.borrow(), vec!["1".to_string()]);
    // 残り行（id=2）は再生成されない。
    assert_eq!(
        count_kind(sink.borrow().log(), |m| matches!(
            m,
            Mutation::Create { .. }
        )),
        0,
        "surviving row's component is not recreated"
    );
    assert!(
        count_kind(sink.borrow().log(), |m| matches!(
            m,
            Mutation::Remove { .. }
        )) >= 1,
        "dropped row is removed"
    );
}
