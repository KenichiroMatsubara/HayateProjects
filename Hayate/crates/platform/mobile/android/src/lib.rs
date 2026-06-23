//! Android プラットフォームアダプタ（ADR-0087）。
//!
//! `hayate-core` + `hayate-scene-renderer-vello` を Android 上で動かし、デモ
//! ボタン（`scene_demo`）を `SceneGraph` に降ろして毎フレーム描画する。タッチ
//! `MotionEvent` は `translate_touch` 経由で座標ベースのポインタ API に流れ、
//! タップでボタンの `:active` 色が切り替わる。IME ブリッジ（ADR-0094）では
//! GameTextInput の絶対バッファを core の `ime_reconcile` で差分して core の編集
//! 呼び出しに変換する（差分ロジックは core 所有。アダプタは android-activity の
//! テキスト入力状態を core 型へマップする薄いグルーのみ）。
//!
//! 非 Android ターゲットでは no-op となり、ホストの `cargo build`/`cargo check`
//! に影響せずワークスペースに置ける。プラットフォーム非依存のシーム
//! （`surface_lifecycle`・`touch_input`・`scene_demo`）はホスト上でも
//! コンパイルされ単体テストされる。

mod scene_demo;
mod surface_lifecycle;
mod touch_input;

// Tsubame JS 駆動経路の Rust 半分（ADR-0112）。埋め込み Hermes が呼ぶ
// apply_mutations を、Web と共有の中立 dispatch 経由で ElementTree に適用する。
// 既定 OFF（非破壊）。プラットフォーム非依存なのでホストでもコンパイル・テストできる。
#[cfg(feature = "tsubame-js")]
mod js_apply;
#[cfg(feature = "tsubame-js")]
mod js_host;

#[cfg(target_os = "android")]
mod app;
#[cfg(target_os = "android")]
mod ime_bridge;

// JS 駆動ループと Hermes(JSI) ブリッジ（ADR-0112）。device 専用（C++/libhermes が
// 要る）なので android かつ feature のときだけコンパイルする。ホスト検証には載らない。
#[cfg(all(target_os = "android", feature = "tsubame-js"))]
mod app_tsubame;
#[cfg(all(target_os = "android", feature = "tsubame-js"))]
mod hermes_bridge;

/// 実機スモークテスト用の RGBA クリアカラー。
pub const STAGE_A_CLEAR_COLOR: [f32; 4] = [0.1, 0.1, 0.12, 1.0];

#[cfg(target_os = "android")]
pub use app::android_main;
