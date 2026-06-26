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
    Battery, BatteryStatus, CapabilityError, SensorKind, SensorSample, Sensors, Subscription,
    SubscriptionSource,
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

    let mut subscription = battery
        .subscribe()
        .expect("fake provider は subscribe を満たす");

    // 複数サンプルを push（高頻度ストリームの「落とさず全部渡す」を含意）。
    let samples = [
        BatteryStatus {
            level: 80,
            charging: true,
        },
        BatteryStatus {
            level: 79,
            charging: false,
        },
        BatteryStatus {
            level: 78,
            charging: false,
        },
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

    battery.push_change(BatteryStatus {
        level: 42,
        charging: false,
    });
    let _ = subscription.poll_changes();

    // drain 後に来た変化は次の poll で順序保持のまま現れる（バッファは drain で空くだけ）。
    battery.push_change(BatteryStatus {
        level: 41,
        charging: false,
    });
    battery.push_change(BatteryStatus {
        level: 40,
        charging: false,
    });
    assert_eq!(
        subscription.poll_changes(),
        vec![
            BatteryStatus {
                level: 41,
                charging: false
            },
            BatteryStatus {
                level: 40,
                charging: false
            },
        ],
    );
}

/// native に触らない fake sensors provider。battery と同じ wave-2 契約土台（`Subscription` ＋
/// `SubscriptionSource`）を、**単一 trait ＋ `SensorKind` 引数**という一段違う形で満たす。
/// `subscribe(kind)` で購読中の kind を記録し、テストが [`emit`] で高頻度サンプルを流し込む。
struct FakeSensors {
    kind: Option<SensorKind>,
    source: Option<SubscriptionSource<SensorSample>>,
}

impl FakeSensors {
    fn new() -> Self {
        Self {
            kind: None,
            source: None,
        }
    }

    /// 購読中のストリームへサンプルを 1 件流す（native sensor callback の marshaling 相当）。
    fn emit(&self, sample: SensorSample) {
        self.source
            .as_ref()
            .expect("subscribe 後にのみ emit される")
            .push(sample);
    }
}

impl Sensors for FakeSensors {
    fn query(&self, kind: SensorKind) -> Result<SensorSample, CapabilityError> {
        // kind で出し分け（sensor ごとに trait を増やさない）。3軸 + timestamp の common 部分集合。
        let axis = match kind {
            SensorKind::Accelerometer => 1.0,
            SensorKind::Gyroscope => 2.0,
            SensorKind::Magnetometer => 3.0,
        };
        Ok(SensorSample {
            x: axis,
            y: axis,
            z: axis,
            timestamp: 0.0,
        })
    }

    fn subscribe(
        &mut self,
        kind: SensorKind,
    ) -> Result<Subscription<SensorSample>, CapabilityError> {
        let (subscription, source) = Subscription::new(|| {});
        self.kind = Some(kind);
        self.source = Some(source);
        Ok(subscription)
    }
}

#[test]
fn high_frequency_sensor_samples_drain_in_order_without_loss() {
    let mut sensors = FakeSensors::new();
    let mut subscription = sensors
        .subscribe(SensorKind::Accelerometer)
        .expect("fake provider は subscribe を満たす");

    // 200Hz 級の高頻度ストリーム: 1 フレーム分の多数サンプルを一気に流し込む。`poll_changes ->
    // Vec` は coalesce せずサンプルを 1 件も落とさず全件返す（coalesce は consumer 裁量・ADR-0120）。
    let samples: Vec<SensorSample> = (0..256)
        .map(|i| SensorSample {
            x: i as f64,
            y: (i * 2) as f64,
            z: (i * 3) as f64,
            timestamp: i as f64 * 0.005,
        })
        .collect();
    for sample in &samples {
        sensors.emit(*sample);
    }

    // 順序保持・全件 drain（高頻度でもサンプル落ちなし）。
    assert_eq!(
        subscription.poll_changes(),
        samples,
        "高頻度の複数サンプルが poll_changes で順序保持・全件 drain される（サンプル落ちなし）"
    );
    // drain 後・新規 emit 無しなら 2 回目は空。
    assert!(subscription.poll_changes().is_empty());
}
