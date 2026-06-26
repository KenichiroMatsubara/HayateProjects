//! battery capability 契約（ADR-0120 wave-2）。モデル: Flutter `battery_plus`
//! （`batteryLevel` の単発取得＋`onBatteryStateChanged` の変化ストリーム）。
//!
//! ストリーム capability の共有契約土台を battery 一本で端から端まで通すトレーサーバレット。
//! `query`（現在値・wave-1 同型）と `subscribe`（変化ストリーム）の 2 メソッドを持ち、契約が
//! 保証するのは「`subscribe` が変化を流す」ことだけ（初期値が要る consumer は `query` を併用）。

use crate::capability::CapabilityError;
use crate::subscription::Subscription;

/// 全 platform 共通に取れる電池状態の common 部分集合 seed。platform 固有フィールド
/// （温度・電源種別など）は実機実装時に拡張する（`DeviceInfo` と同じ流儀・ADR-0120）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BatteryStatus {
    /// 充電残量（パーセント・`0..=100`）。Flutter `battery_plus` の `batteryLevel` 同型。
    pub level: u8,
    /// 充電中か否か。
    pub charging: bool,
}

/// 電池状態の単発取得＋変化ストリーム購読。
pub trait Battery {
    /// 現在の電池状態を単発取得する（wave-1 同型）。
    fn query(&self) -> Result<BatteryStatus, CapabilityError>;

    /// 電池状態の変化ストリームを購読する。返る [`Subscription`] を drain すると蓄積された
    /// 変化が順序保持で得られ、ハンドルの drop で購読が解除される。
    fn subscribe(&mut self) -> Result<Subscription<BatteryStatus>, CapabilityError>;
}
