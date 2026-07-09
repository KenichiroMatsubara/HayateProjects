//! Android パッケージング契約のホスト側検証（ADR-0094）。
//!
//! Rust サンドボックスでは Gradle/AGP を実行できないため、ソースファイルを読んで
//! GameActivity + Gradle のパッケージング契約を固定する。実機ビルドと描画確認は
//! ローカルのエミュレータ/実機で行う。

use std::fs;
use std::path::{Path, PathBuf};

fn read_relative(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn android_cargo_toml() -> String {
    read_relative("Cargo.toml")
}

/// Play 公開パッケージ名（AGP の applicationId）。永久固定。code パッケージとは別物（AGP は
/// applicationId と namespace を別に扱える）。
const PACKAGE_ID: &str = "com.hayateprojects.torimi";
/// Kotlin code パッケージ（= namespace）。ソースのディレクトリ構成と JNI クラス名がこれ由来。
/// applicationId（[`PACKAGE_ID`]）を製品名に変えても code パッケージは据え置くので別定数にする。
const CODE_PACKAGE: &str = "com.hayateprojects.hayate.adapter_android_demo";
/// GameActivity が `android.app.lib_name` で読み込む cdylib 名（`lib`/`.so` なし）。
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
        CODE_PACKAGE.replace('.', "/")
    );
    let kotlin = read_relative(&rel);
    assert!(
        kotlin.contains("class MainActivity : GameActivity()"),
        "the Kotlin host must subclass GameActivity"
    );
    // パスの存在確認は Kotlin パッケージディレクトリが code パッケージ（namespace）と一致することも保証する。
    assert!(Path::new(env!("CARGO_MANIFEST_DIR")).join(&rel).exists());
}

#[test]
fn stage_a_clear_color_is_dark_gray_blue() {
    // 実機での目視確認用: 8bit 表示でおよそ RGB(26, 26, 31)。
    assert_eq!(
        hayate_adapter_android::STAGE_A_CLEAR_COLOR,
        [0.1, 0.1, 0.12, 1.0]
    );
}
