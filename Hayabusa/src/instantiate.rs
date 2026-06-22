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
//!   並べ替えは move（再生成しない）、追加/削除は行 Scope の生成/破棄（ADR-0004）。
//!
//! 兄弟の静的要素・`:if`・`:each` が同じ親に混在しても正しい位置に挿入できるよう、
//! 親要素ごとに [`ChildList`]（スロット順 ＋ 各スロットの現在 ElId）を持ち、制御フローは
//! 自分のスロットの「次の兄弟」を anchor として `insert_before` する（marker ノード不要）。

use crate::expr::{Binding, Scope};
use crate::reactive::{Runtime, ScopeId};
use crate::sink::{ElId, ElementSink};
use crate::template::{EachBlock, HandlerId, IfBlock, Template, TemplateNode};
use crate::value::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// 副作用ハンドラの本体（script の代役。tracer bullet では Rust クロージャ）。
pub type Handler = Box<dyn FnMut()>;

/// click ハンドラの登録簿（要素 → ハンドラ）。動的に mount される行/ブランチの
/// 要素も登録できるよう共有する。
type ClickTargets = Rc<RefCell<HashMap<ElId, HandlerId>>>;

/// 親要素の子スロット管理。各スロットは 1 つの子位置（静的要素・`:if`・`:each`）が
/// 現在寄与している top-level ElId 列を順に保持する。制御フローはここから anchor を
/// 引いて `insert_before` する。
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
    handlers: Vec<Rc<RefCell<Handler>>>,
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

    /// 要素のクリックをディスパッチする。ハンドラ本体を batch で走らせ、その中の
    /// signal write を 1 回の flush（fine-grained patch）にまとめて sink へ落とす。
    /// 対象に click ハンドラが無ければ `false`。
    pub fn click(&self, target: ElId) -> bool {
        let handler_id = match self.click_targets.borrow().get(&target) {
            Some(&id) => id,
            None => return false,
        };
        let handler = self.handlers[handler_id].clone();
        self.rt.batch(|| {
            (handler.borrow_mut())();
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
    let handlers: Vec<Rc<RefCell<Handler>>> = handlers
        .into_iter()
        .map(|h| Rc::new(RefCell::new(h)))
        .collect();
    let ctx = Ctx {
        rt: rt.clone(),
        sink: sink.clone(),
        click_targets: Rc::new(RefCell::new(HashMap::new())),
    };

    // コンポーネントのルート Scope。全 build をこの中で行い、`:if` / `:each` の
    // ブランチ Scope はこの子になる（インスタンス unmount で一括 teardown 可能）。
    let root_scope = rt.create_scope(None);
    let root = rt.run_in_scope(root_scope, || build_element(&ctx, template, scope));
    sink.borrow_mut().set_root(root);

    Instance {
        rt: rt.clone(),
        sink,
        handlers,
        click_targets: ctx.click_targets,
        root,
    }
}

/// 1 要素を instantiate する（text 束縛は Effect として配線、子は ChildList で管理）。
fn build_element<S: ElementSink + 'static>(
    ctx: &Ctx<S>,
    node: &TemplateNode,
    scope: &Scope,
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

    if let Some(handler_id) = node.on_click {
        ctx.click_targets.borrow_mut().insert(id, handler_id);
    }

    if !node.children.is_empty() {
        build_children(ctx, id, &node.children, scope);
    }

    id
}

/// 親要素の子（要素・`:if`・`:each`）を ChildList の各スロットに instantiate する。
fn build_children<S: ElementSink + 'static>(
    ctx: &Ctx<S>,
    parent: ElId,
    children: &[Template],
    scope: &Scope,
) {
    let child_list = Rc::new(RefCell::new(ChildList::new(parent, children.len())));

    for (slot, child) in children.iter().enumerate() {
        match child {
            Template::Element(node) => {
                let child_id = build_element(ctx, node, scope);
                // 初期 build は順次処理なので末尾 append で正しい順序になる。
                ctx.sink.borrow_mut().append_child(parent, child_id);
                child_list.borrow_mut().set_slot(slot, vec![child_id]);
            }
            Template::If(block) => {
                mount_if(ctx, block, scope, &child_list, slot);
            }
            Template::Each(block) => {
                mount_each(ctx, block, scope, &child_list, slot);
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
    child_list: &Rc<RefCell<ChildList>>,
    slot: usize,
) {
    let cond = block.cond.clone();
    let body = Rc::new(block.body.as_ref().clone());
    let scope = scope.clone();
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
            let el = ctx
                .rt
                .run_in_scope(branch, || build_element(&child_ctx, &body, &scope));
            let anchor = child_list.borrow().anchor_after(slot);
            attach(&ctx, child_list.borrow().parent, el, anchor);
            child_list.borrow_mut().set_slot(slot, vec![el]);
            // teardown 時に要素を除去する cleanup を branch scope に登録する。
            ctx.rt.run_in_scope(branch, || {
                let sink = ctx.sink.clone();
                ctx.rt.on_cleanup(move || sink.borrow_mut().remove(el));
            });
            *mounted.borrow_mut() = Some((branch, el));
        } else if !show && is_mounted {
            let (branch, _el) = mounted.borrow_mut().take().unwrap();
            child_list.borrow_mut().set_slot(slot, Vec::new());
            // Scope teardown が cleanup（要素除去）と Effect 破棄をまとめて行う。
            ctx.rt.dispose_scope(branch);
        }
    });
}

/// `:each`（keyed-only）を構造 Effect として mount する。
fn mount_each<S: ElementSink + 'static>(
    ctx: &Ctx<S>,
    block: &EachBlock,
    scope: &Scope,
    child_list: &Rc<RefCell<ChildList>>,
    slot: usize,
) {
    let items_expr = block.items.clone();
    let key_expr = block.key.clone();
    let item_var = block.item_var.clone();
    let body = Rc::new(block.body.as_ref().clone());
    let scope = scope.clone();
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
            let el = ctx
                .rt
                .run_in_scope(branch, || build_element(&child_ctx, &body, &row_scope));
            // teardown 時に要素を除去する cleanup を行 Scope に登録する。
            ctx.rt.run_in_scope(branch, || {
                let sink = ctx.sink.clone();
                ctx.rt.on_cleanup(move || sink.borrow_mut().remove(el));
            });
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
