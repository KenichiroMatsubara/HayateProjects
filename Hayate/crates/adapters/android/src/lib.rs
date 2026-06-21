//! Android プラットフォームアダプタ（ADR-0087）。
//!
//! `hayate-core` + `hayate-scene-renderer-vello` を Android 上で動かし、デモ
//! ボタン（`scene_demo`）を `SceneGraph` に降ろして毎フレーム描画する。タッチ
//! `MotionEvent` は `translate_touch` 経由で座標ベースのポインタ API に流れ、
//! タップでボタンの `:active` 色が切り替わる。IME ブリッジ（ADR-0094）では
//! GameTextInput の絶対バッファを `ime_input` で差分して core の編集呼び出しに
//! 変換する。
//!
//! 非 Android ターゲットでは no-op となり、ホストの `cargo build`/`cargo check`
//! に影響せずワークスペースに置ける。プラットフォーム非依存のシーム
//! （`surface_lifecycle`・`touch_input`・`scene_demo`・`ime_input`）はホスト上でも
//! コンパイルされ単体テストされる。

mod ime_input;
mod scene_demo;
mod surface_lifecycle;
mod touch_input;

#[cfg(target_os = "android")]
mod app;
#[cfg(target_os = "android")]
mod ime_bridge;

/// 実機スモークテスト用の RGBA クリアカラー。
pub const STAGE_A_CLEAR_COLOR: [f32; 4] = [0.1, 0.1, 0.12, 1.0];

#[cfg(target_os = "android")]
pub use app::android_main;
