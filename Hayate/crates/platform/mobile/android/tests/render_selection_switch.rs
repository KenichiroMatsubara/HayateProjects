//! レンダラ（vello/skia）選択の Android 結線契約（issue #802、spec §4 REND-14/15、
//! ADR-0146/0147）。
//!
//! #801 が確立した `hayate_app_host::renderer_selection::native_renderer_selection_policy`
//! （vello → skia の一方向 fallback、forced-override の却下、vello 不在時の skia 単独起動）を
//! Android アダプタでも再利用し、intent extra（`hayate.renderer`）で再ビルドなしに切り替えられる
//! ことを固定する。#795 の `render_backend_switch.rs`（backend/AA スイッチ）と同じ「ソース走査で
//! 契約を固定する」方式——Gradle/AGP・skia-safe cross link・ndk はサンドボックス実行不可のため。

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

fn dev_server_setup_activity() -> String {
    let path = manifest_dir().join(
        "android-app/app/src/main/kotlin/com/hayateprojects/hayate/adapter_android_demo/DevServerSetupActivity.kt",
    );
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

// ── DevServerSetupActivity（唯一の exported LAUNCHER）が intent extra を転送すること ──
//
// MainActivity は android:exported="false" なので `adb shell am start -n .../.MainActivity`
// は権限エラーになる（実機確認済み）。ランタイム切替の intent extra は必ず
// DevServerSetupActivity 経由で MainActivity へ届く——転送しないと adb からの切替キーは
// 常に未指定として観測され、「APK 作り直しなしに切り替えられる」受け入れ条件が満たせない。
#[test]
fn dev_server_setup_activity_forwards_intent_extras_to_main_activity() {
    let kt = dev_server_setup_activity();
    assert!(
        kt.contains("intent.extras") && kt.contains("putExtras"),
        "DevServerSetupActivity must forward its own intent extras onto the MainActivity intent \
         it launches, so `adb shell am start -n .../.DevServerSetupActivity -e hayate.renderer skia` \
         (and the pre-existing #795 hayate.backend/hayate.aa) actually reach MainActivity"
    );
}

// ── policy の再利用（issue #801 の既存シームを再導出しない） ────────────────────────

#[test]
fn android_reuses_the_native_selection_policy_instead_of_re_deriving_it() {
    let app = src("app.rs");
    assert!(
        app.contains("native_renderer_selection_policy"),
        "Android は #801 が確立した hayate_app_host::renderer_selection::\
         native_renderer_selection_policy を再利用する（policy をアダプタ内で再導出しない）"
    );
    assert!(
        app.contains("renderer_config::VELLO_LINKED") && app.contains("renderer_config::forced_renderer()"),
        "policy への入力（vello_linked / forced）は renderer_config が用意する"
    );
}

// ── intent extra 由来の実行時上書き（再ビルド不要） ─────────────────────────────────

#[test]
fn runtime_override_comes_from_the_renderer_intent_extra_pushed_over_jni() {
    let kt = main_activity();
    assert!(
        kt.contains("hayate.renderer"),
        "実行時上書きの口は intent extra（hayate.renderer）——APK 再作成なしで vello/skia を\
         切り替える（#795 の hayate.backend/hayate.aa と同じ操作感）"
    );
    assert!(
        kt.contains("external fun") && kt.contains("nativePushRendererConfig"),
        "読んだ上書き値は JNI native 関数（nativePushRendererConfig）で Rust へ push する"
    );

    let bridge = src("jni_bridge.rs");
    assert!(
        bridge.contains("nativePushRendererConfig") && bridge.contains("store_pushed_renderer"),
        "JNI エクスポートは jni_bridge.rs に置き（封じ込め）、\
         renderer_config::store_pushed_renderer へ渡す"
    );
}

#[test]
fn switch_key_and_value_vocabulary_are_named_constants() {
    let config = src("renderer_config.rs");
    assert!(
        config.contains("pub const RENDERER_INTENT_EXTRA: &str = \"hayate.renderer\""),
        "切替キー名は名前付き定数"
    );
    assert!(
        config.contains("RENDERER_VALUE_VELLO") && config.contains("RENDERER_VALUE_SKIA"),
        "切替値語彙も名前付き定数（マジック文字列の禁止）"
    );
}

// ── 既定順序（vello → skia の名前付き定数） ─────────────────────────────────────────

#[test]
fn default_order_is_the_shared_native_renderer_order_constant() {
    // 確定値（既定 preferred renderer）自体は後続の完全人力 issue が決める——#802 の責務は
    // 「vello→skia の一方向 fallback」という順序そのものを名前付き定数で持つこと
    // （#801 が定義した NATIVE_RENDERER_ORDER を Android も共有する）。
    use hayate_app_host::renderer_selection::{SceneRendererKind, NATIVE_RENDERER_ORDER};
    assert_eq!(
        NATIVE_RENDERER_ORDER,
        [SceneRendererKind::Vello, SceneRendererKind::Skia]
    );
}

// ── logcat 記録（選択レンダラ・選択理由・fallback） ─────────────────────────────────

#[test]
fn selection_rejection_and_fallback_are_logged() {
    let app = src("app.rs");
    assert!(
        app.contains("selected scene renderer:"),
        "選択されたレンダラを logcat に出す（RenderHost::init_with_policy と同じ文言）"
    );
    assert!(
        app.contains("scene renderer rejected:"),
        "policy が事前却下したレンダラとその理由（RendererSelectionReason）を logcat に出す"
    );
    assert!(
        app.contains("GPU init failed"),
        "vello init 失敗は #795 の既存契約テストと同じ文言のまま——skia への一方向 fallback を駆動する"
    );
}

// ── skia raster surface の初期化・present 経路がアダプタに存在すること ─────────────────

#[test]
fn skia_raster_surface_is_wired_as_the_fallback_renderer() {
    let app = src("app.rs");
    assert!(
        app.contains("skia_window::init_skia_surface") && app.contains("spawn_skia_raster_thread"),
        "skia 選択時は ANativeWindow 向け CPU raster surface を初期化し、専用 Raster スレッドで駆動する"
    );

    let skia_window = src("skia_window.rs");
    assert!(
        skia_window.contains("ANativeWindow_lock") || skia_window.contains(".lock(None)"),
        "skia raster は ANativeWindow へ直接 present する（wgpu 非依存, ADR-0146 §3）"
    );
    assert!(
        skia_window.contains("scene_origin"),
        "safe-area インセット（b2, ADR-0144）のシーン平行移動を vello 経路と同様に適用する"
    );
}

#[test]
fn skia_present_uses_the_color_glyph_capable_renderer() {
    // paints_color_glyphs() = true な skia crate を使っていること（カラー絵文字 AC の土台）。
    let skia_present = src("skia_present.rs");
    assert!(
        skia_present.contains("hayate_scene_renderer_skia"),
        "present 経路は hayate-scene-renderer-skia（paints_color_glyphs=true）を使う"
    );
}

// ── vello 既定構成の挙動不変（skia は opt-in / fallback でのみ動く） ─────────────────

#[test]
fn vello_remains_the_first_attempt_when_nothing_is_forced() {
    use hayate_app_host::renderer_selection::{
        native_renderer_selection_policy, RendererCapabilities, SceneRendererKind,
    };
    let plan = native_renderer_selection_policy(true, None).choose(RendererCapabilities {
        webgpu_available: true,
    });
    assert_eq!(
        plan.attempt_order().first().copied(),
        Some(SceneRendererKind::Vello),
        "強制指定が無ければ vello が既定の第一候補のまま（skia は fallback 専用）"
    );
}
