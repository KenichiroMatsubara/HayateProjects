//! App Host 配線（ADR-0117）：Hayabusa を共有 App Host へ `DeliverySink` として mount する。
//!
//! ADR-0117 では **App Host が `ElementTree` 実体を所有**し、フレームごとに
//! `DeliverySink::handle(deliveries, &mut ElementTree)` で借用を渡す。一方 Hayabusa の
//! reactive effect は sink を `Rc<RefCell<S>>` で `'static` に握り、flush 点で mutation を
//! 書く。両者の生存期間の食い違い（借用ツリーは handle 中だけ）を **buffering（脱同期）**で
//! 解く：
//!
//! 1. Hayabusa は [`RecordingSink`] を buffering sink として使い、effect は `Mutation` を
//!    ログへ積むだけにする（初期 instantiate も含め全 mutation がここに溜まる）。
//! 2. `handle` が delivery を handler へルーティングして flush を回し（ログが伸びる）、
//!    フレーム末に溜まった `Mutation` 列を [`apply_mutation`] で借用ツリーへ出し切る。
//!
//! これにより unsafe な生ポインタを使わず、handler 由来・初期構築の双方を同じ drain 経路に
//! 乗せられる。ADR-0117 が言う「return 前にそのフレーム分を出し切る」を満たす。
//!
//! ルーティング：mount 時に click ターゲット ElId へ [`register_listener`] し
//! `ListenerId → ElId` を作る。delivery の `listener_id` を ElId に引き直して
//! [`Instance::click`] を呼ぶ（handler 実行 → batch flush）。App Host は `ListenerId` の
//! 意味も handler の存在も知らない（consumer 非依存）。
//!
//! `feature = "app-host"` 専用。
//!
//! [`register_listener`]: hayate_core::ElementTree::register_listener

use crate::hayate_sink::apply_mutation;
use crate::instantiate::Instance;
use crate::sink::{ElId, RecordingSink};
use hayate_app_host::DeliverySink;
use hayate_core::{DocumentEventKind, ElementId, ElementTree, Event, EventDelivery, ListenerId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// `RecordingSink` を buffering sink に使う Hayabusa インスタンスを、App Host の
/// `DeliverySink` として mount するアダプタ。
///
/// 構築時点では App Host の `ElementTree` をまだ借りられないため、初期構築 mutation の
/// 適用と listener 登録は最初の [`handle`](DeliverySink::handle) で遅延実行する。
pub struct HayabusaApp {
    instance: Instance<RecordingSink>,
    sink: Rc<RefCell<RecordingSink>>,
    listener_to_target: HashMap<ListenerId, ElId>,
    mounted: bool,
}

impl HayabusaApp {
    /// buffering `RecordingSink` 上で instantiate 済みの `Instance` から App Host アダプタを作る。
    pub fn new(instance: Instance<RecordingSink>) -> Self {
        let sink = instance.sink();
        HayabusaApp {
            instance,
            sink,
            listener_to_target: HashMap::new(),
            mounted: false,
        }
    }

    /// 初回 handle 時の遅延 mount：初期構築 mutation を借用ツリーへ適用し、click ターゲットへ
    /// listener を登録して `ListenerId → ElId` を作る。
    fn ensure_mounted(&mut self, tree: &mut ElementTree) {
        if self.mounted {
            return;
        }
        // 1. instantiate が積んだ初期構築 mutation をツリーへ出し切る。
        self.drain_into(tree);
        // 2. click ターゲットへ listener を登録し、delivery を引き戻すための逆引きを作る。
        for elid in self.instance.click_target_ids() {
            let lid = tree.register_listener(ElementId::from_u64(elid.0), DocumentEventKind::Click);
            self.listener_to_target.insert(lid, elid);
        }
        self.mounted = true;
    }

    /// buffering sink に溜まった mutation を借用ツリーへ適用し、ログを空にする。
    fn drain_into(&mut self, tree: &mut ElementTree) {
        for m in self.sink.borrow_mut().take_log() {
            apply_mutation(tree, &m);
        }
    }
}

impl DeliverySink for HayabusaApp {
    fn handle(&mut self, deliveries: &[EventDelivery], tree: &mut ElementTree) {
        self.ensure_mounted(tree);

        for d in deliveries {
            // 現状ルーティングするのは Click のみ（Instance は click ハンドラを持つ）。
            // 他イベント種は後続（on:input 等は P4・ADR-0007 の経路で足す）。
            if matches!(d.event, Event::Click { .. }) {
                if let Some(&elid) = self.listener_to_target.get(&d.listener_id) {
                    // handler 実行 → batch flush。effect は buffering sink へ mutation を積む。
                    self.instance.click(elid);
                }
            }
        }

        // フレーム分（handler 由来）の mutation を借用ツリーへ出し切ってから return する。
        self.drain_into(tree);
    }
}
