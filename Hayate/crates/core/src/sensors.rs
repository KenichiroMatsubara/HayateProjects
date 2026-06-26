//! sensors capability 契約（ADR-0120 wave-2）。モデル: Web `DeviceMotion`/`DeviceOrientation`
//! （`accelerationIncludingGravity` 等）／Flutter `sensors_plus`（`accelerometerEvents` 等）。
//!
//! battery / geolocation が確立した wave-2 ストリーム capability の共有契約土台（Core 所有 RAII
//! [`Subscription`]・ADR-0120）を再利用しつつ、**形が一段違う**: sensor ごとに trait を増やさず、
//! 単一 trait ＋ [`SensorKind`] 引数で出し分ける。`query`（現在値の単発取得）と `subscribe`
//! （変化ストリーム）の 2 メソッドを持ち、契約が保証するのは「`subscribe` が変化を流す」ことだけ。
//!
//! sensor ストリームは 200Hz 級の高頻度になり得る。共有土台の [`Subscription::poll_changes`] が
//! 蓄積サンプルを**順序保持で全件 `Vec` drain**し**1 件も落とさない**ので、coalesce（間引き）が
//! 要る consumer は drain 後に自前で行う（coalesce は consumer 裁量・ADR-0120）。

use crate::capability::CapabilityError;
use crate::subscription::Subscription;

/// 取得対象のセンサ種別 seed。kind で出し分け、sensor ごとに trait を増やさない（ADR-0120）。
/// platform 固有のセンサ（気圧計・近接など）は実機実装時に variant を足す（「先置きしない」）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SensorKind {
    /// 加速度センサ（重力込み・`m/s^2`）。Web `devicemotion` / Flutter `accelerometerEvents` 同型。
    Accelerometer,
    /// ジャイロ（角速度・`rad/s`）。Flutter `gyroscopeEvents` 同型。
    Gyroscope,
    /// 地磁気センサ（`uT`）。Flutter `magnetometerEvents` 同型。
    Magnetometer,
}

/// 全 platform 共通に取れるセンサ標本の common 部分集合 seed。3軸 `f64` ＋ timestamp で、kind に
/// よらず同形（軸の意味は kind で決まる）。platform 固有フィールド（精度ランク・補正状態など）は
/// 実機実装時に拡張する（`BatteryStatus` / `Position` と同じ流儀・ADR-0120）。`f64` を含むため
/// `Eq`/`Hash` は導出しない。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SensorSample {
    /// x 軸成分。単位は kind に従う（加速度なら `m/s^2` 等）。
    pub x: f64,
    /// y 軸成分。
    pub y: f64,
    /// z 軸成分。
    pub z: f64,
    /// サンプル時刻（秒・単調増加）。Web `Event.timeStamp` / native sensor timestamp 同型。
    pub timestamp: f64,
}

/// センサ値の単発取得＋変化ストリーム購読。単一 trait ＋ [`SensorKind`] 引数で全 sensor を出し分け
/// る（`Battery` / `Geolocation` が capability ごとに trait を切るのと**一段違う**形・ADR-0120）。
pub trait Sensors {
    /// 指定 [`SensorKind`] の現在値を単発取得する（wave-1 同型）。
    fn query(&self, kind: SensorKind) -> Result<SensorSample, CapabilityError>;

    /// 指定 [`SensorKind`] の変化ストリームを購読する。返る [`Subscription`] を drain すると蓄積
    /// されたサンプルが順序保持・全件で得られ（高頻度でも落ちない）、ハンドルの drop で購読が解除
    /// される。
    fn subscribe(
        &mut self,
        kind: SensorKind,
    ) -> Result<Subscription<SensorSample>, CapabilityError>;
}
