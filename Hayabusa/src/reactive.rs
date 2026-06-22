//! 自作 fine-grained リアクティブコア（ADR-0003）。
//!
//! Signal / Memo（Computed）/ Effect を、依存の自動追跡を持つ fine-grained な
//! リアクティブグラフとして実装する。外部リアクティブライブラリには委ねず、
//! ランタイムが所有する（ADR-0001 の責務線：reactive 機構は再利用可能で
//! ランタイム所有）。
//!
//! ## 実行セマンティクス
//!
//! - **glitch-free**：依存変化は push でマーク（直接の依存先を `Dirty`、その先を
//!   `Check` に伝播）し、読み取り時に pull で評価する（`update_if_necessary` が
//!   source 鎖をたどって必要時だけ再計算）。memo は lazy（read 時に評価）。
//!   菱形依存でも中間の不整合を観測者へ出さない。
//! - **flush 合体**：[`Runtime::batch`] の中の複数 write は 1 回だけ flush に
//!   まとめられ、Effect は 1 イベントにつき 1 回だけ走る。Hayate の apply_mutations
//!   「1 バッチ／frame」哲学に整合する。

use crate::value::Value;
use std::cell::RefCell;
use std::mem;
use std::rc::Rc;

type NodeId = usize;

/// グラフノードの鮮度。`Clean < Check < Dirty` の半順序で伝播する。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum NodeState {
    /// 値は最新。
    Clean,
    /// source の *いずれか* が変わったかもしれない（pull 時に確認が必要）。
    Check,
    /// source が確実に変わった（pull 時に再計算が必要）。
    Dirty,
}

/// ノードが持つ計算。Signal は計算を持たない（`None`）。
enum Computation {
    /// Signal、または計算実行中の一時的な抜き殻。
    None,
    /// Computed：依存を読みながら値を返す純粋計算。
    Memo(Box<dyn FnMut() -> Value>),
    /// Effect：副作用（binding の prop 書き込み等）。
    Effect(Box<dyn FnMut()>),
}

struct ReNode {
    state: NodeState,
    /// Signal / Memo のキャッシュ値。Effect は `None`。
    value: Option<Value>,
    /// このノードが今回の実行で読んだノード（依存）。
    sources: Vec<NodeId>,
    /// このノードを読んでいるノード（依存先）。
    observers: Vec<NodeId>,
    computation: Computation,
    is_effect: bool,
}

struct Inner {
    nodes: Vec<ReNode>,
    /// 現在実行中の計算ノード。read はこのノードに依存を記録する。
    observer: Option<NodeId>,
    /// flush 待ちの Effect。
    pending: Vec<NodeId>,
    batch_depth: u32,
}

/// fine-grained リアクティブグラフを所有するランタイム。
///
/// `Rc<RefCell<_>>` で内部可変性を持ち、`Signal` / `Memo` / Effect クロージャは
/// このハンドルを clone して捕捉する。借用は常に短命に保ち、ユーザクロージャの
/// 実行中はグラフの借用を一切持たない（再入する `get` / `set` が借用衝突を
/// 起こさないため）。
#[derive(Clone)]
pub struct Runtime {
    inner: Rc<RefCell<Inner>>,
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

impl Runtime {
    pub fn new() -> Self {
        Runtime {
            inner: Rc::new(RefCell::new(Inner {
                nodes: Vec::new(),
                observer: None,
                pending: Vec::new(),
                batch_depth: 0,
            })),
        }
    }

    /// 書き込み可能な Signal を作る。
    pub fn signal(&self, initial: Value) -> Signal {
        let id = self.push_node(ReNode {
            state: NodeState::Clean,
            value: Some(initial),
            sources: Vec::new(),
            observers: Vec::new(),
            computation: Computation::None,
            is_effect: false,
        });
        Signal {
            id,
            rt: self.clone(),
        }
    }

    /// 依存から派生する lazy な Computed を作る。最初の read まで評価しない。
    pub fn memo(&self, f: impl FnMut() -> Value + 'static) -> Memo {
        let id = self.push_node(ReNode {
            state: NodeState::Dirty,
            value: None,
            sources: Vec::new(),
            observers: Vec::new(),
            computation: Computation::Memo(Box::new(f)),
            is_effect: false,
        });
        Memo {
            id,
            rt: self.clone(),
        }
    }

    /// Effect を作り、依存を追跡しながら即座に 1 回実行する。以降は依存が変わる
    /// たびに flush 上で再実行される。binding（signal → element-prop）はこの
    /// Effect として表現される（ADR-0004）。
    pub fn effect(&self, f: impl FnMut() + 'static) {
        let id = self.push_node(ReNode {
            state: NodeState::Dirty,
            value: None,
            sources: Vec::new(),
            observers: Vec::new(),
            computation: Computation::Effect(Box::new(f)),
            is_effect: true,
        });
        // 初回実行（依存追跡と初期適用）。
        self.update_if_necessary(id);
    }

    /// クロージャ内の複数 write を 1 回の flush に合体する（ADR-0003 flush 合体）。
    /// ネスト可能で、最外の batch を抜けたときだけ flush する。
    pub fn batch<R>(&self, f: impl FnOnce() -> R) -> R {
        self.inner.borrow_mut().batch_depth += 1;
        let result = f();
        let depth = {
            let mut inner = self.inner.borrow_mut();
            inner.batch_depth -= 1;
            inner.batch_depth
        };
        if depth == 0 {
            self.flush();
        }
        result
    }

    // --- internals ---

    fn push_node(&self, node: ReNode) -> NodeId {
        let mut inner = self.inner.borrow_mut();
        inner.nodes.push(node);
        inner.nodes.len() - 1
    }

    /// ノードの値を読む。実行中の observer があれば依存を記録し、必要なら
    /// pull で最新化してから返す。
    fn read(&self, id: NodeId) -> Value {
        let observer = self.inner.borrow().observer;
        if let Some(o) = observer {
            self.link(o, id);
        }
        self.update_if_necessary(id);
        self.inner.borrow().nodes[id]
            .value
            .clone()
            .expect("reactive node has no value")
    }

    /// 追跡せずに値を読む（ハンドラ内の `signal.update` 等で自己依存を避ける）。
    fn read_untracked(&self, id: NodeId) -> Value {
        self.inner.borrow().nodes[id]
            .value
            .clone()
            .expect("reactive node has no value")
    }

    /// Signal の値を書く。変化があれば観測者を Dirty に伝播し、batch 外なら flush。
    fn write(&self, id: NodeId, value: Value) {
        let observers = {
            let mut inner = self.inner.borrow_mut();
            if inner.nodes[id].value.as_ref() == Some(&value) {
                return; // 変化なし：伝播しない。
            }
            inner.nodes[id].value = Some(value);
            inner.nodes[id].observers.clone()
        };
        for o in observers {
            self.notify(o, NodeState::Dirty);
        }
        if self.inner.borrow().batch_depth == 0 {
            self.flush();
        }
    }

    /// observer → source の依存リンクを張る（重複は張らない）。
    fn link(&self, observer: NodeId, source: NodeId) {
        let mut inner = self.inner.borrow_mut();
        if !inner.nodes[observer].sources.contains(&source) {
            inner.nodes[observer].sources.push(source);
            inner.nodes[source].observers.push(observer);
        }
    }

    /// 鮮度マークを push 伝播する。直接の依存先には `Dirty`、その先には `Check`。
    /// Clean → 非 Clean に落ちた Effect だけを pending に積む。
    fn notify(&self, id: NodeId, state: NodeState) {
        let observers = {
            let mut inner = self.inner.borrow_mut();
            let node = &mut inner.nodes[id];
            if node.state >= state {
                return; // 既に同等以上にマーク済み。
            }
            let was_clean = node.state == NodeState::Clean;
            node.state = state;
            let is_effect = node.is_effect;
            let observers = node.observers.clone();
            if is_effect && was_clean {
                inner.pending.push(id);
            }
            observers
        };
        for o in observers {
            self.notify(o, NodeState::Check);
        }
    }

    /// pull 評価。`Check` なら source 鎖をたどって Dirty 化を確かめ、Dirty なら
    /// 再計算する。glitch-free の核。
    fn update_if_necessary(&self, id: NodeId) {
        let state = self.inner.borrow().nodes[id].state;
        if state == NodeState::Clean {
            return;
        }
        if state == NodeState::Check {
            let sources = self.inner.borrow().nodes[id].sources.clone();
            for s in sources {
                self.update_if_necessary(s);
                if self.inner.borrow().nodes[id].state == NodeState::Dirty {
                    break;
                }
            }
        }
        if self.inner.borrow().nodes[id].state == NodeState::Dirty {
            self.update(id);
        }
        self.inner.borrow_mut().nodes[id].state = NodeState::Clean;
    }

    /// ノードの計算を再実行する。依存を貼り直し、memo は値の変化を観測者へ伝播する。
    fn update(&self, id: NodeId) {
        // 旧依存リンクを破棄（毎回フレッシュに張り直す）。
        {
            let mut inner = self.inner.borrow_mut();
            let sources = mem::take(&mut inner.nodes[id].sources);
            for s in sources {
                inner.nodes[s].observers.retain(|&o| o != id);
            }
        }

        // observer を退避してこのノードに切り替え、計算を一時的に取り出す
        // （クロージャ実行中はグラフの借用を持たない＝再入安全）。
        let (prev_observer, mut computation) = {
            let mut inner = self.inner.borrow_mut();
            let prev = inner.observer;
            inner.observer = Some(id);
            let comp = mem::replace(&mut inner.nodes[id].computation, Computation::None);
            (prev, comp)
        };

        let mut new_value = None;
        match &mut computation {
            Computation::Memo(f) => new_value = Some(f()),
            Computation::Effect(f) => f(),
            Computation::None => {}
        }

        {
            let mut inner = self.inner.borrow_mut();
            inner.nodes[id].computation = computation;
            inner.observer = prev_observer;
        }

        // memo：値が変わったら観測者を Dirty 化する。
        if let Some(value) = new_value {
            let observers = {
                let mut inner = self.inner.borrow_mut();
                if inner.nodes[id].value.as_ref() == Some(&value) {
                    Vec::new()
                } else {
                    inner.nodes[id].value = Some(value);
                    inner.nodes[id].observers.clone()
                }
            };
            for o in observers {
                self.notify(o, NodeState::Dirty);
            }
        }
    }

    /// pending な Effect を実行しきる。Effect 実行中に新たな write が起きても
    /// 取りこぼさないよう、空になるまで繰り返す。
    fn flush(&self) {
        loop {
            let batch = {
                let mut inner = self.inner.borrow_mut();
                if inner.pending.is_empty() {
                    break;
                }
                mem::take(&mut inner.pending)
            };
            for id in batch {
                if self.inner.borrow().nodes[id].state != NodeState::Clean {
                    self.update_if_necessary(id);
                }
            }
        }
    }
}

/// 書き込み可能なリアクティブ値へのハンドル。
#[derive(Clone)]
pub struct Signal {
    id: NodeId,
    rt: Runtime,
}

impl Signal {
    /// 値を読む（実行中の計算があれば依存として記録される）。
    pub fn get(&self) -> Value {
        self.rt.read(self.id)
    }

    /// 依存を記録せずに値を読む。
    pub fn get_untracked(&self) -> Value {
        self.rt.read_untracked(self.id)
    }

    /// 値を書く。
    pub fn set(&self, value: Value) {
        self.rt.write(self.id, value);
    }

    /// 現在値から次の値を導いて書く（自己依存を避けるため untracked で読む）。
    pub fn update(&self, f: impl FnOnce(Value) -> Value) {
        let next = f(self.get_untracked());
        self.set(next);
    }
}

/// 依存から派生する lazy な Computed へのハンドル。
#[derive(Clone)]
pub struct Memo {
    id: NodeId,
    rt: Runtime,
}

impl Memo {
    /// 値を読む（必要なら遅延評価し、実行中の計算があれば依存として記録される）。
    pub fn get(&self) -> Value {
        self.rt.read(self.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn signal_get_set_roundtrip() {
        let rt = Runtime::new();
        let s = rt.signal(Value::number(1));
        assert_eq!(s.get(), Value::number(1));
        s.set(Value::number(2));
        assert_eq!(s.get(), Value::number(2));
    }

    #[test]
    fn effect_runs_on_creation_then_on_change() {
        let rt = Runtime::new();
        let s = rt.signal(Value::number(0));
        let runs = Rc::new(Cell::new(0));
        let last = Rc::new(RefCell::new(Value::number(-1)));

        let s2 = s.clone();
        let runs2 = runs.clone();
        let last2 = last.clone();
        rt.effect(move || {
            runs2.set(runs2.get() + 1);
            *last2.borrow_mut() = s2.get();
        });

        assert_eq!(runs.get(), 1, "effect runs once on creation");
        assert_eq!(*last.borrow(), Value::number(0));

        s.set(Value::number(5));
        assert_eq!(runs.get(), 2, "effect re-runs when its dependency changes");
        assert_eq!(*last.borrow(), Value::number(5));
    }

    #[test]
    fn effect_does_not_run_when_unrelated_signal_changes() {
        let rt = Runtime::new();
        let tracked = rt.signal(Value::number(0));
        let untracked = rt.signal(Value::number(0));
        let runs = Rc::new(Cell::new(0));

        let t2 = tracked.clone();
        let runs2 = runs.clone();
        rt.effect(move || {
            runs2.set(runs2.get() + 1);
            let _ = t2.get();
        });
        assert_eq!(runs.get(), 1);

        untracked.set(Value::number(99));
        assert_eq!(runs.get(), 1, "effect ignores signals it never reads");

        tracked.set(Value::number(1));
        assert_eq!(runs.get(), 2);
    }

    #[test]
    fn memo_is_lazy_and_memoized() {
        let rt = Runtime::new();
        let s = rt.signal(Value::number(2));
        let computations = Rc::new(Cell::new(0));

        let s2 = s.clone();
        let c2 = computations.clone();
        let doubled = rt.memo(move || {
            c2.set(c2.get() + 1);
            Value::number(s2.get().as_number().unwrap() * 2.0)
        });

        assert_eq!(
            computations.get(),
            0,
            "memo is lazy: not computed until read"
        );
        assert_eq!(doubled.get(), Value::number(4));
        assert_eq!(computations.get(), 1);
        // 再 read は再計算しない（memoize）。
        assert_eq!(doubled.get(), Value::number(4));
        assert_eq!(computations.get(), 1);

        s.set(Value::number(10));
        assert_eq!(doubled.get(), Value::number(20));
        assert_eq!(computations.get(), 2);
    }

    #[test]
    fn diamond_dependency_is_glitch_free() {
        // a → (b, c) → d。a を変えても d は 1 回だけ評価され、中間の不整合を観測しない。
        let rt = Runtime::new();
        let a = rt.signal(Value::number(1));

        let a_b = a.clone();
        let b = rt.memo(move || Value::number(a_b.get().as_number().unwrap() + 1.0));
        let a_c = a.clone();
        let c = rt.memo(move || Value::number(a_c.get().as_number().unwrap() + 10.0));

        let d_runs = Rc::new(Cell::new(0));
        let (b2, c2, runs2) = (b.clone(), c.clone(), d_runs.clone());
        let d = rt.memo(move || {
            runs2.set(runs2.get() + 1);
            Value::number(b2.get().as_number().unwrap() + c2.get().as_number().unwrap())
        });

        assert_eq!(d.get(), Value::number(13)); // (1+1) + (1+10)
        assert_eq!(d_runs.get(), 1);

        a.set(Value::number(2));
        assert_eq!(d.get(), Value::number(15)); // (2+1) + (2+10)
        assert_eq!(d_runs.get(), 2, "d recomputes exactly once, no glitch");
    }

    #[test]
    fn batch_coalesces_writes_into_one_effect_run() {
        let rt = Runtime::new();
        let x = rt.signal(Value::number(0));
        let y = rt.signal(Value::number(0));
        let runs = Rc::new(Cell::new(0));

        let (x2, y2, runs2) = (x.clone(), y.clone(), runs.clone());
        rt.effect(move || {
            runs2.set(runs2.get() + 1);
            let _ = (x2.get(), y2.get());
        });
        assert_eq!(runs.get(), 1);

        rt.batch(|| {
            x.set(Value::number(1));
            y.set(Value::number(1));
        });
        assert_eq!(runs.get(), 2, "two writes in a batch flush once");
    }
}
