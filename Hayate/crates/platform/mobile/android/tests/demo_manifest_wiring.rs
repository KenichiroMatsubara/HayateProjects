//! Miharashi Android ホストの Demo Manifest 駆動（初回自動ロード＋デモ選択メニュー）の device 配線契約（#743）。
//!
//! パース・エントリ選択 → boot target 解決・取得失敗の明示エラーの純ロジック（`demo_manifest`）と、
//! release/debug 既定の分離（`dev_server_target`）はホスト単体テストで緑（src/*.rs の `#[cfg(test)]`）。
//! 一方 `app_tsubame`（実 fetch/boot）と Kotlin UI は device 専用でホストにはコンパイルされない
//! （ADR-0112 / ADR-0001）。そこで reload_wiring.rs / dev_server_target_wiring.rs と同じく、ソースを読んで
//! 「初回起動が manifest 先頭デモを自動ロードし、取得失敗を明示エラー＋ URL 入力誘導にし、Kotlin の
//! デモ選択メニューが表示名で並ぶ」配線が据わっていることを固定する。実 UI と実機 fetch/boot はローカル
//! 実機で検証する（本 issue 外）。

use std::fs;
use std::path::PathBuf;

fn read_relative(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn app_tsubame_src() -> String {
    read_relative("src/app_tsubame.rs")
}

fn setup_activity_src() -> String {
    read_relative(
        "android-app/app/src/main/kotlin/com/hayateprojects/hayate/adapter_android_demo/DevServerSetupActivity.kt",
    )
}

#[test]
fn first_launch_auto_loads_the_first_demo_from_the_manifest() {
    let src = app_tsubame_src();
    // 接続先未設定の初回起動（release）は plan_boot の ManifestAutoload 経路に入り、Demo Endpoint の
    // manifest 先頭デモを OS スタック fetch で解決して boot する（ゼロ入力で動く・ADR-0003）。
    assert!(
        src.contains("demo_manifest::plan_boot"),
        "the host must route boot through demo_manifest::plan_boot (#743)"
    );
    assert!(
        src.contains("ManifestAutoload") && src.contains("first_boot_target_fetched"),
        "the manifest-autoload path must fetch the manifest and resolve the first demo (#743)"
    );
}

#[test]
fn release_and_debug_defaults_are_told_apart_by_the_build_profile() {
    let src = app_tsubame_src();
    // release ビルドのみ manifest 自動ロードに入る。debug（エミュレータ loopback）は従来どおり
    // 単一バンドル直 boot（既存経路不変・#534）。分岐はビルドプロファイル（debug_assertions）。
    assert!(
        src.contains("!cfg!(debug_assertions)"),
        "release vs debug default must be told apart by the build profile (#743)"
    );
}

#[test]
fn a_manifest_fetch_failure_is_an_explicit_error_not_a_crash() {
    let src = app_tsubame_src();
    // manifest 取得/解釈の失敗は明示エラー（error_overlay）にして pump に進めない（謎クラッシュ回避）。
    // 誘導文言（「URL 入力…」）は DemoManifestError::message が持ち、host はそれをそのまま出す。
    assert!(
        src.contains("autoload_error") && src.contains("error_overlay::show_error"),
        "a manifest fetch/parse failure must surface as an explicit overlay error (#743)"
    );
    // 明示エラー＋ URL 入力誘導の文言は純ロジック側の契約テストで固定済み（src/demo_manifest.rs）。
    let manifest_src = read_relative("src/demo_manifest.rs");
    assert!(
        manifest_src.contains("URL 入力"),
        "the explicit error must guide the user to the URL-entry path (#743)"
    );
}

#[test]
fn the_manifest_fetch_reuses_the_os_network_stack() {
    // 手書き HTTP を再導入せず、#740 の OS スタック委譲（Kotlin BundleFetchBridge）を再利用する。
    // バンドル取得とマニフェスト取得は同じ入口（bundle_source::fetch_url）を共有する。
    let bundle_src = read_relative("src/bundle_source.rs");
    assert!(
        bundle_src.contains("pub fn fetch_url"),
        "bundle_source must expose a shared OS-stack GET entry (#740/#743)"
    );
    let manifest_src = read_relative("src/demo_manifest.rs");
    assert!(
        manifest_src.contains("bundle_source::fetch_url"),
        "the manifest fetch must reuse the OS-stack fetch, not hand-rolled HTTP (#743)"
    );
}

#[test]
fn the_launcher_shows_a_demo_selection_menu_by_display_name() {
    let kotlin = setup_activity_src();
    // Demo Manifest（/demos.json）を取得して表示名でデモ選択メニューを並べる（#743）。
    assert!(
        kotlin.contains("DEMO_MANIFEST_ROUTE") && kotlin.contains("demos"),
        "the launcher must fetch the Demo Manifest and list its demos (#743)"
    );
    // 選択＝そのバンドル URL を保存してネイティブ描画を起動する（Direct boot に合流）。
    assert!(
        kotlin.contains("bundleUrl") && kotlin.contains("MainActivity"),
        "selecting a demo must persist its bundle URL and launch the native host (#743)"
    );
    // 取得はメインスレッド外（OS スタック・ADR-0002）。NetworkOnMainThreadException を避ける。
    assert!(
        kotlin.contains("Thread") && kotlin.contains("HttpURLConnection"),
        "the manifest fetch must run off the UI thread over the OS network stack (#743)"
    );
}

#[test]
fn the_demo_endpoint_and_manifest_route_are_shared_wire_between_rust_and_kotlin() {
    // ルート `/demos.json` と Demo Endpoint URL は Rust（`demo_manifest` / `dev_server_target`）と
    // Kotlin（launcher）が同値で共有する wire 契約。ズレると取得先が食い違う。
    let manifest_src = read_relative("src/demo_manifest.rs");
    let target_src = read_relative("src/dev_server_target.rs");
    let kotlin = setup_activity_src();
    assert!(
        manifest_src.contains("\"/demos.json\"") && kotlin.contains("\"/demos.json\""),
        "the manifest route must match between the Rust seam and the Kotlin launcher (#743)"
    );
    assert!(
        target_src.contains("miharashi-demo-endpoint.workers.dev")
            && kotlin.contains("miharashi-demo-endpoint.workers.dev"),
        "the Demo Endpoint URL constant must match between Rust and Kotlin (#743)"
    );
}
