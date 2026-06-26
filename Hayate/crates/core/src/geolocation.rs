//! geolocation capability 契約（ADR-0120 wave-2）。モデル: Web Geolocation API
//! （`getCurrentPosition` の単発取得＋`watchPosition` の変化ストリーム）／Flutter `geolocator`。
//!
//! battery が確立した wave-2 ストリーム capability の共有契約土台（Core 所有 RAII
//! [`Subscription`]・ADR-0120）を再利用する。`query`（現在位置・`getCurrentPosition` 同型）と
//! `subscribe`（位置変化ストリーム・`watchPosition` 同型）の 2 メソッドを持ち、契約が保証するのは
//! 「`subscribe` が変化を流す」ことだけ（初期位置が要る consumer は `query` を併用）。
//!
//! **権限は据え置き（ADR-0119/0120）**: geolocation は権限ゲート付きだが、scaffold 段階では
//! stub が `Err(Unimplemented)` を返すのみで、[`CapabilityError`] に `PermissionDenied` variant は
//! 足さない（実機実装で native 権限フローを通すときに追加する・error variant も「先置きしない」）。

use crate::capability::CapabilityError;
use crate::subscription::Subscription;

/// 全 platform 共通に取れる位置情報の common 部分集合 seed。platform 固有フィールド
/// （altitude・heading・speed・timestamp など）は実機実装時に拡張する（`BatteryStatus` /
/// `Connectivity` と同じ流儀・ADR-0120）。`f64` を含むため `Eq`/`Hash` は導出しない。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Position {
    /// 緯度（度・WGS84）。Web `coords.latitude` 同型。
    pub lat: f64,
    /// 経度（度・WGS84）。Web `coords.longitude` 同型。
    pub lng: f64,
    /// 水平精度（メートル・`coords.accuracy` 同型）。値が小さいほど高精度。
    pub accuracy: f64,
}

/// 位置情報の単発取得＋変化ストリーム購読。値型 [`Position`] と対をなす Core 契約
/// （`Battery` / `ConnectivityProvider` と同型・ADR-0120）。
pub trait Geolocation {
    /// 現在位置を単発取得する（`getCurrentPosition` 同型）。
    fn query(&self) -> Result<Position, CapabilityError>;

    /// 位置変化ストリームを購読する（`watchPosition` 同型）。返る [`Subscription`] を drain すると
    /// 蓄積された変化が順序保持で得られ、ハンドルの drop で購読が解除される。
    fn subscribe(&mut self) -> Result<Subscription<Position>, CapabilityError>;
}
