//! Miharashi Android ホストの「端末入力 dev-server URL が fetch と reload の両方を駆動する」配線契約（#534）。
//!
//! パース・既定・入力読み戻しの純ロジック（`dev_server_target`）はホスト単体テストで緑
//! （src/dev_server_target.rs の `#[cfg(test)]`）。一方 `app_tsubame` は device 専用でホストには
//! コンパイルされない（ADR-0112）。そこで apk_packaging.rs / reload_wiring.rs と同じく、ソースを読んで
//! 「端末が入れた URL を 1 つの target に解決し、その host:port でバンドル fetch（HTTP）と reload 購読
//! （WS）の両方を張る」配線が据わっていることを固定する（保持・再接続）。実 UI と実機 fetch/boot は
//! ローカル実機で検証する（本 issue 外）。QR は対象外（将来）。

use std::fs;
use std::path::PathBuf;

fn read_relative(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn app_tsubame_src() -> String {
    read_relative("src/app_tsubame.rs")
}

#[test]
fn host_resolves_the_dev_server_from_the_url_the_device_ui_entered() {
    let src = app_tsubame_src();
    // 端末 UI が internal data dir に書いた URL を読み戻して target に解決する単一の入口。
    assert!(
        src.contains("dev_server_target::resolve_entered"),
        "the host must resolve the dev-server target from the entered URL (#534)"
    );
    // 入力の在り処は `AndroidApp::internal_data_path`（Kotlin の EditText が書く dir）。
    assert!(
        src.contains("internal_data_path"),
        "the entered URL is read from the app internal data path (where the Kotlin UI writes it)"
    );
}

#[test]
fn the_resolved_target_drives_the_bundle_fetch() {
    let src = app_tsubame_src();
    // バンドル fetch は target 駆動（ハードコード定数源 fetch_dev_bundle は廃止）。
    assert!(
        src.contains("bundle_source::fetch_from"),
        "the bundle fetch must be driven by the resolved target, not a hardcoded source (#534)"
    );
    assert!(
        !src.contains("fetch_dev_bundle"),
        "the hardcoded-default fetch entry is replaced by the target-driven fetch_from (#534)"
    );
}

#[test]
fn the_same_target_drives_the_reload_subscription() {
    let src = app_tsubame_src();
    // reload 購読も同じ解決済み target で駆動する（保持：fetch と reload が同じ配信点を指す。
    // #742 で host/port の切り出しをやめ、target 丸ごとを共有——https 由来なら wss も含めて
    // scheme が purely に写る）。ハードコード定数（bundle_source::DEV_SERVER_HOST/PORT）は
    // もう参照しない。
    assert!(
        src.contains("target: target.clone()"),
        "the reload subscription must share the same resolved target as the fetch (#534/#742)"
    );
    assert!(
        !src.contains("bundle_source::DEV_SERVER_HOST"),
        "the WS reload must no longer hardcode the dev-server host (#534)"
    );
}

#[test]
fn the_url_filename_is_a_shared_wire_contract_between_kotlin_and_native() {
    // Kotlin（writer）と Rust（reader）が同じファイル名を使う wire 契約。ここがズレると入力が届かない。
    let target_src = read_relative("src/dev_server_target.rs");
    let kotlin = setup_activity_src();
    assert!(
        target_src.contains("miharashi-dev-server-url.txt"),
        "native reads the entered URL from a named file under the internal data dir (#534)"
    );
    assert!(
        kotlin.contains("miharashi-dev-server-url.txt"),
        "the Kotlin UI must write to the same file the native host reads (shared wire contract, #534)"
    );
}

fn setup_activity_src() -> String {
    read_relative(
        "android-app/app/src/main/kotlin/com/hayateprojects/hayate/adapter_android_demo/DevServerSetupActivity.kt",
    )
}

#[test]
fn a_minimal_device_ui_captures_the_url_and_persists_it() {
    let kotlin = setup_activity_src();
    // 端末上の最小 UI：EditText で URL を入力し、internal files dir へ書く（ネイティブが読み戻す）。
    assert!(
        kotlin.contains("EditText"),
        "the device UI must offer a text field to enter the dev-server URL (#534)"
    );
    assert!(
        kotlin.contains("filesDir") && kotlin.contains("writeText"),
        "the entered URL must be persisted under the app internal files dir (#534)"
    );
    // 前回値を field に戻すことで保持を可視化する。
    assert!(
        kotlin.contains("readText"),
        "the UI shows the previously entered URL again on next launch (retention, #534)"
    );
    // QR は対象外（将来）と明記する。
    assert!(
        kotlin.contains("QR"),
        "the device UI must document QR scanning as out of scope / future (#534)"
    );
}

#[test]
fn the_url_entry_screen_is_the_launcher_not_the_game_activity() {
    let manifest = read_relative("android-app/app/src/main/AndroidManifest.xml");
    assert!(
        manifest.contains(".DevServerSetupActivity") && manifest.contains("LAUNCHER"),
        "the URL-entry screen must be declared as the launcher activity (#534)"
    );
    // MainActivity（GameActivity）は入力後に明示 Intent で起動されるので、もう LAUNCHER ではない。
    let after_main = manifest.split(".MainActivity").nth(1).unwrap_or("");
    assert!(
        !after_main.contains("LAUNCHER"),
        "MainActivity must no longer be the launcher — the URL-entry screen is (#534)"
    );
}
