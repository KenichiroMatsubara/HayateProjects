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

// 音声出力 capability の Android leaf（ADR-0117）。Family Adapter（`hayate-adapter-mobile`）が
// cfg(target_os) でリンクし統一 facade として露出するため pub。純粋部分はホストでコンパイル/
// テストされ、AudioTrack FFI glue は target_os="android" のみ。
pub mod audio_output;
// QR スキャナ leaf（ADR-0125）。Code Scanner は Kotlin/Java API なので本 leaf は唯一の
// Rust↔Kotlin JNI seam。純粋部（契約再露出）はホスト、JNI glue は target_os="android" のみ。
pub mod qr_scanner;
// wave-1 capability scaffold stub（ADR-0119）。Family Adapter が cfg(target_os) でリンクし
// `MobileXxx` facade として露出するため pub。純粋 stub なのでホストでもコンパイル/テストされる。
pub mod capability_stubs;
// Torimi Android ホストのバンドル源（#532）。dev-server からの HTTP fetch + marshalling。
// プラットフォーム非依存（素の TCP / std）なのでホストでもコンパイル・テストされる。
#[cfg(feature = "tsubame-js")]
mod bundle_source;
// Torimi Android ホストが接続する dev-server の指定（#534）。端末 UI が入れた URL を host:port へ
// 正規化し、fetch / reload の両方を同じ target で駆動する。純粋なのでホストでもコンパイル・テストされる。
#[cfg(feature = "tsubame-js")]
mod dev_server_target;
// Demo Manifest（`/demos.json`）のパース・エントリ選択 → boot target 解決・取得失敗の明示エラー
// （ADR-0003 / #743）。プラットフォーム非依存の純 Rust なのでホストでもコンパイル・テストされる。
#[cfg(feature = "tsubame-js")]
mod demo_manifest;
// Torimi Android ホストの protocol version 突き合わせ（#533）。Web #530 と同じ contract の
// 純 Rust ミラー。プラットフォーム非依存なのでホストでもコンパイル・テストされる。
#[cfg(feature = "tsubame-js")]
mod protocol_handshake;
// Torimi Android ホストの Device Log 送信シーム（#787-789・ADR-0005）。JS/host ログをバッファに
// 積み seq を採番、定期／即時にバッチ化して注入ポートで POST する純 Rust シーム。注入 clock ＋
// モック送信ポートでホスト上でコンパイル・テストされ、OkHttp/JSI 配線は device 専用の薄いグルー。
#[cfg(feature = "tsubame-js")]
mod device_log;
// Torimi Android ホストの full reload ループ orchestration（#533）。device 依存を注入シームに
// 逃がした純 Rust なのでホストでもコンパイル・テストされる。
#[cfg(feature = "tsubame-js")]
mod torimi_reload;
// reload WS クライアント（#533）。subscribe_reload の device connect シーム。中身は素の std なので
// ホストでコンパイル検証はできるが、実駆動は device の app_tsubame のみ。
#[cfg(feature = "tsubame-js")]
mod reload_socket;
mod scene_demo;
// on-demand フレームループの起床/継続判定（ADR-0117 / ADR-0126）。android 非依存の純粋
// 判定器なので、ホストの cargo test で振る舞いを固定する（app_tsubame の実ループが利用）。
mod frame_schedule;
mod surface_lifecycle;
// 安全領域インセット（edge-to-edge / b2, issue #794・ADR-0144）。Kotlin から JNI で push された
// WindowInsets（systemBars + displayCutout）の格納庫＋純粋計算（レイアウトビューポート縮小・
// シーン平行移動原点・タッチ座標補正）。android 非依存なのでホストでコンパイル・テストされ、
// `app.rs`（android のみ）がフレームループから消費する。
mod safe_area;
// 描画バックエンド（Vulkan/GL）・AA 方式（Area/MSAA8/MSAA16）のランタイム選択（issue #795・
// ADR-0145）。intent extra 由来の上書き解釈・既定値（名前付き定数）・実効設定のグローバル格納。
// android 非依存の純粋部（enum・resolve・格納）はホストでコンパイル・テストされ、intent extra の
// 取得（Kotlin→Rust JNI push）と wgpu instance への適用は device 専用の薄いグルー。
mod render_config;
// レンダラ（vello/skia）のランタイム選択（issue #802、spec §4 REND-15、ADR-0146/0147）。
// intent extra（`hayate.renderer`）由来の強制指定解釈・グローバル格納。実際の選択ロジックは
// `hayate_app_host::renderer_selection::native_renderer_selection_policy`（issue #801）を
// 再利用する——本モジュールはその入力（forced override）を用意するだけ。android 非依存の
// 純粋部はホストでコンパイル・テストされ、intent extra の取得（Kotlin→Rust JNI push）は
// device 専用の薄いグルー。
mod renderer_config;
// skia raster フレームの CPU present 用ピクセル変換（issue #802・ADR-0146 §3）。desktop の
// `skia_present.rs`（softbuffer 0RGB）と同型——ANativeWindow へは RGBX_8888 で直接書く。
// hayate-scene-renderer-skia の raster surface 生成・読み戻しだけに依存し ndk には触れない
// ので、android 非依存でホストでもコンパイル・テストされる（`skia_window.rs` が device 専用の
// ANativeWindow 提示面を持つ）。
mod skia_present;
// 実機発音検証用のテストトーン生成器（ADR-0117 / #562）。NDK 非依存の純粋計算なので
// ホストでもコンパイル・テストされ、AAudio glue（audio_output.rs）はこのバッファを書く
// だけの薄いグルーに保つ。
mod test_tone;
mod touch_input;
// タッチドラッグ→スクロール配線（ADR-0082）。Web アダプタ（`hayate-adapter-web`）の
// 参照実装を移植した platform-free なジェスチャ配線層。NDK 非依存なのでホストで
// コンパイル・テストされ、`app.rs::process_touch_input`（android のみ）が実 MotionEvent
// からこれを駆動する薄いグルーに徹する。
mod touch_scroll;

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
// skia raster の ANativeWindow 提示面（issue #802・ADR-0146 §3）。wgpu 非依存の CPU present —
// vello の GpuSurface（GPU）と並立し、Renderer Selection Policy の一方向 fallback 先。
#[cfg(target_os = "android")]
mod skia_window;
// Rust↔Kotlin JNI の共通下地（ADR-0125 の「封じ込め」方針の一般化）。`jni::`/`ndk_context::`
// の直接使用はここ 1 ファイルに封じ込め、`qr_scanner` / `error_overlay` はこれだけを使う
// （`tests/qr_scanner_encapsulation.rs` が強制）。
#[cfg(target_os = "android")]
mod jni_bridge;
// boot 失敗・panic を画面に出すネイティブ View オーバーレイ（#530 系）。Hayate（要素ツリー→
// GPU パイプライン）に一切依存しない「潰れない土台」なので、Hayate/GPU 自体の初期化が壊れて
// いても呼べる（Web ホストの生 DOM error panel と対称）。JNI 依存のため device 専用。
#[cfg(target_os = "android")]
mod error_overlay;

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
