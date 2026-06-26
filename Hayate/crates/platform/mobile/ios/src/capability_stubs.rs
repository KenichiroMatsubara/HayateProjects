//! iOS leaf の capability scaffold stub（ADR-0119）。
//!
//! Android leaf（`capability_stubs.rs`）の鏡写し。各 capability を「型として存在し、呼べば
//! `Err(Unimplemented)` を返す」状態で置く。契約の正本は `hayate_core`、stub は Flutter
//! `platform_interface` の throw-by-default を Rust の `Err` で表す（panic しない）。実機実装が
//! 入ると各 stub は自分のモジュール（`#[cfg(target_os="ios")]` FFI glue つき）へ昇格する。
//!
//! clipboard は capability に含めない（編集境界 `element::clipboard::Clipboard`・ADR-0097 が所有）。

use hayate_core::capability::CapabilityError;
use hayate_core::{
    Battery, BatteryStatus, Biometric, Connectivity, ConnectivityProvider, DeviceInfo,
    DeviceInfoProvider, FileFilter, FilePicker, Geolocation, HapticKind, Haptics, KeyValueStore,
    LocalNotification, LocalNotifications, PickedFile, Position, SavePath, SecureStorage,
    SensorKind, SensorSample, Sensors, Share, Subscription, UrlLauncher,
};

/// この leaf の platform 名（`CapabilityError` に載る）。
const PLATFORM: &str = "ios";

/// `Unimplemented` を本 leaf の platform 名で作る簡約子。
fn ni(capability: &'static str) -> CapabilityError {
    CapabilityError::unimplemented(capability, PLATFORM)
}

/// haptics の iOS stub（実装時 `UIImpactFeedbackGenerator` / `UISelectionFeedbackGenerator`）。
#[derive(Default)]
pub struct IosHaptics;
impl Haptics for IosHaptics {
    fn feedback(&mut self, _kind: HapticKind) -> Result<(), CapabilityError> {
        Err(ni("haptics"))
    }
}

/// local notification の iOS stub（実装時 `UNUserNotificationCenter`）。
#[derive(Default)]
pub struct IosLocalNotifications;
impl LocalNotifications for IosLocalNotifications {
    fn show(&mut self, _notification: LocalNotification) -> Result<(), CapabilityError> {
        Err(ni("local_notification"))
    }
    fn cancel(&mut self, _id: i32) -> Result<(), CapabilityError> {
        Err(ni("local_notification"))
    }
    fn cancel_all(&mut self) -> Result<(), CapabilityError> {
        Err(ni("local_notification"))
    }
}

/// url launcher の iOS stub（実装時 `UIApplication.open(_:)`）。
#[derive(Default)]
pub struct IosUrlLauncher;
impl UrlLauncher for IosUrlLauncher {
    fn can_launch(&self, _url: &str) -> Result<bool, CapabilityError> {
        Err(ni("url_launcher"))
    }
    fn launch(&mut self, _url: &str) -> Result<bool, CapabilityError> {
        Err(ni("url_launcher"))
    }
}

/// secure storage の iOS stub（実装時 Keychain Services）。
#[derive(Default)]
pub struct IosSecureStorage;
impl SecureStorage for IosSecureStorage {
    fn read(&self, _key: &str) -> Result<Option<String>, CapabilityError> {
        Err(ni("secure_storage"))
    }
    fn write(&mut self, _key: &str, _value: &str) -> Result<(), CapabilityError> {
        Err(ni("secure_storage"))
    }
    fn delete(&mut self, _key: &str) -> Result<(), CapabilityError> {
        Err(ni("secure_storage"))
    }
}

/// device info の iOS stub（実装時 `UIDevice`）。
#[derive(Default)]
pub struct IosDeviceInfo;
impl DeviceInfoProvider for IosDeviceInfo {
    fn query(&self) -> Result<DeviceInfo, CapabilityError> {
        Err(ni("device_info"))
    }
}

/// share の iOS stub（実装時 `UIActivityViewController`）。
#[derive(Default)]
pub struct IosShare;
impl Share for IosShare {
    fn share_text(&mut self, _text: &str, _subject: Option<&str>) -> Result<(), CapabilityError> {
        Err(ni("share"))
    }
}

/// file picker の iOS stub（実装時 `UIDocumentPickerViewController`）。
#[derive(Default)]
pub struct IosFilePicker;
impl FilePicker for IosFilePicker {
    fn open_file(&mut self, _filter: &FileFilter) -> Result<Option<PickedFile>, CapabilityError> {
        Err(ni("file_picker"))
    }
    fn save_file(&mut self, _suggested_name: &str) -> Result<Option<SavePath>, CapabilityError> {
        Err(ni("file_picker"))
    }
}

/// key-value storage の iOS stub（実装時 `UserDefaults`）。
#[derive(Default)]
pub struct IosKeyValueStore;
impl KeyValueStore for IosKeyValueStore {
    fn get_string(&self, _key: &str) -> Result<Option<String>, CapabilityError> {
        Err(ni("key_value_store"))
    }
    fn set_string(&mut self, _key: &str, _value: &str) -> Result<(), CapabilityError> {
        Err(ni("key_value_store"))
    }
    fn remove(&mut self, _key: &str) -> Result<(), CapabilityError> {
        Err(ni("key_value_store"))
    }
    fn contains_key(&self, _key: &str) -> Result<bool, CapabilityError> {
        Err(ni("key_value_store"))
    }
}

/// battery の iOS stub（wave-2・実装時 `UIDevice` battery monitoring / `batteryLevelDidChange`）。
/// `query`/`subscribe` とも `Err(Unimplemented)`（ストリーム購読も含め未実装・ADR-0120）。
#[derive(Default)]
pub struct IosBattery;
impl Battery for IosBattery {
    fn query(&self) -> Result<BatteryStatus, CapabilityError> {
        Err(ni("battery"))
    }
    fn subscribe(&mut self) -> Result<Subscription<BatteryStatus>, CapabilityError> {
        Err(ni("battery"))
    }
}

/// connectivity の iOS stub（wave-2・実装時 `NWPathMonitor`）。`query`/`subscribe` とも
/// `Err(Unimplemented)`（接続変化の native 登録も含め未実装・ADR-0120）。
#[derive(Default)]
pub struct IosConnectivity;
impl ConnectivityProvider for IosConnectivity {
    fn query(&self) -> Result<Connectivity, CapabilityError> {
        Err(ni("connectivity"))
    }
    fn subscribe(&mut self) -> Result<Subscription<Connectivity>, CapabilityError> {
        Err(ni("connectivity"))
    }
}

/// geolocation の iOS stub（wave-2・実装時 `CLLocationManager`）。`query`/`subscribe` とも
/// `Err(Unimplemented)`（位置変化の native 登録も含め未実装・ADR-0120）。権限ゲート付きだが
/// scaffold では権限据え置き（`PermissionDenied` は返さず `Unimplemented` のまま・ADR-0119/0120）。
#[derive(Default)]
pub struct IosGeolocation;
impl Geolocation for IosGeolocation {
    fn query(&self) -> Result<Position, CapabilityError> {
        Err(ni("geolocation"))
    }
    fn subscribe(&mut self) -> Result<Subscription<Position>, CapabilityError> {
        Err(ni("geolocation"))
    }
}

/// sensors の iOS stub（wave-2・実装時 `CMMotionManager`）。単一 trait ＋ `SensorKind` 引数で全
/// sensor を出し分ける。`query`/`subscribe` とも全 kind で `Err(Unimplemented)`（高頻度センサ
/// ストリームの native 登録も含め未実装・ADR-0120）。
#[derive(Default)]
pub struct IosSensors;
impl Sensors for IosSensors {
    fn query(&self, _kind: SensorKind) -> Result<SensorSample, CapabilityError> {
        Err(ni("sensors"))
    }
    fn subscribe(
        &mut self,
        _kind: SensorKind,
    ) -> Result<Subscription<SensorSample>, CapabilityError> {
        Err(ni("sensors"))
    }
}

/// biometric の iOS stub（実装時 `LAContext`）。
#[derive(Default)]
pub struct IosBiometric;
impl Biometric for IosBiometric {
    fn is_available(&self) -> Result<bool, CapabilityError> {
        Err(ni("biometric"))
    }
    fn authenticate(&mut self, _reason: &str) -> Result<bool, CapabilityError> {
        Err(ni("biometric"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// scaffold の核となる性質: 各 stub は呼ぶと panic せず `Err(Unimplemented{platform:"ios"})`
    /// を返す。Android leaf と対称に host 検証する（ADR-0119）。
    #[test]
    fn every_stub_reports_unimplemented_on_ios() {
        assert_eq!(IosHaptics.feedback(HapticKind::Vibrate), Err(ni("haptics")));
        assert_eq!(
            IosLocalNotifications.cancel_all(),
            Err(ni("local_notification"))
        );
        assert_eq!(
            IosUrlLauncher.can_launch("https://x"),
            Err(ni("url_launcher"))
        );
        assert_eq!(IosSecureStorage.read("k"), Err(ni("secure_storage")));
        assert_eq!(IosDeviceInfo.query(), Err(ni("device_info")));
        assert_eq!(IosShare.share_text("t", None), Err(ni("share")));
        assert_eq!(
            IosFilePicker.open_file(&FileFilter::default()),
            Err(ni("file_picker"))
        );
        assert_eq!(
            IosKeyValueStore.contains_key("k"),
            Err(ni("key_value_store"))
        );
        assert_eq!(IosBiometric.is_available(), Err(ni("biometric")));
        // wave-2 battery（ADR-0120）: query/subscribe とも Unimplemented を返し panic しない。
        assert_eq!(IosBattery.query(), Err(ni("battery")));
        assert_eq!(
            IosBattery.subscribe().map(|_| ()),
            Err(ni("battery")),
            "battery subscribe も未実装（ストリーム購読の native 登録はまだ無い）"
        );
        // wave-2 connectivity（ADR-0120）: battery と同型。query/subscribe とも Unimplemented。
        assert_eq!(IosConnectivity.query(), Err(ni("connectivity")));
        assert_eq!(
            IosConnectivity.subscribe().map(|_| ()),
            Err(ni("connectivity")),
            "connectivity subscribe も未実装（接続変化の native 登録はまだ無い）"
        );
        // wave-2 geolocation（ADR-0120）: battery と同型。query/subscribe とも Unimplemented。
        // 権限ゲート付きだが scaffold では権限据え置き（`PermissionDenied` は返さない）。
        assert_eq!(IosGeolocation.query(), Err(ni("geolocation")));
        assert_eq!(
            IosGeolocation.subscribe().map(|_| ()),
            Err(ni("geolocation")),
            "geolocation subscribe も未実装（位置変化の native 登録はまだ無い）"
        );
        // wave-2 sensors（ADR-0120）: 単一 trait ＋ SensorKind 引数。query/subscribe とも全 kind で
        // Unimplemented を返し panic しない（高頻度ストリームの native 登録はまだ無い）。
        for kind in [
            SensorKind::Accelerometer,
            SensorKind::Gyroscope,
            SensorKind::Magnetometer,
        ] {
            assert_eq!(IosSensors.query(kind), Err(ni("sensors")));
            assert_eq!(
                IosSensors.subscribe(kind).map(|_| ()),
                Err(ni("sensors")),
                "sensors subscribe も未実装（センサストリームの native 登録はまだ無い）"
            );
        }
    }

    /// platform 名が正しく載る（android leaf と取り違えていない）。
    #[test]
    fn unimplemented_error_names_the_ios_platform() {
        assert_eq!(
            IosHaptics.feedback(HapticKind::LightImpact),
            Err(CapabilityError::Unimplemented {
                capability: "haptics",
                platform: "ios"
            })
        );
    }
}
