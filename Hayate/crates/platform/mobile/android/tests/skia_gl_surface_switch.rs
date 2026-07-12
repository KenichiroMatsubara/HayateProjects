//! skia GL（Ganesh/EGL）surface の Android 結線契約（issue #803、spec §4 REND-14/15、
//! ADR-0146 §3・ADR-0147）。
//!
//! #802 の `render_selection_switch.rs`（vello/skia 切替）と同じ「ソース走査で契約を固定する」
//! 方式——EGL/Ganesh・Gradle/AGP・skia-safe cross link はサンドボックス実行不可のため。純粋部
//! （`SkiaSurfaceKind` の解釈・既定値・グローバル格納）は `renderer_config.rs` の単体テストが
//! ホストで固定し、ここでは device 専用配線（Kotlin push・EGL 管理・fallback・観測ログ）の
//! 存在と所在（アダプタ封じ込め、REND-07）を固定する。

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

// ── intent extra 由来の実行時切替（再ビルド不要、#795/#802 と同じ操作感） ────────────────

#[test]
fn skia_surface_override_comes_from_the_intent_extra_pushed_over_jni() {
    let kt = main_activity();
    assert!(
        kt.contains("hayate.skia_surface"),
        "skia 内 raster/GL の切替口は intent extra（hayate.skia_surface）——\
         `adb shell am start -e hayate.renderer skia -e hayate.skia_surface gl` で\
         APK 再作成なしに切り替える"
    );

    let bridge = src("jni_bridge.rs");
    assert!(
        bridge.contains("store_pushed_skia_surface"),
        "JNI エクスポート（jni_bridge.rs へ封じ込め）は intent extra 由来の値を\
         renderer_config::store_pushed_skia_surface へ渡す"
    );
}

// ── GL 選択時の初期化と、失敗時の skia raster への一方向 fallback（boot は落とさない） ──

#[test]
fn skia_gl_is_attempted_when_selected_and_falls_back_to_raster_on_failure() {
    let app = src("app.rs");
    assert!(
        app.contains("effective_skia_surface"),
        "skia 選択時は intent extra 由来の実効 surface（renderer_config::effective_skia_surface）\
         で raster/GL を分岐する"
    );
    assert!(
        app.contains("skia_gl_window::init_skia_gl_surface"),
        "GL 選択時は EGL window surface + Ganesh を初期化する（skia_gl_window.rs、device 専用）"
    );
    assert!(
        app.contains("falling back to skia raster"),
        "EGL/GL 初期化失敗は理由をログに残して skia raster へ落ち、boot を落とさない\
         （issue #803 受け入れ条件）"
    );
}

// ── EGL/GPU 情報の観測（GL 選択時に logcat へ） ────────────────────────────────────────

#[test]
fn egl_and_gpu_info_are_logged_when_gl_is_selected() {
    let gl = src("skia_gl_window.rs");
    assert!(
        gl.contains("EGL vendor") && gl.contains("GL renderer"),
        "GL 選択時は EGL ベンダ・GL レンダラ文字列を logcat に出す（issue #803 受け入れ条件、\
         ADR-0146 §5 の観測語彙）"
    );
}

// ── present は eglSwapBuffers（Ganesh GL の OS handoff） ────────────────────────────────

#[test]
fn gl_present_swaps_the_egl_window_surface() {
    let gl = src("skia_gl_window.rs");
    assert!(
        gl.contains("eglSwapBuffers"),
        "GL 経路の present は EGLSurface（ANativeWindow 結線）の eglSwapBuffers"
    );
    assert!(
        gl.contains("wrap_backend_render_target"),
        "描画先は FBO0 を包む Ganesh backend render target（skia の GL surface）——\
         painter は渡された Canvas に描くだけ（ADR-0146 §3）"
    );
}

// ── EGL 管理はアダプタに閉じる（REND-07: core に第二の GPU 抽象を持ち込まない） ─────────

#[test]
fn egl_management_stays_inside_the_android_adapter() {
    // scene→Canvas 変換層（#800 の painter）に GL 対応のための変更が入っていないこと
    // （surface 非依存の実証、issue #803 受け入れ条件）。painter は skia_safe::gpu にも
    // EGL にも触れない——GL 化は「Canvas の出自が変わる」だけ。
    let painter_path = manifest_dir().join("../../../scene-renderers/skia/src/painter.rs");
    let painter = fs::read_to_string(&painter_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", painter_path.display()));
    assert!(
        !painter.contains("gpu") && !painter.to_ascii_lowercase().contains("egl"),
        "painter（scene→Canvas 変換層）は GL/EGL/gpu を一切知らない（ADR-0146 §3・REND-07）"
    );

    // core（hayate-core の render seam）にも EGL/GL 抽象を持ち込まない。
    let core_render = manifest_dir().join("../../../core/src/render/painter.rs");
    if let Ok(core_render) = fs::read_to_string(&core_render) {
        assert!(
            !core_render.to_ascii_lowercase().contains("egl"),
            "core の ScenePainter seam は EGL を知らない（REND-07）"
        );
    }
}
