//! Template IR の instantiate / bind / fine-grained patch（ADR-0004 / ADR-0006）。
//!
//! ランタイムが Template IR を instantiate（要素を sink に作る）し、reactive binding を
//! Effect として配線する。binding は **純粋式 ＋ target（element, prop）**（ADR-0004）で、
//! 評価が読んだ signal だけに依存するため、count が変わったとき再走するのは
//! `{count}` を束ねた text 束縛 Effect **だけ**になる（fine-grained patch）。
//! ハンドラ（副作用の本体）は signal を触るだけで sink には触れない。
//!
//! sink は `Rc<RefCell<S>>` で共有し、binding Effect がそこへ書き込む。flush 合体
//! （ADR-0003）により、1 クリック内の複数 write は 1 回の patch にまとまる。

use crate::expr::Scope;
use crate::reactive::Runtime;
use crate::sink::{ElId, ElementSink};
use crate::template::{HandlerId, TemplateNode};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// 副作用ハンドラの本体（script の代役。tracer bullet では Rust クロージャ）。
pub type Handler = Box<dyn FnMut()>;

/// instantiate 済みのコンポーネントインスタンス。
///
/// クリック等のインタラクションの入口を持ち、ハンドラ実行 → flush までを駆動する。
pub struct Instance<S: ElementSink> {
    rt: Runtime,
    sink: Rc<RefCell<S>>,
    handlers: Vec<Rc<RefCell<Handler>>>,
    /// 要素 → その click ハンドラ。
    click_targets: HashMap<ElId, HandlerId>,
    root: ElId,
}

impl<S: ElementSink + 'static> Instance<S> {
    /// ルート要素のハンドル。
    pub fn root(&self) -> ElId {
        self.root
    }

    /// 共有 sink のハンドル（テストでの記録参照用）。
    pub fn sink(&self) -> Rc<RefCell<S>> {
        self.sink.clone()
    }

    /// 要素のクリックをディスパッチする。ハンドラ本体を batch で走らせ、その中の
    /// signal write を 1 回の flush（fine-grained patch）にまとめて sink へ落とす。
    ///
    /// 対象に click ハンドラが無ければ何もしない（`true` を返したときだけ起動）。
    pub fn click(&self, target: ElId) -> bool {
        let Some(&handler_id) = self.click_targets.get(&target) else {
            return false;
        };
        let handler = self.handlers[handler_id].clone();
        self.rt.batch(|| {
            (handler.borrow_mut())();
        });
        true
    }
}

/// Template IR を instantiate し、binding を配線して `Instance` を返す。
///
/// - `template`：手組み Template IR（ADR-0006）
/// - `scope`：式評価のスコープ（component の signal / prop / `:each` item）
/// - `handlers`：`on_click` 等が参照する副作用本体（script の代役）
/// - `sink`：mutation を受ける element ツリー（共有）
pub fn instantiate<S: ElementSink + 'static>(
    rt: &Runtime,
    template: &TemplateNode,
    scope: &Scope,
    handlers: Vec<Handler>,
    sink: Rc<RefCell<S>>,
) -> Instance<S> {
    let handlers: Vec<Rc<RefCell<Handler>>> = handlers
        .into_iter()
        .map(|h| Rc::new(RefCell::new(h)))
        .collect();
    let mut click_targets = HashMap::new();

    let root = build_node(rt, template, scope, &sink, &mut click_targets);
    sink.borrow_mut().set_root(root);

    Instance {
        rt: rt.clone(),
        sink,
        handlers,
        click_targets,
        root,
    }
}

/// 1 ノードを再帰的に instantiate する。text 束縛は Effect として配線する。
fn build_node<S: ElementSink + 'static>(
    rt: &Runtime,
    node: &TemplateNode,
    scope: &Scope,
    sink: &Rc<RefCell<S>>,
    click_targets: &mut HashMap<ElId, HandlerId>,
) -> ElId {
    let id = sink.borrow_mut().create_element(node.kind);

    // 静的テキスト（束縛なし）は 1 回だけ書く。
    if let Some(text) = &node.static_text {
        sink.borrow_mut().set_text(id, text);
    }

    // reactive text 束縛：純粋式を読む Effect を張る。読んだ signal だけに依存し、
    // 変化時はこの要素の text だけを patch する（fine-grained）。
    if let Some(expr) = &node.text_binding {
        let expr = expr.clone();
        let scope = scope.clone();
        let sink = sink.clone();
        rt.effect(move || {
            let value = expr.eval(&scope);
            sink.borrow_mut().set_text(id, &value.to_display_string());
        });
    }

    if let Some(handler_id) = node.on_click {
        click_targets.insert(id, handler_id);
    }

    for child in &node.children {
        let child_id = build_node(rt, child, scope, sink, click_targets);
        sink.borrow_mut().append_child(id, child_id);
    }

    id
}
