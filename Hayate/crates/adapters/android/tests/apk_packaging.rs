//! Host-side checks for the Android packaging contract (ADR-0094, issue #195).
//!
//! The Rust sandbox cannot run Gradle/AGP, so these tests lock the GameActivity
//! + Gradle packaging contract by reading the source files; on-device build and
//! render verification still happen on a local emulator/device.

use std::fs;
use std::path::{Path, PathBuf};

fn read_relative(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn android_cargo_toml() -> String {
    read_relative("Cargo.toml")
}

const PACKAGE_ID: &str = "com.hayateprojects.hayate.adapter_android_demo";
/// cdylib name GameActivity loads via `android.app.lib_name` (no `lib`/`.so`).
const LIB_NAME: &str = "hayate_adapter_android";

#[test]
fn cargo_toml_declares_cdylib_for_the_apk() {
    let contents = android_cargo_toml();
    assert!(
        contents.contains("crate-type = [\"cdylib\", \"rlib\"]"),
        "GameActivity loads a cdylib; rust-android-gradle packages it into the APK"
    );
}

#[test]
fn cargo_toml_uses_the_game_activity_backend() {
    let contents = android_cargo_toml();
    assert!(
        contents.contains("features = [\"game-activity\"]"),
        "stage C IME needs GameActivity/GameTextInput, not native-activity (ADR-0094)"
    );
    assert!(
        !contents.contains("[package.metadata.android]"),
        "cargo-apk metadata must be gone — Gradle/Manifest are the single source of truth"
    );
}

#[test]
fn gradle_app_pins_package_id_and_arm64_abi() {
    let gradle = read_relative("android-app/app/build.gradle.kts");
    assert!(
        gradle.contains(&format!("applicationId = \"{PACKAGE_ID}\"")),
        "app module must declare the package id"
    );
    assert!(
        gradle.contains("abiFilters += \"arm64-v8a\""),
        "wgpu/Vulkan ships arm64-v8a only for now"
    );
    assert!(
        gradle.contains("androidx.games:games-activity"),
        "GameActivity AAR provides the GameTextInput IME path"
    );
}

#[test]
fn manifest_declares_vulkan_and_loads_the_cdylib() {
    let manifest = read_relative("android-app/app/src/main/AndroidManifest.xml");
    assert!(
        manifest.contains("android.hardware.vulkan.level"),
        "wgpu uses Vulkan on Android; manifest must declare the feature"
    );
    assert!(
        manifest.contains(&format!("android:value=\"{LIB_NAME}\"")),
        "android.app.lib_name must match the cdylib GameActivity loads"
    );
}

#[test]
fn main_activity_is_a_thin_game_activity_host() {
    let rel = format!(
        "android-app/app/src/main/kotlin/{}/MainActivity.kt",
        PACKAGE_ID.replace('.', "/")
    );
    let kotlin = read_relative(&rel);
    assert!(
        kotlin.contains("class MainActivity : GameActivity()"),
        "the Kotlin host must subclass GameActivity"
    );
    // Path also asserts the Kotlin package directory matches the package id.
    assert!(Path::new(env!("CARGO_MANIFEST_DIR")).join(&rel).exists());
}

#[test]
fn stage_a_clear_color_is_dark_gray_blue() {
    // Visible on-device check: roughly RGB(26, 26, 31) on an 8-bit display.
    assert_eq!(
        hayate_adapter_android::STAGE_A_CLEAR_COLOR,
        [0.1, 0.1, 0.12, 1.0]
    );
}
