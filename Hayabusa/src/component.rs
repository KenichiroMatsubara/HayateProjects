//! コンポーネント合成（ADR-0004）。
//!
//! `.hybs` 1 つ ＝ 1 コンポーネント。コンポーネントの合成は **ランタイム上の
//! コンポーネントインスタンス**として行う。各インスタンスは独自の signal スコープ
//! （Scope・ADR-0003）・prop 入力・emit 出力・lifecycle を持つ。
//!
//! 責務線（ADR-0004 の表）：
//!
//! | 概念 | ランタイム（再利用可能） | script（言語固有） |
//! | ---- | ---- | ---- |
//! | prop | 親スコープの式 → 子の入力 signal への束縛配線 | `prop(...)` の宣言面 |
//! | emit | 子の emit → 親の登録ハンドラへのルーティング | `emit(...)` 呼び出し＋親ハンドラ本体 |
//! | 境界 | インスタンス化・signal スコープ隔離・lifecycle 駆動 | `on_mount` / `on_destroy` 本体 |
//!
//! tracer bullet ではコンパイラが無いので、「script」は Rust クロージャ（[`Component`]
//! の setup）で代役する。prop はランタイムが Memo（親式の評価）として配線し、子は
//! それを Binding として読むだけ — 子は親 signal を直接知らない。

use crate::expr::{Binding, Expr, Scope};
use crate::reactive::{Memo, Runtime};
use crate::template::{HandlerId, TemplateNode};
use crate::value::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// 副作用ハンドラの本体（script の代役）。click は payload を使わず（`Value::Bool(false)`
/// が渡る）、emit は payload を運ぶ。
pub type Handler = Box<dyn FnMut(Value)>;

/// マウント後に 1 度だけ走る lifecycle コールバックの共有キュー。
type MountQueue = Rc<RefCell<Vec<Box<dyn FnOnce()>>>>;

/// 子の emit を親の登録ハンドラへルーティングするハンドル（ランタイム所有）。
#[derive(Clone, Default)]
pub struct Emit {
    handlers: Rc<HashMap<String, Rc<RefCell<Handler>>>>,
}

impl Emit {
    pub(crate) fn new(handlers: HashMap<String, Rc<RefCell<Handler>>>) -> Self {
        Emit {
            handlers: Rc::new(handlers),
        }
    }

    /// イベントを親へ送る。対応する親ハンドラがあれば payload 付きで呼ぶ。
    pub fn emit(&self, event: &str, payload: Value) {
        if let Some(h) = self.handlers.get(event) {
            (h.borrow_mut())(payload);
        }
    }
}

/// setup（script 代役）に渡るコンテキスト。prop accessor・emit・lifecycle 登録・
/// ローカル signal 生成のためのランタイムを提供する。
pub struct SetupCx {
    rt: Runtime,
    props: HashMap<String, Memo>,
    emit: Emit,
    on_mount: MountQueue,
}

impl SetupCx {
    pub(crate) fn new(
        rt: Runtime,
        props: HashMap<String, Memo>,
        emit: Emit,
        on_mount: MountQueue,
    ) -> Self {
        SetupCx {
            rt,
            props,
            emit,
            on_mount,
        }
    }

    /// ローカル signal / memo を作るためのランタイム。setup は instance Scope の中で
    /// 走るので、ここで作った reactive はインスタンス所有になる（unmount で破棄）。
    pub fn rt(&self) -> &Runtime {
        &self.rt
    }

    /// prop accessor。親式を評価する Memo への Binding（子はこれを読むだけ）。
    /// 未宣言の prop は falsy に倒す（防御的）。
    pub fn prop(&self, name: &str) -> Binding {
        match self.props.get(name) {
            Some(m) => Binding::Memo(m.clone()),
            None => Binding::Const(Value::Bool(false)),
        }
    }

    /// イベントを親へ emit する。
    pub fn emit(&self, event: &str, payload: Value) {
        self.emit.emit(event, payload);
    }

    /// emit ハンドル（ハンドラに渡して中から emit したいとき用）。
    pub fn emitter(&self) -> Emit {
        self.emit.clone()
    }

    /// マウント後に 1 度だけ走るコールバックを登録する。
    pub fn on_mount(&self, f: impl FnOnce() + 'static) {
        self.on_mount.borrow_mut().push(Box::new(f));
    }

    /// アンマウント時に走る cleanup を登録する（instance Scope の dispose に乗る）。
    pub fn on_destroy(&self, f: impl FnOnce() + 'static) {
        self.rt.on_cleanup(f);
    }
}

/// setup が返すコンポーネントの実体：ローカル scope（prop＋local の束縛）・このインスタンス
/// のハンドラ・テンプレート。
pub struct ComponentView {
    pub scope: Scope,
    pub handlers: Vec<Handler>,
    pub template: TemplateNode,
}

/// コンポーネント定義。`setup` は各インスタンスで呼ばれ、ローカル状態・lifecycle・
/// ハンドラ・テンプレートを組み立てる（script の代役）。
pub struct Component {
    pub props: Vec<String>,
    setup: Box<dyn Fn(&SetupCx) -> ComponentView>,
}

impl Component {
    pub fn new(
        props: Vec<String>,
        setup: impl Fn(&SetupCx) -> ComponentView + 'static,
    ) -> Rc<Self> {
        Rc::new(Component {
            props,
            setup: Box::new(setup),
        })
    }

    pub(crate) fn run_setup(&self, cx: &SetupCx) -> ComponentView {
        (self.setup)(cx)
    }
}

/// テンプレート中のコンポーネント出現箇所（子コンポーネントの宣言）。
///
/// `props` は親スコープで評価する式、`events` は子イベント名 → 親ハンドラ（親の
/// ハンドラ列の添字）。データのみなので Template は Clone のまま保てる。
#[derive(Clone)]
pub struct ComponentSlot {
    pub component: Rc<Component>,
    pub props: Vec<(String, Expr)>,
    pub events: Vec<(String, HandlerId)>,
}

impl ComponentSlot {
    pub fn new(component: Rc<Component>) -> Self {
        ComponentSlot {
            component,
            props: Vec::new(),
            events: Vec::new(),
        }
    }

    /// prop を束縛する（親スコープで評価される式）。
    pub fn prop(mut self, name: impl Into<String>, expr: Expr) -> Self {
        self.props.push((name.into(), expr));
        self
    }

    /// 子イベントを親ハンドラ（添字）へ配線する。
    pub fn on(mut self, event: impl Into<String>, handler: HandlerId) -> Self {
        self.events.push((event.into(), handler));
        self
    }
}
