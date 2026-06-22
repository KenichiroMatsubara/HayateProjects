//! 構造 reconcile のスライス（ADR-0004）：`:if` の mount/teardown と `:each` の
//! keyed reconcile（in-place 値更新・追加/削除・move）を検証する。

use hayabusa::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

fn count_kind(log: &[Mutation], pred: impl Fn(&Mutation) -> bool) -> usize {
    log.iter().filter(|m| pred(m)).count()
}

// ---------------------------------------------------------------------------
// :if
// ---------------------------------------------------------------------------

#[test]
fn if_mounts_and_unmounts_between_static_siblings() {
    let rt = Runtime::new();
    let show = rt.signal(Value::Bool(false));

    // <view> "A" {:if show}<text>B</text>{/if} "C" </view>
    let template = TemplateNode::new(ElementKind::View)
        .child(TemplateNode::new(ElementKind::Text).text("A"))
        .child(IfBlock::new(
            Expr::var("show"),
            TemplateNode::new(ElementKind::Text).text("B"),
        ))
        .child(TemplateNode::new(ElementKind::Text).text("C"));

    let scope = Scope::new().with("show", Binding::Signal(show.clone()));
    let sink = Rc::new(RefCell::new(RecordingSink::new()));
    let _app = instantiate(&rt, &template, &scope, vec![], sink.clone());

    // 初期: A=1, C=2（if は falsy で未 mount）。
    let a = ElId(1);
    let c = ElId(2);
    assert_eq!(
        sink.borrow().text_mutations(),
        vec![(a, "A".into()), (c, "C".into())]
    );

    // mount: 兄弟 C の直前に B を挿入する。
    sink.borrow_mut().clear_log();
    show.set(Value::Bool(true));
    let b = ElId(3);
    assert_eq!(
        sink.borrow().log(),
        &[
            Mutation::Create {
                id: b,
                kind: ElementKind::Text
            },
            Mutation::SetText {
                id: b,
                text: "B".into()
            },
            Mutation::InsertBefore {
                parent: ElId(0),
                child: b,
                before: c
            },
        ]
    );

    // unmount: B を除去する。
    sink.borrow_mut().clear_log();
    show.set(Value::Bool(false));
    assert_eq!(sink.borrow().log(), &[Mutation::Remove { id: b }]);
}

#[test]
fn if_teardown_disposes_the_branch_effect() {
    let rt = Runtime::new();
    let show = rt.signal(Value::Bool(true));
    let msg = rt.signal(Value::string("hi"));

    let template = TemplateNode::new(ElementKind::View).child(IfBlock::new(
        Expr::var("show"),
        TemplateNode::new(ElementKind::Text).bind_text(Expr::var("msg")),
    ));

    let scope = Scope::new()
        .with("show", Binding::Signal(show.clone()))
        .with("msg", Binding::Signal(msg.clone()));
    let sink = Rc::new(RefCell::new(RecordingSink::new()));
    let _app = instantiate(&rt, &template, &scope, vec![], sink.clone());

    let body = ElId(1);
    // 表示中は msg 変化が body を patch する。
    sink.borrow_mut().clear_log();
    msg.set(Value::string("yo"));
    assert_eq!(sink.borrow().text_mutations(), vec![(body, "yo".into())]);

    // 非表示にすると body 除去 ＋ ブランチ Effect の dispose。
    sink.borrow_mut().clear_log();
    show.set(Value::Bool(false));
    assert_eq!(sink.borrow().log(), &[Mutation::Remove { id: body }]);

    // teardown 済みなので msg を変えても何も patch されない。
    sink.borrow_mut().clear_log();
    msg.set(Value::string("ghost"));
    assert!(
        sink.borrow().log().is_empty(),
        "disposed branch effect must not patch"
    );
}

// ---------------------------------------------------------------------------
// :each（keyed-only）
// ---------------------------------------------------------------------------

fn item(id: i64, label: &str) -> Value {
    Value::record([
        ("id".to_string(), Value::number(id as f64)),
        ("label".to_string(), Value::string(label)),
    ])
}

/// <view>{:each items by item.id}<text>{item.label}</text>{/each}</view>
fn each_app() -> (Runtime, Signal, Rc<RefCell<RecordingSink>>) {
    let rt = Runtime::new();
    let items = rt.signal(Value::list([item(1, "one"), item(2, "two")]));

    let template = TemplateNode::new(ElementKind::View).child(EachBlock::new(
        Expr::var("items"),
        "item",
        Expr::var("item").member("id"),
        TemplateNode::new(ElementKind::Text).bind_text(Expr::var("item").member("label")),
    ));

    let scope = Scope::new().with("items", Binding::Signal(items.clone()));
    let sink = Rc::new(RefCell::new(RecordingSink::new()));
    let _app = instantiate(&rt, &template, &scope, vec![], sink.clone());
    (rt, items, sink)
}

#[test]
fn each_renders_initial_rows() {
    let (_rt, _items, sink) = each_app();
    // 2 行ぶんのテキストが描かれる。
    let texts = sink.borrow().text_mutations();
    assert_eq!(
        texts,
        vec![(ElId(1), "one".into()), (ElId(2), "two".into())]
    );
}

#[test]
fn each_same_key_value_update_patches_only_that_row_in_place() {
    let (rt, items, sink) = each_app();
    sink.borrow_mut().clear_log();

    // id=2 のラベルだけ変える（キー順は不変）。
    rt.batch(|| items.set(Value::list([item(1, "one"), item(2, "TWO")])));

    // 行は再生成されず、変わった行の text だけが in-place patch される。
    assert_eq!(
        sink.borrow().text_mutations(),
        vec![(ElId(2), "TWO".into())]
    );
    assert_eq!(
        count_kind(sink.borrow().log(), |m| matches!(
            m,
            Mutation::Create { .. }
        )),
        0
    );
    assert_eq!(
        count_kind(sink.borrow().log(), |m| matches!(
            m,
            Mutation::Remove { .. }
        )),
        0
    );
    assert_eq!(
        count_kind(sink.borrow().log(), |m| matches!(
            m,
            Mutation::InsertBefore { .. }
        )),
        0,
        "value-only update must not reorder"
    );
}

#[test]
fn each_append_creates_only_the_new_row() {
    let (rt, items, sink) = each_app();
    sink.borrow_mut().clear_log();

    rt.batch(|| {
        items.set(Value::list([
            item(1, "one"),
            item(2, "two"),
            item(3, "three"),
        ]))
    });

    let log = sink.borrow();
    // 新しい行が 1 つだけ作られ、既存行は再生成されない。
    assert_eq!(
        count_kind(log.log(), |m| matches!(m, Mutation::Create { .. })),
        1
    );
    assert_eq!(
        count_kind(log.log(), |m| matches!(m, Mutation::Remove { .. })),
        0
    );
    assert!(log.text_mutations().iter().any(|(_, t)| t == "three"));
}

#[test]
fn each_remove_disposes_only_the_dropped_row() {
    let (rt, items, sink) = each_app();
    sink.borrow_mut().clear_log();

    // id=1 を消す。
    rt.batch(|| items.set(Value::list([item(2, "two")])));

    let log = sink.borrow();
    assert_eq!(
        count_kind(log.log(), |m| matches!(m, Mutation::Remove { .. })),
        1
    );
    assert_eq!(
        count_kind(log.log(), |m| matches!(m, Mutation::Create { .. })),
        0
    );
    // 消えるのは行 1（ElId(1)）。
    assert!(log
        .log()
        .iter()
        .any(|m| matches!(m, Mutation::Remove { id } if *id == ElId(1))));
}

#[test]
fn each_reorder_moves_rows_without_recreating() {
    let (rt, items, sink) = each_app();
    sink.borrow_mut().clear_log();

    // 順序を入れ替える（同一キー・同一ラベル）。
    rt.batch(|| items.set(Value::list([item(2, "two"), item(1, "one")])));

    let log = sink.borrow();
    // 再生成も削除も無く、move（InsertBefore）だけが起きる。値は変わらないので
    // text の patch も無い。
    assert_eq!(
        count_kind(log.log(), |m| matches!(m, Mutation::Create { .. })),
        0
    );
    assert_eq!(
        count_kind(log.log(), |m| matches!(m, Mutation::Remove { .. })),
        0
    );
    assert!(
        count_kind(log.log(), |m| matches!(m, Mutation::InsertBefore { .. })) > 0,
        "reorder must move rows"
    );
    assert!(
        log.text_mutations().is_empty(),
        "reorder with unchanged values must not re-render text"
    );
}
