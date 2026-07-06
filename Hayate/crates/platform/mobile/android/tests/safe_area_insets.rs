//! 最下点固定ピクセルずれの回帰防止（安全領域インセットの契約）。
//!
//! GameActivity の SurfaceView はフルウィンドウ（edge-to-edge）に配置され、実機
//! （実測: ColorOS / 1080x2400・ステータスバー110px・ナビバー132px）では
//! `AndroidApp::content_rect()` もフルウィンドウ (0,0,1080,2400) を返す——インセットは
//! onContentRectChanged ではなく WindowInsets 経由でしか届かない。そのため Rust 側の
//! `safe_window_dimensions`（content_rect 由来）だけでは補正が no-op になり、最下点が
//! ナビゲーションバー分だけ固定ピクセルずれて到達不能・上端がステータスバーの裏に潜る。
//!
//! 根治は Kotlin 側: MainActivity が WindowInsets（systemBars + displayCutout）を
//! SurfaceView のマージンとして適用し、ANativeWindow 自体を安全領域サイズにする。
//! これで Rust の window 寸法・レイアウトビューポート・タッチ座標が全て一致する。
//! Gradle/AGP はサンドボックスで実行できないため、ソースを読んで契約を固定する
//! （`apk_packaging.rs` と同じ方式）。

use std::fs;
use std::path::PathBuf;

fn main_activity() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "android-app/app/src/main/kotlin/com/hayateprojects/hayate/adapter_android_demo/MainActivity.kt",
    );
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

#[test]
fn main_activity_fits_the_surface_view_to_the_safe_area() {
    let kt = main_activity();
    assert!(
        kt.contains("setOnApplyWindowInsetsListener"),
        "MainActivity must listen to WindowInsets — content_rect() はフルウィンドウを返す端末が\
         あり、Rust 側だけでは最下点の固定ピクセルずれを補正できない"
    );
    assert!(
        kt.contains("systemBars()") && kt.contains("displayCutout()"),
        "systemBars + displayCutout を安全領域として扱う"
    );
    assert!(
        kt.contains("setMargins"),
        "インセットは SurfaceView のマージンとして適用し、ANativeWindow 自体を安全領域サイズにする"
    );
}

#[test]
fn touch_events_are_translated_into_surface_local_coordinates() {
    let kt = main_activity();
    assert!(
        kt.contains("onTouchEvent") && kt.contains("offsetLocation"),
        "GameActivity はタッチを Activity.onTouchEvent（ウィンドウ座標）で受けるため、\
         SurfaceView をマージンで下げたら MotionEvent も SurfaceView 相対へ平行移動しないと\
         タッチが systemBars.top 分だけ下にずれる"
    );
    assert!(
        kt.contains("onGenericMotionEvent"),
        "ホイール/ホバー等の generic motion も同じ座標系ずれの対象"
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
