//! ホスト可読の Family Adapter facade 契約（ADR-0117）。
//!
//! Family Adapter はビルド時 `cfg(target_os)` で片方の leaf をリンクする facade であって、
//! ランタイム dispatch ではない。実機 SDK 無しに leaf を実体化できないため、ソース走査で
//! 「facade が cfg で正しい leaf を選ぶ」「契約の正本は Core のまま」「ランタイム分岐を
//! 持ち込まない」を固定する（`*_packaging` / `*_encapsulation` パターン準拠）。

use std::fs;
use std::path::PathBuf;

fn read_relative(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn lib_rs() -> String {
    read_relative("src/lib.rs")
}

#[test]
fn facade_selects_the_android_leaf_under_cfg() {
    let lib = lib_rs();
    assert!(
        lib.contains("#[cfg(target_os = \"android\")]")
            && lib.contains("hayate_adapter_android::audio_output::AudioTrackOutput"),
        "on android the facade must resolve MobileAudioOutput to the AudioTrack leaf impl"
    );
}

#[test]
fn facade_selects_the_ios_leaf_under_cfg() {
    let lib = lib_rs();
    assert!(
        lib.contains("#[cfg(target_os = \"ios\")]")
            && lib.contains("hayate_adapter_ios::audio_output::AvAudioEngineOutput"),
        "on ios the facade must resolve MobileAudioOutput to the AVAudioEngine leaf impl"
    );
}

#[test]
fn facade_exposes_a_single_unified_capability_name() {
    let lib = lib_rs();
    // 両 cfg 分岐とも同一の型名へ解決し、上位は leaf を名指ししない。
    assert_eq!(
        lib.matches("pub type MobileAudioOutput").count(),
        2,
        "both targets must expose the same unified facade name `MobileAudioOutput`"
    );
}

#[test]
fn facade_reexports_the_core_contract_as_the_single_source_of_truth() {
    let lib = lib_rs();
    assert!(
        lib.contains("pub use hayate_core::") && lib.contains("AudioOutput"),
        "the capability contract is canonical in Core; the facade re-exports it, not redefines it"
    );
    assert!(
        !lib.contains("trait AudioOutput"),
        "the facade must not define its own AudioOutput trait (contract lives in Core)"
    );
}

#[test]
fn facade_is_build_time_cfg_not_runtime_dispatch() {
    let lib = lib_rs();
    // ランタイム分岐の徴候を禁止: trait object による動的 dispatch も、実行時 OS 判定も持たない。
    assert!(
        !lib.contains("dyn AudioOutput"),
        "build-time cfg selection must not degrade into a `dyn AudioOutput` runtime dispatch"
    );
    assert!(
        !lib.contains("consts::OS") && !lib.contains("cfg!("),
        "the leaf must be chosen at build time via #[cfg], not by a runtime OS check"
    );
}

#[test]
fn manifest_gates_each_leaf_dependency_by_target() {
    let manifest = read_relative("Cargo.toml");
    assert!(
        manifest.contains("[target.'cfg(target_os = \"android\")'.dependencies]")
            && manifest.contains("hayate-adapter-android"),
        "the android leaf must be a target-gated dependency (linked only on android)"
    );
    assert!(
        manifest.contains("[target.'cfg(target_os = \"ios\")'.dependencies]")
            && manifest.contains("hayate-adapter-ios"),
        "the ios leaf must be a target-gated dependency (linked only on ios)"
    );
}
