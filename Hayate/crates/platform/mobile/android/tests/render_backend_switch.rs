//! 描画バックエンド / AA 方式のランタイム切替の契約（issue #795・ADR-0145）。
//!
//! Nothing Phone 3a（Adreno 710）で CSS Gallery ページのパス描画が破綻する切り分けのため、wgpu
//! バックエンド（Vulkan / GL）と vello の AA 方式（Area / MSAA8 / MSAA16）を **再ビルドなし**で
//! 切り替える。ADR-0138/0140 の「常時コンパイル＋ランタイムフラグ」流儀（cargo feature や別ビルドを
//! 作らない）。実行時上書きは intent extra（`adb shell am start -e hayate.backend gl -e hayate.aa
//! msaa8`）で、値の取得は Kotlin→Rust の JNI push。既定値（Area・Vulkan）は名前付き定数。
//!
//! Gradle/AGP・wgpu はサンドボックスで実行できないため、ソースを読んで契約を固定する
//! （`apk_packaging.rs` / `ios_packaging.rs` と同じ方式）。

use std::fs;
use std::path::PathBuf;

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn src(rel: &str) -> String {
    let path = manifest_dir().join("src").join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn main_activity() -> String {
    let path = manifest_dir().join(
        "android-app/app/src/main/kotlin/com/hayateprojects/hayate/adapter_android_demo/MainActivity.kt",
    );
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn web_vello_backend() -> String {
    // hayate-adapter-web の vello 経路（web が既定 Area のまま挙動不変であることを固定する対象）。
    let path = manifest_dir().join("../../web/src/backend/vello.rs");
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

// ── バックエンド選択（Vulkan / GL） ─────────────────────────────────────────

#[test]
fn wgpu_backend_is_selected_at_runtime_not_hardcoded() {
    let app = src("app.rs");
    assert!(
        app.contains("effective_backend") && app.contains("to_wgpu"),
        "wgpu インスタンスのバックエンドは render_config::effective_backend().to_wgpu() で\
         ランタイム選択する（Vulkan / GL）"
    );
    assert!(
        !app.contains("backends: wgpu::Backends::VULKAN"),
        "Backends::VULKAN 固定を廃し、ランタイム選択に置き換える（#795）"
    );
}

// ── AA 方式の注入（Area / MSAA8 / MSAA16） ──────────────────────────────────

#[test]
fn aa_method_is_injected_into_the_vello_renderer() {
    let app = src("app.rs");
    assert!(
        app.contains("new_with_options") && app.contains("effective_aa"),
        "VelloSceneRenderer を effective_aa() 由来の AA 方式で構築する（new_with_options）"
    );
}

#[test]
fn web_path_stays_default_area_unchanged() {
    let web = web_vello_backend();
    assert!(
        web.contains("VelloSceneRenderer::new("),
        "web 経路は VelloSceneRenderer::new（既定 Area）のまま——AA 方式は注入しない（挙動不変）"
    );
    assert!(
        !web.contains("new_with_options"),
        "web 経路に AA 方式注入を持ち込まない（#795: Android アダプタだけが実験用に切り替える）"
    );
}

// ── intent extra 由来の実行時上書き（再ビルド不要） ─────────────────────────

#[test]
fn runtime_override_comes_from_intent_extras_pushed_over_jni() {
    let kt = main_activity();
    assert!(
        kt.contains("hayate.backend") && kt.contains("hayate.aa"),
        "実行時上書きの口は intent extra（hayate.backend / hayate.aa）——APK 再作成なしで 3 実験を回す"
    );
    assert!(
        kt.contains("getStringExtra"),
        "intent extra は Activity の getStringExtra で読む"
    );
    assert!(
        kt.contains("external fun") && kt.contains("nativePushRenderConfig"),
        "読んだ上書き値は JNI native 関数（nativePushRenderConfig）で Rust へ push する"
    );

    let bridge = src("jni_bridge.rs");
    assert!(
        bridge.contains("nativePushRenderConfig") && bridge.contains("store_pushed_config"),
        "JNI エクスポートは jni_bridge.rs に置き（封じ込め）、render_config::store_pushed_config へ渡す"
    );
}

// ── logcat 記録（実験記録・上流報告） ───────────────────────────────────────

#[test]
fn selection_and_gpu_adapter_info_are_logged() {
    let app = src("app.rs");
    assert!(
        app.contains("get_info"),
        "選択された GPU アダプタ情報（名前・ドライバ）を adapter.get_info() で取得して logcat に出す\
         （wgpu/naga への上流報告にそのまま使う）"
    );
    assert!(
        app.contains("log::info!") || app.contains("log::warn!"),
        "選択された AA 方式・バックエンド・アダプタ情報を logcat に出す"
    );
}

// ── GL 取得失敗でも boot を落とさない ───────────────────────────────────────

#[test]
fn gl_adapter_failure_does_not_crash_boot() {
    let app = src("app.rs");
    // init_gpu_surface は Result を返し、CreateSurface ハンドラは Err を logcat に出して続行する
    // （surface 無しで boot は生き、失敗理由がログに残る）。GL で adapter/device 取得に失敗する
    // 端末でも同じ経路で落ちない。
    assert!(
        app.contains("GPU init failed"),
        "GPU 初期化失敗（GL で adapter/device 取得不可を含む）は boot を落とさず logcat に理由を残す"
    );
    assert!(
        app.contains("no compatible wgpu adapter") || app.contains("request_device"),
        "adapter/device 取得の失敗理由を Err メッセージに含める（ログに残す）"
    );
}
