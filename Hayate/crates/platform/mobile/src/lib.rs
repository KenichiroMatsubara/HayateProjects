//! Mobile Family Adapter（ADR-0117）。
//!
//! family（android + ios）で統一できる platform-bound capability（音声出力）を、ビルド時
//! `cfg(target_os)` で片方の leaf 実装をリンクして単一 facade として上位へ露出する。これは
//! ランタイム dispatch ではない（Flutter channel / RN bridge の機構は借りない）— cargo が
//! ターゲットごとに正確に片方の leaf をリンクする。capability 契約の正本は常に Core
//! （[`hayate_core::AudioOutput`]）であり、本 crate はそれを再露出するだけで別契約を切らない。
//! `web` は family of 1 のため Family Adapter を持たず leaf を直接置く。
//!
//! 今 facade に載る capability は音声出力のみ（android = `AudioTrack` / ios = `AVAudioEngine`）。
//! 他 capability は 2 実装が揃ってから足す（空 facade を先置きしない）。

// 契約・形式・named constant は Core が正本。上位はこの再露出を通じて使う。
pub use hayate_core::{
    AudioFormat, AudioOutput, DEFAULT_BUFFER_FRAMES, DEFAULT_CHANNEL_COUNT, DEFAULT_SAMPLE_RATE_HZ,
};

// wave-1 capability scaffold の契約・型も Core が正本。facade はこれを再露出するだけで別契約を
// 切らない（ADR-0119）。clipboard は capability に含めない（ADR-0097 の編集境界が所有）。
pub use hayate_core::{
    Biometric, CapabilityError, DeviceInfo, DeviceInfoProvider, FileFilter, FilePicker, HapticKind,
    Haptics, KeyValueStore, LocalNotification, LocalNotifications, PickedFile, SavePath,
    SecureStorage, Share, UrlLauncher,
};

// QR スキャナ capability（ADR-0125）も Core が正本。facade は契約・値型を再露出するだけ。
// async-UI 一発取得（file_picker と同型）。android = Google Code Scanner / ios = VisionKit。
pub use hayate_core::{QrScanner, ScannedCode};

// wave-2 ストリーム capability の共有契約土台（ADR-0120）も Core が正本。facade は battery trait・
// 値型・RAII 購読ハンドル（`Subscription`）と producer 側（`SubscriptionSource`）を再露出するだけ
// で別契約を切らない。
pub use hayate_core::{Battery, BatteryStatus, Subscription, SubscriptionSource};

// connectivity（ADR-0120）も同じ wave-2 契約土台を再利用する。facade は値型 `Connectivity` と
// trait `ConnectivityProvider` を再露出するだけで別契約を切らない。
pub use hayate_core::{Connectivity, ConnectivityProvider};

// geolocation（ADR-0120）も同じ wave-2 契約土台を再利用する。facade は値型 `Position` と trait
// `Geolocation` を再露出するだけで別契約を切らない（権限は据え置き・ADR-0119/0120）。
pub use hayate_core::{Geolocation, Position};

// sensors（ADR-0120）も同じ wave-2 契約土台を再利用する。facade は単一 trait `Sensors`・kind enum
// `SensorKind`・値型 `SensorSample` を再露出するだけで別契約を切らない（高頻度 drain は土台の
// `poll_changes -> Vec` が全件保持）。
pub use hayate_core::{SensorKind, SensorSample, Sensors};

/// family 統一の音声出力 facade。ビルド対象に応じて、Core の [`AudioOutput`] を満たす leaf
/// 実装（android = `AudioTrack` / ios = `AVAudioEngine`）へ解決する単一の型名。上位は leaf を
/// 名指しせず本 facade だけを参照する。
#[cfg(target_os = "android")]
pub type MobileAudioOutput = hayate_adapter_android::audio_output::AudioTrackOutput;

/// family 統一の音声出力 facade。ビルド対象に応じて、Core の [`AudioOutput`] を満たす leaf
/// 実装（android = `AudioTrack` / ios = `AVAudioEngine`）へ解決する単一の型名。上位は leaf を
/// 名指しせず本 facade だけを参照する。
#[cfg(target_os = "ios")]
pub type MobileAudioOutput = hayate_adapter_ios::audio_output::AvAudioEngineOutput;

// --- wave-1 capability scaffold facade（ADR-0119）---
// audio と同型: 各 capability につき統一 facade 型名 1 つを、ビルド対象に応じて android/ios の
// leaf stub へ cfg で解決する。上位は leaf を名指しせず `MobileXxx` だけを参照する。stub は
// 呼ぶと `Err(Unimplemented)` を返す（実機実装で leaf 中身が差し替わっても facade 名は不変）。

#[cfg(target_os = "android")]
pub type MobileHaptics = hayate_adapter_android::capability_stubs::AndroidHaptics;
#[cfg(target_os = "ios")]
pub type MobileHaptics = hayate_adapter_ios::capability_stubs::IosHaptics;

#[cfg(target_os = "android")]
pub type MobileLocalNotifications =
    hayate_adapter_android::capability_stubs::AndroidLocalNotifications;
#[cfg(target_os = "ios")]
pub type MobileLocalNotifications = hayate_adapter_ios::capability_stubs::IosLocalNotifications;

#[cfg(target_os = "android")]
pub type MobileUrlLauncher = hayate_adapter_android::capability_stubs::AndroidUrlLauncher;
#[cfg(target_os = "ios")]
pub type MobileUrlLauncher = hayate_adapter_ios::capability_stubs::IosUrlLauncher;

#[cfg(target_os = "android")]
pub type MobileSecureStorage = hayate_adapter_android::capability_stubs::AndroidSecureStorage;
#[cfg(target_os = "ios")]
pub type MobileSecureStorage = hayate_adapter_ios::capability_stubs::IosSecureStorage;

#[cfg(target_os = "android")]
pub type MobileDeviceInfo = hayate_adapter_android::capability_stubs::AndroidDeviceInfo;
#[cfg(target_os = "ios")]
pub type MobileDeviceInfo = hayate_adapter_ios::capability_stubs::IosDeviceInfo;

#[cfg(target_os = "android")]
pub type MobileShare = hayate_adapter_android::capability_stubs::AndroidShare;
#[cfg(target_os = "ios")]
pub type MobileShare = hayate_adapter_ios::capability_stubs::IosShare;

#[cfg(target_os = "android")]
pub type MobileFilePicker = hayate_adapter_android::capability_stubs::AndroidFilePicker;
#[cfg(target_os = "ios")]
pub type MobileFilePicker = hayate_adapter_ios::capability_stubs::IosFilePicker;

#[cfg(target_os = "android")]
pub type MobileKeyValueStore = hayate_adapter_android::capability_stubs::AndroidKeyValueStore;
#[cfg(target_os = "ios")]
pub type MobileKeyValueStore = hayate_adapter_ios::capability_stubs::IosKeyValueStore;

#[cfg(target_os = "android")]
pub type MobileBiometric = hayate_adapter_android::capability_stubs::AndroidBiometric;
#[cfg(target_os = "ios")]
pub type MobileBiometric = hayate_adapter_ios::capability_stubs::IosBiometric;

// QR スキャナ facade（ADR-0125）。android は実機実装（Code Scanner・`qr_scanner` モジュールへ昇格）、
// ios は stub（VisionKit 実装は後）。上位は leaf を名指しせず `MobileQrScanner` だけを参照し、
// iOS/Android を単一 API で扱う。web は family-of-1 で別 leaf（host-web の `scanQrFromCamera`）。
#[cfg(target_os = "android")]
pub type MobileQrScanner = hayate_adapter_android::qr_scanner::AndroidQrScanner;
#[cfg(target_os = "ios")]
pub type MobileQrScanner = hayate_adapter_ios::capability_stubs::IosQrScanner;

// --- wave-2 stream capability scaffold facade（ADR-0120）---
// battery が wave-2 ストリーム契約土台のトレーサーバレット。wave-1 と同型の cfg facade で、
// 上位は leaf を名指しせず `MobileBattery` だけを参照する。stub は query/subscribe とも
// `Err(Unimplemented)` を返す（実機実装で leaf 中身が差し替わっても facade 名は不変）。

#[cfg(target_os = "android")]
pub type MobileBattery = hayate_adapter_android::capability_stubs::AndroidBattery;
#[cfg(target_os = "ios")]
pub type MobileBattery = hayate_adapter_ios::capability_stubs::IosBattery;

// connectivity（ADR-0120）: battery と同型の cfg facade。stub は query/subscribe とも
// `Err(Unimplemented)` を返す（実機実装で leaf 中身が差し替わっても facade 名は不変）。
#[cfg(target_os = "android")]
pub type MobileConnectivity = hayate_adapter_android::capability_stubs::AndroidConnectivity;
#[cfg(target_os = "ios")]
pub type MobileConnectivity = hayate_adapter_ios::capability_stubs::IosConnectivity;

// geolocation（ADR-0120）: battery と同型の cfg facade。stub は query/subscribe とも
// `Err(Unimplemented)` を返す（権限ゲート付きだが scaffold では権限据え置き・ADR-0119/0120）。
#[cfg(target_os = "android")]
pub type MobileGeolocation = hayate_adapter_android::capability_stubs::AndroidGeolocation;
#[cfg(target_os = "ios")]
pub type MobileGeolocation = hayate_adapter_ios::capability_stubs::IosGeolocation;

// sensors（ADR-0120）: 単一 trait ＋ SensorKind 引数だが facade の形は battery と同型の cfg facade。
// stub は query/subscribe とも `Err(Unimplemented)` を返す（実機実装で leaf 中身が差し替わっても
// facade 名は不変）。
#[cfg(target_os = "android")]
pub type MobileSensors = hayate_adapter_android::capability_stubs::AndroidSensors;
#[cfg(target_os = "ios")]
pub type MobileSensors = hayate_adapter_ios::capability_stubs::IosSensors;
