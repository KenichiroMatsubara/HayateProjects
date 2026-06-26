//! wave-2 ストリーム capability 契約土台のシーム3 conformance（ADR-0120）。
//!
//! leaf stub は host で `Err(Unimplemented)` しか返せない（native SDK 無し）ので、契約の正しい
//! 振る舞い——「subscribe → 変化 push（複数サンプル含む）→ `poll_changes` が順序保持で全件 drain
//! → 2 回目は空 → drop で native 解除が走る」——は **native 非依存の host fake provider** を
//! 公開契約（`Battery` ＋ `Subscription` ＋ producer 側 `SubscriptionSource`）越しに実装して検証する。
//! 実機 leaf が入っても契約は不変なので、この conformance がストリーム契約の正本テストになる。
//!
//! 型は mobile facade 経由で参照し、facade が Core 契約をそのまま再露出していることも兼ねて固める。

use std::cell::Cell;
use std::rc::Rc;

use hayate_adapter_mobile::{
    Battery, BatteryStatus, CapabilityError, Subscription, SubscriptionSource,
};

/// native に触らない fake battery provider。`subscribe` で producer 側 [`SubscriptionSource`] を
/// 自身に保持し、テストが [`push_change`] で変化を流し込む。drop 時に解除フラグを立てる。
struct FakeBattery {
    source: Option<SubscriptionSource<BatteryStatus>>,
    unsubscribed: Rc<Cell<bool>>,
}

impl FakeBattery {
    fn new(unsubscribed: Rc<Cell<bool>>) -> Self {
        Self {
            source: None,
            unsubscribed,
        }
    }

    /// 購読中のストリームへ変化を 1 件流す（native callback の marshaling 相当）。
    fn push_change(&self, status: BatteryStatus) {
        self.source
            .as_ref()
            .expect("subscribe 後にのみ push される")
            .push(status);
    }
}

impl Battery for FakeBattery {
    fn query(&self) -> Result<BatteryStatus, CapabilityError> {
        Ok(BatteryStatus {
            level: 50,
            charging: false,
        })
    }

    fn subscribe(&mut self) -> Result<Subscription<BatteryStatus>, CapabilityError> {
        let unsubscribed = Rc::clone(&self.unsubscribed);
        let (subscription, source) = Subscription::new(move || unsubscribed.set(true));
        self.source = Some(source);
        Ok(subscription)
    }
}

#[test]
fn subscribe_drains_pushed_changes_in_order_then_empties_then_unsubscribes_on_drop() {
    let unsubscribed = Rc::new(Cell::new(false));
    let mut battery = FakeBattery::new(Rc::clone(&unsubscribed));

    let mut subscription = battery.subscribe().expect("fake provider は subscribe を満たす");

    // 複数サンプルを push（高頻度ストリームの「落とさず全部渡す」を含意）。
    let samples = [
        BatteryStatus { level: 80, charging: true },
        BatteryStatus { level: 79, charging: false },
        BatteryStatus { level: 78, charging: false },
    ];
    for status in samples {
        battery.push_change(status);
    }

    // 順序保持で全件 drain。
    assert_eq!(
        subscription.poll_changes(),
        samples.to_vec(),
        "poll_changes は蓄積された変化を順序保持で全件 drain する"
    );

    // 2 回目は空（drain 済み・新規 push 無し）。
    assert!(
        subscription.poll_changes().is_empty(),
        "drain 後・新規 push 無しなら 2 回目の poll は空"
    );

    // drop までは native 解除は走らない。
    assert!(!unsubscribed.get(), "ハンドル生存中は解除されない");

    // RAII: ハンドル drop で leaf の native 解除フックが走る（best-effort）。
    drop(subscription);
    assert!(
        unsubscribed.get(),
        "Subscription の drop が native 登録解除を走らせる（RAII）"
    );
}

#[test]
fn changes_pushed_after_a_drain_surface_on_the_next_poll() {
    let unsubscribed = Rc::new(Cell::new(false));
    let mut battery = FakeBattery::new(unsubscribed);
    let mut subscription = battery.subscribe().expect("subscribe");

    battery.push_change(BatteryStatus { level: 42, charging: false });
    let _ = subscription.poll_changes();

    // drain 後に来た変化は次の poll で順序保持のまま現れる（バッファは drain で空くだけ）。
    battery.push_change(BatteryStatus { level: 41, charging: false });
    battery.push_change(BatteryStatus { level: 40, charging: false });
    assert_eq!(
        subscription.poll_changes(),
        vec![
            BatteryStatus { level: 41, charging: false },
            BatteryStatus { level: 40, charging: false },
        ],
    );
}
