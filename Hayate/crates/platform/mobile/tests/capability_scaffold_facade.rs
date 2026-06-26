//! wave-1 capability scaffold の facade 契約（ADR-0119）。
//!
//! `audio_facade_selection.rs` と同じソース走査で、各 capability の facade が「両 cfg 分岐とも
//! 同一の統一型名へ解決する」「契約の正本は Core のまま（facade は再露出のみ）」「ランタイム
//! dispatch を持ち込まない」を固定する。実機 SDK 無しに leaf を実体化できないため走査で pin する。

use std::fs;
use std::path::PathBuf;

fn lib_rs() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/lib.rs");
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// wave-1 の facade 型名と、対応する android/ios の leaf stub 型名。
const FACADES: &[(&str, &str, &str)] = &[
    ("MobileHaptics", "AndroidHaptics", "IosHaptics"),
    (
        "MobileLocalNotifications",
        "AndroidLocalNotifications",
        "IosLocalNotifications",
    ),
    ("MobileUrlLauncher", "AndroidUrlLauncher", "IosUrlLauncher"),
    (
        "MobileSecureStorage",
        "AndroidSecureStorage",
        "IosSecureStorage",
    ),
    ("MobileDeviceInfo", "AndroidDeviceInfo", "IosDeviceInfo"),
    ("MobileShare", "AndroidShare", "IosShare"),
    ("MobileFilePicker", "AndroidFilePicker", "IosFilePicker"),
    (
        "MobileKeyValueStore",
        "AndroidKeyValueStore",
        "IosKeyValueStore",
    ),
    ("MobileBiometric", "AndroidBiometric", "IosBiometric"),
    // wave-2 ストリーム capability 土台（ADR-0120）: battery も wave-1 と同型の cfg facade。
    ("MobileBattery", "AndroidBattery", "IosBattery"),
    // wave-2 connectivity（ADR-0120）: battery の契約土台を再利用した同型 cfg facade。
    (
        "MobileConnectivity",
        "AndroidConnectivity",
        "IosConnectivity",
    ),
];

#[test]
fn each_facade_exposes_one_unified_name_resolving_to_both_leaves() {
    let lib = lib_rs();
    for (facade, android_leaf, ios_leaf) in FACADES {
        // 両 cfg 分岐とも同一の統一型名（上位は leaf を名指ししない）。
        assert_eq!(
            lib.matches(&format!("pub type {facade}")).count(),
            2,
            "{facade} は android/ios 両ターゲットで同名の facade を露出しなければならない"
        );
        assert!(
            lib.contains(&format!(
                "hayate_adapter_android::capability_stubs::{android_leaf}"
            )),
            "{facade} は android で {android_leaf} leaf へ解決しなければならない"
        );
        assert!(
            lib.contains(&format!("hayate_adapter_ios::capability_stubs::{ios_leaf}")),
            "{facade} は ios で {ios_leaf} leaf へ解決しなければならない"
        );
    }
}

#[test]
fn facade_reexports_core_contracts_without_redefining_them() {
    let lib = lib_rs();
    // 契約・型は Core が正本。facade は再露出のみで trait を切らない。
    assert!(
        lib.contains("pub use hayate_core::"),
        "facade は capability 契約を Core から再露出する"
    );
    for contract in [
        "Haptics",
        "LocalNotifications",
        "UrlLauncher",
        "SecureStorage",
        "DeviceInfoProvider",
        "Share",
        "FilePicker",
        "KeyValueStore",
        "Biometric",
        "CapabilityError",
        "Battery",
        "ConnectivityProvider",
    ] {
        assert!(
            !lib.contains(&format!("trait {contract}")),
            "facade は {contract} を再定義してはならない（契約は Core 所有）"
        );
    }
}

#[test]
fn facade_is_build_time_cfg_not_runtime_dispatch() {
    let lib = lib_rs();
    assert!(
        !lib.contains("dyn Haptics") && !lib.contains("dyn FilePicker"),
        "ビルド時 cfg 選択を dyn trait のランタイム dispatch に堕とさない"
    );
    assert!(
        !lib.contains("cfg!(") && !lib.contains("consts::OS"),
        "leaf は #[cfg] のビルド時選択で、実行時 OS 判定で選ばない"
    );
}

#[test]
fn clipboard_is_not_re_scaffolded_as_a_capability() {
    let lib = lib_rs();
    // clipboard は ADR-0097 の編集境界が所有する。capability facade に同名を作らない（ADR-0119）。
    assert!(
        !lib.contains("MobileClipboard"),
        "clipboard は capability に含めない（ADR-0097 の編集境界が所有）"
    );
}
