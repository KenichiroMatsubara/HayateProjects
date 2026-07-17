//! 安全領域インセットの契約（edge-to-edge / b2, issue #794・ADR-0144）。
//!
//! **b2 方式**: GameActivity の SurfaceView はフルウィンドウ（edge-to-edge）のまま、GPU
//! surface/swapchain もフルウィンドウ。WindowInsets（systemBars + displayCutout。IME は含めない
//! — GameTextInput が別途処理する）を JNI で Rust へ push し、レイアウトビューポート縮小・シーン
//! 平行移動・バー裏のルート背景色クリア・タッチ座標補正を **アダプタ内（Rust）で完結**する。
//!
//! 旧「マージン方式」（WindowInsets を SurfaceView の `setMargins` にして ANativeWindow 自体を
//! 縮め、Kotlin 側で MotionEvent を平行移動）は Nothing Phone 3a（Android 15 世代）でリスナーが
//! 端末依存で不発になりステータスバー侵食を起こしたため撤去した（ADR-0144）。`AndroidApp::
//! content_rect()` はフルウィンドウを返す端末があるため、Rust 側 content_rect フォールバックだけ
//! でも補正できず、JNI push を一次ソースにする必要がある。
//!
//! Gradle/AGP・wgpu はサンドボックスで実行できないため、ソースを読んで契約を固定する
//! （`apk_packaging.rs` と同じ方式）。

use std::fs;
use std::path::PathBuf;

fn main_activity() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "android-app/app/src/main/kotlin/com/hayateprojects/hayate/adapter_android_demo/MainActivity.kt",
    );
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn src(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

// ── Kotlin (MainActivity) の b2 契約 ────────────────────────────────────────

#[test]
fn surface_view_stays_edge_to_edge_without_margins_or_kotlin_touch_correction() {
    let kt = main_activity();
    assert!(
        !kt.contains("setMargins"),
        "b2: SurfaceView はフルウィンドウのまま。マージン方式（ANativeWindow を縮める）は\
         端末依存で不発になりステータスバー侵食を起こしたため撤去する（ADR-0144）"
    );
    assert!(
        !kt.contains("offsetLocation"),
        "b2: Kotlin 側のタッチ平行移動（MotionEvent の SurfaceView 相対補正）は撤去し、\
         タッチ座標補正を Rust 側（safe_area::correct_touch）へ一本化する"
    );
    assert!(
        kt.contains("setDecorFitsSystemWindows") || kt.contains("enableEdgeToEdge"),
        "b2: SurfaceView をフルウィンドウに広げる edge-to-edge を明示的に有効化する"
    );
}

#[test]
fn insets_are_pushed_to_rust_over_jni_per_listener_and_as_a_snapshot() {
    let kt = main_activity();
    assert!(
        kt.contains("setOnApplyWindowInsetsListener"),
        "WindowInsets はリスナー発火ごとに取得する"
    );
    assert!(
        kt.contains("systemBars()") && kt.contains("displayCutout()"),
        "安全領域は systemBars + displayCutout（IME は GameTextInput が別途処理するので含めない）"
    );
    assert!(
        !kt.contains("Type.ime()"),
        "IME インセットは含めない（GameTextInput が別途処理する）"
    );
    assert!(
        kt.contains("external fun") && kt.contains("nativePushSafeAreaInsets"),
        "取得したインセットは JNI native 関数（nativePushSafeAreaInsets）で Rust へ push する"
    );
    assert!(
        kt.contains("rootWindowInsets"),
        "リスナー不発端末への保険として onCreate 後に rootWindowInsets スナップショットも一度 push する"
    );
}

#[test]
fn the_insets_listener_does_not_consume_the_insets() {
    let kt = main_activity();
    assert!(
        !kt.contains("CONSUMED"),
        "インセットを消費すると GameActivity 自身の SurfaceView リスナー（GameTextInput の \
         IME インセット処理）に届かなくなる — 下流へ流すこと"
    );
}

#[test]
fn received_insets_are_logged_for_per_device_diagnosis() {
    let kt = main_activity();
    assert!(
        kt.contains("Log.") || kt.contains("android.util.Log"),
        "受信したインセット値は logcat に記録し、端末別のインセット配送問題を診断可能にする"
    );
}

#[test]
fn status_bar_icon_appearance_is_set_from_a_named_constant() {
    let kt = main_activity();
    assert!(
        kt.contains("isAppearanceLightStatusBars"),
        "ステータスバーのアイコン色は isAppearanceLightStatusBars で静的に設定する"
    );
    assert!(
        kt.contains("LIGHT_STATUS_BAR_ICONS") || kt.contains("const val"),
        "isAppearanceLightStatusBars の値は名前付き定数（マジック真偽値の禁止）"
    );
}

// ── Rust (JNI push の着地点 + アダプタ内完結) の b2 契約 ─────────────────────

#[test]
fn jni_bridge_exposes_the_native_inset_push_entry_point() {
    let bridge = src("jni_bridge.rs");
    assert!(
        bridge.contains("nativePushSafeAreaInsets"),
        "Kotlin→Rust の JNI エクスポート（Java_..._nativePushSafeAreaInsets）は JNI 封じ込め方針に\
         従い jni_bridge.rs に置く（jni:: を直接使える唯一のファイル）"
    );
    assert!(
        bridge.contains("store_pushed_insets"),
        "push された値は safe_area::store_pushed_insets でフレームループ可読なグローバルに格納する"
    );
}

#[test]
fn app_completes_safe_area_handling_inside_the_adapter() {
    let app = src("app.rs");
    assert!(
        app.contains("render_scene_with_offset"),
        "b2: シーンを安全領域インセット分だけ平行移動して描画する（バー裏はルート背景色でクリア）"
    );
    assert!(
        app.contains("scene_origin"),
        "描画の平行移動原点は safe_area::SafeAreaInsets::scene_origin から導く"
    );
    assert!(
        app.contains("correct_touch"),
        "タッチ座標補正は Rust 側（safe_area::correct_touch）で行う（Kotlin から一本化）"
    );
    assert!(
        app.contains("pushed_insets"),
        "JNI push 値を一次ソースにし、content_rect 由来はフォールバックへ降格する"
    );
}
