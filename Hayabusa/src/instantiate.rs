//! Template IR の instantiate / bind / fine-grained patch（ADR-0004 / ADR-0006）。
//!
//! ランタイムが Template IR を instantiate（要素を sink に作る）し、reactive binding を
//! Effect として配線する。**構造の reconcile（要素の追加・削除・並べ替え）もランタイムが
//! 所有する**（ADR-0004）。
//!
//! - **binding**：純粋式 ＋ target（element, prop）。読んだ signal だけに依存するため、
//!   値変化時はその text ノードだけが patch される（fine-grained）。
//! - **`:if`**：条件 signal を追跡する構造 Effect。truthy↔falsy で body の Scope を
//!   mount / teardown する。
//! - **`:each`**：keyed-only。同一キーの値更新は行の item signal を in-place patch、
//!   並べ替えは move（再生成しない）、追加/削除は行 Scope の生成/破棄。
//! - **コンポーネント**：インスタンスごとに独立した Scope・prop 入力・emit 出力・
//!   lifecycle を持つ（component.rs / ADR-0004）。
//!
//! 兄弟の静的要素・`:if`・`:each`・コンポーネントが同じ親に混在しても正しい位置へ
//! 挿入できるよう、親要素ごとに [`ChildList`]（スロット順 ＋ 各スロットの現在 ElId）を
//! 持ち、動的ブロックは自分のスロットの「次の兄弟」を anchor として `insert_before`
//! する（marker ノード不要）。
//!
//! ハンドラは ElId → 解決済みクロージャの登録簿（`click_targets`）に入れる。コンポーネント
//! インスタンスは自分の handler 集合を持つため、build 時に解決して登録する。

use crate::component::{Component, ComponentSlot, Emit, Handler, SetupCx};
use crate::expr::{Binding, Scope};
use crate::reactive::{Runtime, ScopeId};
use crate::sink::{ElId, ElementSink};
use crate::template::{EachBlock, IfBlock, Template, TemplateNode};
use crate::value::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// build 時に引き回す、現在の handler 集合（最上位 or コンポーネントインスタンスごと）。
type Handlers = Rc<Vec<Rc<RefCell<Handler>>>>;

/// click ハンドラの登録簿（要素 → 解決済みハンドラ）。動的に mount される行/ブランチ/
/// コンポーネントの要素も登録できるよう共有する。
type ClickTargets = Rc<RefCell<HashMap<ElId, Rc<RefCell<Handler>>>>>;

/// 親要素の子スロット管理。各スロットは 1 つの子位置が現在寄与している top-level ElId
/// 列を順に保持する。動的ブロックはここから anchor を引いて `insert_before` する。
struct ChildList {
    parent: ElId,
    slots: Vec<Vec<ElId>>,
}

impl ChildList {
    fn new(parent: ElId, slot_count: usize) -> Self {
        ChildList {
            parent,
            slots: vec![Vec::new(); slot_count],
        }
    }

    /// スロット `idx` の直後（より後ろの最初の非空スロットの先頭要素）。`None` は末尾。
    fn anchor_after(&self, idx: usize) -> Option<ElId> {
        self.slots[idx + 1..].iter().flatten().copied().next()
    }

    fn set_slot(&mut self, idx: usize, ids: Vec<ElId>) {
        self.slots[idx] = ids;
    }
}

/// instantiate 済みのコンポーネントインスタンス。
///
/// クリック等のインタラクションの入口を持ち、ハンドラ実行 → flush までを駆動する。
pub struct Instance<S: ElementSink> {
    rt: Runtime,
    sink: Rc<RefCell<S>>,
    click_targets: ClickTargets,
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

    /// click ハンドラが登録されている要素の ElId 一覧。App Host への mount 時に、
    /// これらへ listener を登録して `ListenerId → ElId` を組む（src/app_host.rs）。
    pub fn click_target_ids(&self) -> Vec<ElId> {
        self.click_targets.borrow().keys().copied().collect()
    }

    /// 要素のクリックをディスパッチする。ハンドラ本体を batch で走らせ、その中の
    /// signal write を 1 回の flush（fine-grained patch）にまとめて sink へ落とす。
    /// 対象に click ハンドラが無ければ `false`。
    pub fn click(&self, target: ElId) -> bool {
        let handler = match self.click_targets.borrow().get(&target) {
            Some(h) => h.clone(),
            None => return false,
        };
        self.rt.batch(|| {
            (handler.borrow_mut())(Value::Bool(false));
        });
        true
    }
}

/// instantiate 中に各所へ引き回す共有コンテキスト。
struct Ctx<S: ElementSink> {
    rt: Runtime,
    sink: Rc<RefCell<S>>,
    click_targets: ClickTargets,
}

impl<S: ElementSink> Clone for Ctx<S> {
    fn clone(&self) -> Self {
        Ctx {
            rt: self.rt.clone(),
            sink: self.sink.clone(),
            click_targets: self.click_targets.clone(),
        }
    }
}

/// Template IR を instantiate し、binding を配線して `Instance` を返す。
pub fn instantiate<S: ElementSink + 'static>(
    rt: &Runtime,
    template: &TemplateNode,
    scope: &Scope,
    handlers: Vec<Handler>,
    sink: Rc<RefCell<S>>,
) -> Instance<S> {
    let handlers: Handlers = Rc::new(
        handlers
            .into_iter()
            .map(|h| Rc::new(RefCell::new(h)))
            .collect(),
    );
    let ctx = Ctx {
        rt: rt.clone(),
        sink: sink.clone(),
        click_targets: Rc::new(RefCell::new(HashMap::new())),
    };

    // コンポーネントのルート Scope。全 build をこの中で行い、動的ブロックの Scope は
    // この子になる（インスタンス unmount で一括 teardown 可能）。
    let root_scope = rt.create_scope(None);
    let root = rt.run_in_scope(root_scope, || {
        build_element(&ctx, template, scope, &handlers)
    });
    sink.borrow_mut().set_root(root);

    Instance {
        rt: rt.clone(),
        sink,
        click_targets: ctx.click_targets,
        root,
    }
}

/// 1 要素を instantiate する（text 束縛は Effect として配線、子は ChildList で管理）。
fn build_element<S: ElementSink + 'static>(
    ctx: &Ctx<S>,
    node: &TemplateNode,
    scope: &Scope,
    handlers: &Handlers,
) -> ElId {
    let id = ctx.sink.borrow_mut().create_element(node.kind);

    // 静的テキスト（束縛なし）は 1 回だけ書く。
    if let Some(text) = &node.static_text {
        ctx.sink.borrow_mut().set_text(id, text);
    }

    // reactive text 束縛：純粋式を読む Effect を張る（読んだ signal だけに依存）。
    if let Some(expr) = &node.text_binding {
        let expr = expr.clone();
        let scope = scope.clone();
        let sink = ctx.sink.clone();
        ctx.rt.effect(move || {
            let value = expr.eval(&scope);
            sink.borrow_mut().set_text(id, &value.to_display_string());
        });
    }

    // click ハンドラを現在の handler 集合から解決して登録する。
    if let Some(handler_id) = node.on_click {
        ctx.click_targets
            .borrow_mut()
            .insert(id, handlers[handler_id].clone());
    }

    if !node.children.is_empty() {
        build_children(ctx, id, &node.children, scope, handlers);
    }

    id
}

/// 親要素の子（要素・`:if`・`:each`・コンポーネント）を ChildList の各スロットに
/// instantiate する。
fn build_children<S: ElementSink + 'static>(
    ctx: &Ctx<S>,
    parent: ElId,
    children: &[Template],
    scope: &Scope,
    handlers: &Handlers,
) {
    let child_list = Rc::new(RefCell::new(ChildList::new(parent, children.len())));

    for (slot, child) in children.iter().enumerate() {
        match child {
            Template::Element(node) => {
                let child_id = build_element(ctx, node, scope, handlers);
                // 初期 build は順次処理なので末尾 append で正しい順序になる。
                ctx.sink.borrow_mut().append_child(parent, child_id);
                child_list.borrow_mut().set_slot(slot, vec![child_id]);
            }
            Template::If(block) => mount_if(ctx, block, scope, handlers, &child_list, slot),
            Template::Each(block) => mount_each(ctx, block, scope, handlers, &child_list, slot),
            Template::Component(comp) => {
                mount_component(ctx, comp, scope, handlers, &child_list, slot)
            }
        }
    }
}

/// 親 `parent` に、`anchor` の直前（None なら末尾）に `child` を取り付ける。
fn attach<S: ElementSink>(ctx: &Ctx<S>, parent: ElId, child: ElId, anchor: Option<ElId>) {
    let mut sink = ctx.sink.borrow_mut();
    match anchor {
        Some(before) => sink.insert_before(parent, child, before),
        None => sink.append_child(parent, child),
    }
}

/// `:if` を構造 Effect として mount する。条件 signal を追跡し、truthy↔falsy で
/// body の Scope を mount / teardown する。
fn mount_if<S: ElementSink + 'static>(
    ctx: &Ctx<S>,
    block: &IfBlock,
    scope: &Scope,
    handlers: &Handlers,
    child_list: &Rc<RefCell<ChildList>>,
    slot: usize,
) {
    let cond = block.cond.clone();
    let body = Rc::new(block.body.as_ref().clone());
    let scope = scope.clone();
    let handlers = handlers.clone();
    let ctx = ctx.clone();
    let child_list = child_list.clone();
    let owner = current_owner(&ctx.rt);

    // 現在 mount 中の (branch scope, 要素 id)。falsy のときは None。
    let mounted: Rc<RefCell<Option<(ScopeId, ElId)>>> = Rc::new(RefCell::new(None));

    let rt = ctx.rt.clone();
    rt.effect(move || {
        let show = cond.eval(&scope).truthy();
        let is_mounted = mounted.borrow().is_some();
        if show && !is_mounted {
            let branch = ctx.rt.create_scope(Some(owner));
            let child_ctx = ctx.clone();
            let body = body.clone();
            let scope = scope.clone();
            let handlers = handlers.clone();
            let el = ctx.rt.run_in_scope(branch, || {
                build_element(&child_ctx, &body, &scope, &handlers)
            });
            let anchor = child_list.borrow().anchor_after(slot);
            attach(&ctx, child_list.borrow().parent, el, anchor);
            child_list.borrow_mut().set_slot(slot, vec![el]);
            register_remove(&ctx, branch, el);
            *mounted.borrow_mut() = Some((branch, el));
        } else if !show && is_mounted {
            let (branch, _el) = mounted.borrow_mut().take().unwrap();
            child_list.borrow_mut().set_slot(slot, Vec::new());
            // Scope teardown が cleanup（要素除去）と Effect 破棄をまとめて行う。
            ctx.rt.dispose_scope(branch);
        }
    });
}

/// 要素 `el` を teardown 時に除去する cleanup を `scope` に登録する。
fn register_remove<S: ElementSink + 'static>(ctx: &Ctx<S>, scope: ScopeId, el: ElId) {
    let ctx2 = ctx.clone();
    ctx.rt.run_in_scope(scope, || {
        let sink = ctx2.sink.clone();
        ctx2.rt.on_cleanup(move || sink.borrow_mut().remove(el));
    });
}

/// コンポーネントインスタンスを mount する（ADR-0004）。
///
/// 1. インスタンス Scope を作る（owner の子）。
/// 2. prop を Memo（親式の評価）として配線する — 子は Binding として読むだけ。
/// 3. emit を親ハンドラへルーティングする Emit を作る。
/// 4. setup（script 代役）をインスタンス Scope で走らせ、ローカル状態・lifecycle・
///    ハンドラ・テンプレートを得る。
/// 5. テンプレートをインスタンス Scope ＋ インスタンス handler 集合で build する。
/// 6. on_mount を実行する。
fn mount_component<S: ElementSink + 'static>(
    ctx: &Ctx<S>,
    slot_decl: &ComponentSlot,
    parent_scope: &Scope,
    parent_handlers: &Handlers,
    child_list: &Rc<RefCell<ChildList>>,
    slot: usize,
) {
    let rt = ctx.rt.clone();
    let owner = current_owner(&rt);
    let instance = rt.create_scope(Some(owner));

    // (2) prop 配線：親式を評価する Memo をインスタンス Scope に作る。親 signal が
    // 変われば Memo が再計算され、子の binding（Memo を読む Effect）が fine-grained に
    // patch される。
    let mut props = HashMap::new();
    for (name, expr) in &slot_decl.props {
        let expr = expr.clone();
        let ps = parent_scope.clone();
        let memo = rt.run_in_scope(instance, || rt.memo(move || expr.eval(&ps)));
        props.insert(name.clone(), memo);
    }

    // (3) emit 配線：子イベント名 → 親ハンドラ（親の handler 集合の添字）。
    let mut emit_map = HashMap::new();
    for (event, handler_id) in &slot_decl.events {
        emit_map.insert(event.clone(), parent_handlers[*handler_id].clone());
    }
    let emit = Emit::new(emit_map);

    // (4) setup をインスタンス Scope で実行する（ローカル signal・on_destroy が
    // インスタンス所有になる）。
    let on_mount = Rc::new(RefCell::new(Vec::<Box<dyn FnOnce()>>::new()));
    let cx = SetupCx::new(rt.clone(), props, emit, on_mount.clone());
    let view = rt.run_in_scope(instance, || run_setup(&slot_decl.component, &cx));

    let inst_handlers: Handlers = Rc::new(
        view.handlers
            .into_iter()
            .map(|h| Rc::new(RefCell::new(h)))
            .collect(),
    );

    // (5) テンプレートを build する。
    let el = rt.run_in_scope(instance, || {
        build_element(ctx, &view.template, &view.scope, &inst_handlers)
    });
    let anchor = child_list.borrow().anchor_after(slot);
    attach(ctx, child_list.borrow().parent, el, anchor);
    child_list.borrow_mut().set_slot(slot, vec![el]);
    register_remove(ctx, instance, el);

    // (6) on_mount を実行する。
    for f in on_mount.borrow_mut().drain(..) {
        f();
    }
}

fn run_setup(component: &Rc<Component>, cx: &SetupCx) -> crate::component::ComponentView {
    component.run_setup(cx)
}

/// `:each`（keyed-only）を構造 Effect として mount する。
fn mount_each<S: ElementSink + 'static>(
    ctx: &Ctx<S>,
    block: &EachBlock,
    scope: &Scope,
    handlers: &Handlers,
    child_list: &Rc<RefCell<ChildList>>,
    slot: usize,
) {
    let items_expr = block.items.clone();
    let key_expr = block.key.clone();
    let item_var = block.item_var.clone();
    let body = Rc::new(block.body.as_ref().clone());
    let scope = scope.clone();
    let handlers = handlers.clone();
    let ctx = ctx.clone();
    let child_list = child_list.clone();
    let owner = current_owner(&ctx.rt);

    // キー順の行状態。
    let rows: Rc<RefCell<Vec<Row>>> = Rc::new(RefCell::new(Vec::new()));

    let rt = ctx.rt.clone();
    rt.effect(move || {
        // items 式を評価（このリスト signal を追跡）。
        let items = match items_expr.eval(&scope) {
            Value::List(list) => (*list).clone(),
            _ => Vec::new(),
        };

        // 各 item のキーを計算（item_var を item 値に束縛して key 式を評価）。
        let new_keys: Vec<String> = items
            .iter()
            .map(|item| {
                let key_scope = scope
                    .clone()
                    .with(item_var.as_str(), Binding::Const(item.clone()));
                key_expr.eval(&key_scope).to_display_string()
            })
            .collect();

        reconcile_each(
            &ctx,
            &rows,
            &child_list,
            slot,
            owner,
            &item_var,
            &scope,
            &handlers,
            &body,
            &items,
            &new_keys,
        );
    });
}

/// 1 行の保持状態（ADR-0004：行は keyed。値更新は signal 経由で in-place patch）。
struct Row {
    key: String,
    branch: ScopeId,
    /// 行の item 値 signal。同一キーの値更新はこの signal を set して in-place patch。
    item: crate::reactive::Signal,
    el: ElId,
}

/// keyed reconcile：新しいキー列に合わせて行を再利用・生成・削除し、順序が変わった
/// ときだけ move する。
#[allow(clippy::too_many_arguments)]
fn reconcile_each<S: ElementSink + 'static>(
    ctx: &Ctx<S>,
    rows: &Rc<RefCell<Vec<Row>>>,
    child_list: &Rc<RefCell<ChildList>>,
    slot: usize,
    owner: ScopeId,
    item_var: &str,
    scope: &Scope,
    handlers: &Handlers,
    body: &Rc<TemplateNode>,
    items: &[Value],
    new_keys: &[String],
) {
    // 旧行を key→index で引けるようにする。
    let old_index: HashMap<String, usize> = rows
        .borrow()
        .iter()
        .enumerate()
        .map(|(i, r)| (r.key.clone(), i))
        .collect();
    let old_order: Vec<String> = rows.borrow().iter().map(|r| r.key.clone()).collect();

    // 新しい行ベクタを構築する（再利用 or 生成）。
    let mut next_rows: Vec<Row> = Vec::with_capacity(new_keys.len());
    let mut taken = vec![false; rows.borrow().len()];

    for (item, key) in items.iter().zip(new_keys.iter()) {
        if let Some(&i) = old_index.get(key) {
            // 再利用：同一キーの値更新は item signal を set して in-place patch する。
            taken[i] = true;
            let row = &rows.borrow()[i];
            let (branch, el, item_sig) = (row.branch, row.el, row.item.clone());
            item_sig.set(item.clone());
            next_rows.push(Row {
                key: key.clone(),
                branch,
                item: item_sig,
                el,
            });
        } else {
            // 新規行：行 Scope を作り、item signal を束ねた body を build する。
            let branch = ctx.rt.create_scope(Some(owner));
            let item_sig = ctx.rt.run_in_scope(branch, || ctx.rt.signal(item.clone()));
            let row_scope = scope
                .clone()
                .with(item_var, Binding::Signal(item_sig.clone()));
            let child_ctx = ctx.clone();
            let body = body.clone();
            let handlers = handlers.clone();
            let el = ctx.rt.run_in_scope(branch, || {
                build_element(&child_ctx, &body, &row_scope, &handlers)
            });
            register_remove(ctx, branch, el);
            next_rows.push(Row {
                key: key.clone(),
                branch,
                item: item_sig,
                el,
            });
        }
    }

    // 取り残された旧行（新キーに無い）を teardown する。
    let removed: Vec<ScopeId> = rows
        .borrow()
        .iter()
        .enumerate()
        .filter(|(i, _)| !taken[*i])
        .map(|(_, r)| r.branch)
        .collect();
    for branch in removed {
        ctx.rt.dispose_scope(branch);
    }

    // 並べ替え：キー順が変わったときだけ move する（値だけの更新では動かさない）。
    let order_changed = old_order != *new_keys;
    let new_ids: Vec<ElId> = next_rows.iter().map(|r| r.el).collect();

    *rows.borrow_mut() = next_rows;
    child_list.borrow_mut().set_slot(slot, new_ids.clone());

    if order_changed {
        // 後ろから anchor を更新しつつ insert_before で目標順に並べる。
        let parent = child_list.borrow().parent;
        let mut anchor = child_list.borrow().anchor_after(slot);
        for &el in new_ids.iter().rev() {
            attach(ctx, parent, el, anchor);
            anchor = Some(el);
        }
    }
}

/// build 中の現在の所有スコープ。build は必ずルート Scope の中で走るので存在する。
fn current_owner(rt: &Runtime) -> ScopeId {
    rt.current_scope()
        .expect("build must run inside an owner scope")
}
