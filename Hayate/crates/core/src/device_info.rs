//! device info capability 契約（ADR-0119）。モデル: `device_info_plus`
//! （platform 別の大きな struct を返す）。Core では common 部分集合のみを持つ。

use crate::capability::CapabilityError;

/// 全 platform 共通に取れるデバイス情報の部分集合。platform 固有フィールドは実装時に拡張。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeviceInfo {
    pub os_name: String,
    pub os_version: String,
    pub model: String,
    pub manufacturer: String,
}

/// デバイス情報の単発取得。
pub trait DeviceInfoProvider {
    fn query(&self) -> Result<DeviceInfo, CapabilityError>;
}
