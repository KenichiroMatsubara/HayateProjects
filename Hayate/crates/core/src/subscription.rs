//! wave-2 ストリーム capability の共有契約土台: Core 所有の RAII 購読ハンドル（ADR-0120）。
//!
//! `subscribe()` が返す [`Subscription`] は **pollable**: [`Subscription::poll_changes`] が蓄積された
//! 変化を**順序保持で `Vec` drain** する（`poll_deliveries` と同型）。Core 契約に値コールバック
//! （`FnMut(T)`）は置かない — threading marshaling とバッファリングは leaf（Platform Adapter）に
//! 隠れる前提で、consumer は自スレッドの flush 点で `Vec<T>` を引くだけ。
//!
//! [`Subscription`] は **RAII ハンドル**で、購読の生存そのもの。所有者は consumer（アプリ／
//! Hayabusa ランタイム側）で、`Drop` が leaf へ native 登録の解除を伝える（**契約と Drop 意味論は
//! Core、解除の native 手続きは leaf**）。`Drop` は値を返せないため**解除失敗は best-effort で握り
//! 潰す**（解除に `Result` を取らない）。明示 `unsubscribe(id)` は設けない — 手動ペアの呼び忘れに
//! よる native listener／sensor のリーク（電池消費直結）を型で防ぐ。
//!
//! 多重購読は「ハンドル 1 つ = 購読 1 つ」だけを契約する。native 登録の集約／参照カウントは leaf
//! 裁量。Core は単一スレッド（ADR-0003）なので共有バッファは `Rc<RefCell<…>>`。

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

/// 変化ストリームの consumer 側ハンドル。`subscribe()` が返す。値は [`poll_changes`] で
/// フレームの flush 点に drain し、`Drop` で leaf の native 登録を解除する（best-effort）。
///
/// [`poll_changes`]: Subscription::poll_changes
pub struct Subscription<T> {
    changes: Rc<RefCell<VecDeque<T>>>,
    on_unsubscribe: Option<Box<dyn FnOnce()>>,
}

/// 変化ストリームの producer 側ハンドル。leaf（または host fake provider）が保持し、native
/// callback を marshaling した変化を [`push`] で流し込む。consumer の [`Subscription`] と同一
/// バッファを共有する（`subscribe()` 内で対にして生成）。
///
/// [`push`]: SubscriptionSource::push
pub struct SubscriptionSource<T> {
    changes: Rc<RefCell<VecDeque<T>>>,
}

impl<T> Subscription<T> {
    /// 購読ハンドルと producer 側 [`SubscriptionSource`] を対にして生成する。`on_unsubscribe` は
    /// ハンドルの `Drop` 時に一度だけ走る native 解除フック（leaf が native 登録解除を仕込む）。
    pub fn new(on_unsubscribe: impl FnOnce() + 'static) -> (Self, SubscriptionSource<T>) {
        let changes: Rc<RefCell<VecDeque<T>>> = Rc::new(RefCell::new(VecDeque::new()));
        let subscription = Self {
            changes: Rc::clone(&changes),
            on_unsubscribe: Some(Box::new(on_unsubscribe)),
        };
        let source = SubscriptionSource { changes };
        (subscription, source)
    }

    /// 蓄積された変化を**順序保持で全件 drain** して返す。drain 後のバッファは空になるので、
    /// 続けて呼ぶと（新たな push が無ければ）空 `Vec` を返す。
    pub fn poll_changes(&mut self) -> Vec<T> {
        self.changes.borrow_mut().drain(..).collect()
    }
}

impl<T> SubscriptionSource<T> {
    /// native 由来の変化を 1 件、購読バッファ末尾へ積む（順序保持）。
    pub fn push(&self, change: T) {
        self.changes.borrow_mut().push_back(change);
    }
}

impl<T> Drop for Subscription<T> {
    fn drop(&mut self) {
        // best-effort: 解除フックは一度だけ走らせ、失敗は握り潰す（`Result` を取らない）。
        if let Some(on_unsubscribe) = self.on_unsubscribe.take() {
            on_unsubscribe();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn poll_changes_drains_accumulated_changes_in_order() {
        let (mut sub, source) = Subscription::new(|| {});
        source.push(1);
        source.push(2);
        source.push(3);
        assert_eq!(sub.poll_changes(), vec![1, 2, 3]);
    }

    #[test]
    fn second_poll_after_a_drain_is_empty() {
        let (mut sub, source) = Subscription::<i32>::new(|| {});
        source.push(10);
        let _ = sub.poll_changes();
        assert!(sub.poll_changes().is_empty());
    }

    #[test]
    fn drop_runs_the_unsubscribe_hook_exactly_once() {
        let calls = Rc::new(Cell::new(0u32));
        let counter = Rc::clone(&calls);
        let (sub, _source) = Subscription::<i32>::new(move || counter.set(counter.get() + 1));
        assert_eq!(calls.get(), 0, "解除は drop まで走らない");
        drop(sub);
        assert_eq!(calls.get(), 1, "drop で解除フックがちょうど一度走る");
    }
}
