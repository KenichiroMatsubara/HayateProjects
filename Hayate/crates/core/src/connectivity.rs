//! connectivity capability 契約（ADR-0120 wave-2）。モデル: Flutter `connectivity_plus`
//! （`checkConnectivity` の単発取得＋`onConnectivityChanged` の変化ストリーム）。
//!
//! wave-2 ストリーム capability の共有契約土台（battery が確立・ADR-0120）を再利用する。
//! `query`（現在の接続種別・wave-1 同型）と `subscribe`（変化ストリーム）の 2 メソッドを持ち、
//! 契約が保証するのは「`subscribe` が変化を流す」ことだけ（初期値が要る consumer は `query` を併用）。
//! 「最新状態だけ見たい」離散遷移も [`Subscription`] の `Vec` drain で表現できる（最後だけ見る）。

use crate::capability::CapabilityError;
use crate::subscription::Subscription;

/// 全 platform 共通に取れる接続種別の common 部分集合 seed。platform 固有の種別
/// （ethernet・vpn・bluetooth など）は実機実装時に拡張する（`BatteryStatus` と同じ流儀・ADR-0120）。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Connectivity {
    /// 接続なし（オフライン）。値が無い状態を型に持たせる既定値。
    #[default]
    None,
    /// Wi-Fi 接続。
    Wifi,
    /// モバイル回線（cellular）接続。
    Cellular,
}

/// 接続種別の単発取得＋変化ストリーム購読。値型 [`Connectivity`] と対をなす Core 契約
/// （`DeviceInfo` / `DeviceInfoProvider` と同型の命名・ADR-0120）。
pub trait ConnectivityProvider {
    /// 現在の接続種別を単発取得する（wave-1 同型）。
    fn query(&self) -> Result<Connectivity, CapabilityError>;

    /// 接続種別の変化ストリームを購読する。返る [`Subscription`] を drain すると蓄積された
    /// 変化が順序保持で得られ、ハンドルの drop で購読が解除される。
    fn subscribe(&mut self) -> Result<Subscription<Connectivity>, CapabilityError>;
}
