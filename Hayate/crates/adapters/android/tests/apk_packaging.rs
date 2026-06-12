//! Host-side checks for `cargo-apk` packaging metadata (issue #195).
//!
//! On-device render verification still requires a local emulator; these tests
//! lock in the packaging contract that `cargo apk build` relies on.

use std::fs;
use std::path::PathBuf;

fn android_cargo_toml() -> String {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(manifest_dir.join("Cargo.toml")).expect("read Cargo.toml")
}

#[test]
fn cargo_toml_declares_cdylib_for_apk_packaging() {
    let contents = android_cargo_toml();
    assert!(
        contents.contains("crate-type = [\"cdylib\", \"rlib\"]"),
        "cargo-apk requires a cdylib artifact"
    );
}

#[test]
fn cargo_toml_declares_android_package_name() {
    let contents = android_cargo_toml();
    assert!(
        contents.contains("package = \"com.hayateprojects.hayate.adapter_android_demo\""),
        "missing [package.metadata.android] package id"
    );
}

#[test]
fn cargo_toml_pins_aarch64_apk_target() {
    let contents = android_cargo_toml();
    assert!(
        contents.contains("build_targets = [\"aarch64-linux-android\"]"),
        "stage A smoke test should ship arm64-v8a only"
    );
}

#[test]
fn cargo_toml_declares_vulkan_requirement_for_wgpu() {
    let contents = android_cargo_toml();
    assert!(
        contents.contains("name = \"android.hardware.vulkan.level\""),
        "wgpu uses Vulkan on Android; manifest must declare the feature"
    );
}

#[test]
fn stage_a_clear_color_is_dark_gray_blue() {
    // Visible on-device check: roughly RGB(26, 26, 31) on an 8-bit display.
    assert_eq!(hayate_adapter_android::STAGE_A_CLEAR_COLOR, [0.1, 0.1, 0.12, 1.0]);
}
