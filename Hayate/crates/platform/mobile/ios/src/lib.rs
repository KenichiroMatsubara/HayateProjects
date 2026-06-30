//! iOS プラットフォームアダプタ（ADR-0114）。
//!
//! `hayate-core` + `hayate-scene-renderer-vello` を iOS 上で動かし、デモボタン
//! （`scene_demo`）を `SceneGraph` に降ろして CADisplayLink ごとに Metal へ描画する。
//! UITouch は `translate_touch` 経由で座標ベースのポインタ API に流れ、タップで
//! ボタンの `:active` 色が切り替わる。IME は UITextInput の増分コールバック
//! （`insertText` / `setMarkedText` / ...）を `ime_input` でコアの「確定 text_content
//! + 末尾 preedit」モデルへ変換する。
//!
//! Android アダプタ（ADR-0087）の de-risk 構造を鏡写しにする。すなわち、プラット
//! フォーム非依存のシーム（`surface_lifecycle` / `touch_input` / `ime_input` /
//! `scene_demo`）はホストでもコンパイルされ単体テストされ、`#[cfg(target_os="ios")]`
//! の objc2/UIKit/Metal グルー（`app` / `ime_bridge`）は iOS ターゲットでのみ
//! コンパイルされる（Mac/実機検証はローカル）。非 iOS ターゲットではシームのみ
//! ビルドされ、ホストの `cargo build`/`cargo check` に影響しない。

// 音声出力 capability の iOS leaf（ADR-0117）。Family Adapter（`hayate-adapter-mobile`）が
// cfg(target_os) でリンクし統一 facade として露出するため pub。純粋部分はホストでコンパイル/
// テストされ、AVAudioEngine FFI glue は target_os="ios" のみ。
pub mod audio_output;
// QR スキャナ leaf（ADR-0125）。VisionKit DataScannerViewController を Swift ホスト経由で呼ぶ。
// 純粋部（契約再露出）はホスト、FFI glue は target_os="ios" のみ。Android leaf と対称。
pub mod qr_scanner;
// wave-1 capability scaffold stub（ADR-0119）。Family Adapter が cfg(target_os) でリンクし
// `MobileXxx` facade として露出するため pub。純粋 stub なのでホストでもコンパイル/テストされる。
pub mod capability_stubs;
mod ime_input;
mod scene_demo;
mod surface_lifecycle;
mod touch_input;

#[cfg(target_os = "ios")]
mod app;
#[cfg(target_os = "ios")]
mod ime_bridge;

/// 実機スモークテスト用の RGBA クリアカラー。Android と同値にして、両ネイティブ
/// アダプタの stage A 目視確認を揃える。
pub const STAGE_A_CLEAR_COLOR: [f32; 4] = [0.1, 0.1, 0.12, 1.0];

#[cfg(target_os = "ios")]
pub use app::ios_main;
