//! Android leaf の capability scaffold stub（ADR-0119）。
//!
//! wave-1 の各 capability を「型として存在し、呼べば `Err(Unimplemented)` を返す」状態で
//! 置く。契約の正本は `hayate_core`、stub は throw-by-default（Flutter `platform_interface`
//! の写し）を Rust の `Err` で表す。panic はしない。実機実装が入ると各 stub は `audio_output`
//! と同じく自分のモジュール（FFI glue つき）へ昇格する — それまでは host でコンパイル/テスト
//! できる純粋 stub に保つ（NDK 不要）。
//!
//! clipboard は capability に含めない（編集境界 `element::clipboard::Clipboard`・ADR-0097 が所有）。

use hayate_core::capability::CapabilityError;
use hayate_core::{
    Battery, BatteryStatus, Biometric, Connectivity, ConnectivityProvider, DeviceInfo,
    DeviceInfoProvider, FileFilter, FilePicker, Geolocation, HapticKind, Haptics, KeyValueStore,
    LocalNotification, LocalNotifications, PickedFile, Position, SavePath, SecureStorage, Share,
    Subscription, UrlLauncher,
};

/// この leaf の platform 名（`CapabilityError` に載る）。
const PLATFORM: &str = "android";

/// `Unimplemented` を本 leaf の platform 名で作る簡約子。
fn ni(capability: &'static str) -> CapabilityError {
    CapabilityError::unimplemented(capability, PLATFORM)
}

/// haptics の Android stub（実装時 `HapticFeedbackConstants` / `Vibrator`）。
#[derive(Default)]
pub struct AndroidHaptics;
impl Haptics for AndroidHaptics {
    fn feedback(&mut self, _kind: HapticKind) -> Result<(), CapabilityError> {
        Err(ni("haptics"))
    }
}

/// local notification の Android stub（実装時 `NotificationManager`）。
#[derive(Default)]
pub struct AndroidLocalNotifications;
impl LocalNotifications for AndroidLocalNotifications {
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

/// url launcher の Android stub（実装時 `Intent.ACTION_VIEW`）。
#[derive(Default)]
pub struct AndroidUrlLauncher;
impl UrlLauncher for AndroidUrlLauncher {
    fn can_launch(&self, _url: &str) -> Result<bool, CapabilityError> {
        Err(ni("url_launcher"))
    }
    fn launch(&mut self, _url: &str) -> Result<bool, CapabilityError> {
        Err(ni("url_launcher"))
    }
}

/// secure storage の Android stub（実装時 Keystore + EncryptedSharedPreferences）。
#[derive(Default)]
pub struct AndroidSecureStorage;
impl SecureStorage for AndroidSecureStorage {
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

/// device info の Android stub（実装時 `android.os.Build`）。
#[derive(Default)]
pub struct AndroidDeviceInfo;
impl DeviceInfoProvider for AndroidDeviceInfo {
    fn query(&self) -> Result<DeviceInfo, CapabilityError> {
        Err(ni("device_info"))
    }
}

/// share の Android stub（実装時 `Intent.ACTION_SEND`）。
#[derive(Default)]
pub struct AndroidShare;
impl Share for AndroidShare {
    fn share_text(&mut self, _text: &str, _subject: Option<&str>) -> Result<(), CapabilityError> {
        Err(ni("share"))
    }
}

/// file picker の Android stub（実装時 Storage Access Framework）。
#[derive(Default)]
pub struct AndroidFilePicker;
impl FilePicker for AndroidFilePicker {
    fn open_file(&mut self, _filter: &FileFilter) -> Result<Option<PickedFile>, CapabilityError> {
        Err(ni("file_picker"))
    }
    fn save_file(&mut self, _suggested_name: &str) -> Result<Option<SavePath>, CapabilityError> {
        Err(ni("file_picker"))
    }
}

/// key-value storage の Android stub（実装時 `SharedPreferences`）。
#[derive(Default)]
pub struct AndroidKeyValueStore;
impl KeyValueStore for AndroidKeyValueStore {
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

/// battery の Android stub（wave-2・実装時 `BatteryManager` / `ACTION_BATTERY_CHANGED`）。
/// `query`/`subscribe` とも `Err(Unimplemented)`（ストリーム購読も含め未実装・ADR-0120）。
#[derive(Default)]
pub struct AndroidBattery;
impl Battery for AndroidBattery {
    fn query(&self) -> Result<BatteryStatus, CapabilityError> {
        Err(ni("battery"))
    }
    fn subscribe(&mut self) -> Result<Subscription<BatteryStatus>, CapabilityError> {
        Err(ni("battery"))
    }
}

/// connectivity の Android stub（wave-2・実装時 `ConnectivityManager` /
/// `NetworkCallback`）。`query`/`subscribe` とも `Err(Unimplemented)`（接続変化の native
/// 登録も含め未実装・ADR-0120）。
#[derive(Default)]
pub struct AndroidConnectivity;
impl ConnectivityProvider for AndroidConnectivity {
    fn query(&self) -> Result<Connectivity, CapabilityError> {
        Err(ni("connectivity"))
    }
    fn subscribe(&mut self) -> Result<Subscription<Connectivity>, CapabilityError> {
        Err(ni("connectivity"))
    }
}

/// geolocation の Android stub（wave-2・実装時 `FusedLocationProviderClient` /
/// `LocationManager`）。`query`/`subscribe` とも `Err(Unimplemented)`（位置変化の native 登録も
/// 含め未実装・ADR-0120）。権限ゲート付きだが scaffold では権限据え置き（`PermissionDenied` は
/// 返さず `Unimplemented` のまま・ADR-0119/0120）。
#[derive(Default)]
pub struct AndroidGeolocation;
impl Geolocation for AndroidGeolocation {
    fn query(&self) -> Result<Position, CapabilityError> {
        Err(ni("geolocation"))
    }
    fn subscribe(&mut self) -> Result<Subscription<Position>, CapabilityError> {
        Err(ni("geolocation"))
    }
}

/// biometric の Android stub（実装時 `BiometricPrompt`）。
#[derive(Default)]
pub struct AndroidBiometric;
impl Biometric for AndroidBiometric {
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

    /// scaffold の核となる性質: 各 stub は呼ぶと panic せず `Err(Unimplemented{platform:"android"})`
    /// を返す。実機 SDK 無しに公開契約越しでホスト検証できる（ADR-0119）。
    #[test]
    fn every_stub_reports_unimplemented_on_android() {
        assert_eq!(
            AndroidHaptics.feedback(HapticKind::Vibrate),
            Err(ni("haptics"))
        );
        assert_eq!(
            AndroidLocalNotifications.cancel_all(),
            Err(ni("local_notification"))
        );
        assert_eq!(AndroidUrlLauncher.can_launch("https://x"), Err(ni("url_launcher")));
        assert_eq!(AndroidSecureStorage.read("k"), Err(ni("secure_storage")));
        assert_eq!(AndroidDeviceInfo.query(), Err(ni("device_info")));
        assert_eq!(AndroidShare.share_text("t", None), Err(ni("share")));
        assert_eq!(
            AndroidFilePicker.open_file(&FileFilter::default()),
            Err(ni("file_picker"))
        );
        assert_eq!(
            AndroidKeyValueStore.contains_key("k"),
            Err(ni("key_value_store"))
        );
        assert_eq!(AndroidBiometric.is_available(), Err(ni("biometric")));
        // wave-2 battery（ADR-0120）: query/subscribe とも Unimplemented を返し panic しない。
        assert_eq!(AndroidBattery.query(), Err(ni("battery")));
        assert_eq!(
            AndroidBattery.subscribe().map(|_| ()),
            Err(ni("battery")),
            "battery subscribe も未実装（ストリーム購読の native 登録はまだ無い）"
        );
        // wave-2 connectivity（ADR-0120）: battery と同型。query/subscribe とも Unimplemented。
        assert_eq!(AndroidConnectivity.query(), Err(ni("connectivity")));
        assert_eq!(
            AndroidConnectivity.subscribe().map(|_| ()),
            Err(ni("connectivity")),
            "connectivity subscribe も未実装（接続変化の native 登録はまだ無い）"
        );
        // wave-2 geolocation（ADR-0120）: battery と同型。query/subscribe とも Unimplemented。
        // 権限ゲート付きだが scaffold では権限据え置き（`PermissionDenied` は返さない）。
        assert_eq!(AndroidGeolocation.query(), Err(ni("geolocation")));
        assert_eq!(
            AndroidGeolocation.subscribe().map(|_| ()),
            Err(ni("geolocation")),
            "geolocation subscribe も未実装（位置変化の native 登録はまだ無い）"
        );
    }

    /// platform 名が正しく載る（ios leaf と取り違えていない）。
    #[test]
    fn unimplemented_error_names_the_android_platform() {
        assert_eq!(
            AndroidHaptics.feedback(HapticKind::LightImpact),
            Err(CapabilityError::Unimplemented {
                capability: "haptics",
                platform: "android"
            })
        );
    }
}
