//! iOS パッケージング契約のホスト側検証（ADR-0114）。
//!
//! Rust サンドボックス（および非 Mac CI）では Xcode/xcodebuild を実行できないため、
//! ソースファイルを読んで Xcode + cargo staticlib のパッケージング契約を固定する。
//! 実機ビルドと描画確認はローカルの Mac/シミュレータ/実機で行う（ADR-0087/0094 と
//! 同じ検証ギャップ）。`hayate-adapter-android` の `apk_packaging.rs` の鏡写し。

use std::fs;
use std::path::{Path, PathBuf};

fn read_relative(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn ios_cargo_toml() -> String {
    read_relative("Cargo.toml")
}

const BUNDLE_ID: &str = "com.hayateprojects.hayate.adapter_ios_demo";
/// Xcode がリンクする staticlib 名（`lib` 接頭辞 + `.a`）。
const STATIC_LIB: &str = "libhayate_adapter_ios.a";

#[test]
fn cargo_toml_declares_staticlib_for_the_app() {
    let contents = ios_cargo_toml();
    assert!(
        contents.contains("crate-type = [\"staticlib\", \"rlib\"]"),
        "Xcode links a staticlib into the app binary; rlib keeps the host-testable seams runnable"
    );
}

#[test]
fn swift_host_owns_uikit_and_the_metal_layer() {
    // ADR-0113 shape 1: UIKit / UITextInput は薄い Swift ホストが束ね、Rust は ObjC-free。
    // よって CAMetalLayer の所有とテキスト入力は HayateView.swift 側にある。
    let view = read_relative("ios-app/Hayate/HayateView.swift");
    assert!(
        view.contains("CAMetalLayer"),
        "the Swift host must own the CAMetalLayer (wgpu's Metal surface)"
    );
    assert!(
        view.contains("UIKeyInput") || view.contains("UITextInput"),
        "the Swift host must own keyboard/text input (UITextInput conformance)"
    );
}

#[test]
fn rust_glue_targets_metal_via_core_animation_layer() {
    // Rust は Swift から渡された CAMetalLayer ポインタで wgpu Metal サーフェスを張る。
    let app = read_relative("src/app.rs");
    assert!(
        app.contains("Backends::METAL"),
        "the iOS glue must select the wgpu Metal backend"
    );
    assert!(
        app.contains("CoreAnimationLayer"),
        "the surface is built from the CAMetalLayer via SurfaceTargetUnsafe::CoreAnimationLayer"
    );
}

#[test]
fn info_plist_pins_bundle_id() {
    let plist = read_relative("ios-app/Hayate/Info.plist");
    assert!(
        plist.contains(BUNDLE_ID),
        "Info.plist must declare the bundle id (single source of truth, ADR-0114)"
    );
}

#[test]
fn info_plist_requires_metal() {
    let plist = read_relative("ios-app/Hayate/Info.plist");
    assert!(
        plist.contains("UIRequiredDeviceCapabilities") && plist.contains("metal"),
        "wgpu renders via Metal on iOS; Info.plist must require the metal capability \
         (analogue of Android's android.hardware.vulkan.level)"
    );
}

#[test]
fn info_plist_declares_a_scene_manifest() {
    let plist = read_relative("ios-app/Hayate/Info.plist");
    assert!(
        plist.contains("UIApplicationSceneManifest"),
        "the surface_lifecycle state machine is driven by UIScene lifecycle; \
         Info.plist must declare the scene manifest"
    );
}

#[test]
fn xcode_project_links_the_static_lib() {
    let pbxproj = read_relative("ios-app/Hayate.xcodeproj/project.pbxproj");
    assert!(
        pbxproj.contains(STATIC_LIB),
        "the Xcode project must link the cargo staticlib (analogue of Android's \
         android.app.lib_name ↔ cdylib name)"
    );
}

#[test]
fn app_and_scene_delegates_are_thin_hosts() {
    let app_delegate = read_relative("ios-app/Hayate/AppDelegate.swift");
    assert!(
        app_delegate.contains("UIApplicationDelegate"),
        "AppDelegate must be a thin UIApplicationDelegate host"
    );

    let scene_delegate = read_relative("ios-app/Hayate/SceneDelegate.swift");
    // 薄ホストは Rust 入口 / Hayate view を参照するだけで、アプリロジックは持たない
    // （Android の `MainActivity : GameActivity()` 相当）。
    assert!(
        scene_delegate.contains("HayateView") || scene_delegate.contains("ios_main"),
        "SceneDelegate must wire up the Hayate view / Rust entry point (logic stays in Rust)"
    );
    // パスの存在確認は薄ホスト一式が揃っていることも保証する。
    assert!(Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("ios-app/Hayate/HayateView.swift")
        .exists());
}

#[test]
fn stage_a_clear_color_is_dark_gray_blue() {
    // 実機での目視確認用: 8bit 表示でおよそ RGB(26, 26, 31)。Android と同値。
    assert_eq!(
        hayate_adapter_ios::STAGE_A_CLEAR_COLOR,
        [0.1, 0.1, 0.12, 1.0]
    );
}
