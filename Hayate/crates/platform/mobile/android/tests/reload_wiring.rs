//! Miharashi Android ホストの full reload ＋ protocol version 整合の device 配線契約（#533）。
//!
//! 突き合わせの純ロジック（`protocol_handshake`）と再構築の orchestration（`miharashi_reload`）は
//! ホスト単体テストで緑（src/*.rs の `#[cfg(test)]`）。一方 `app_tsubame` / C++ JSI / 実 WS は
//! device 専用でホストにはコンパイルされない（ADR-0112）。そこで apk_packaging.rs / bundle_source_wiring.rs
//! と同じく、ソースを読んで「host が版数を突き合わせ、WS reload で Hermes ランタイムを作り直す」配線が
//! 据わっていることを固定する。実描画と reload 体験はローカル実機で検証する（本 issue 外）。

use std::fs;
use std::path::PathBuf;

fn read_relative(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn app_tsubame_src() -> String {
    read_relative("src/app_tsubame.rs")
}

#[test]
fn host_checks_protocol_version_against_the_native_decoder_version() {
    let src = app_tsubame_src();
    // 焼き込み版数は Web ホストと同じ source of truth（生成された native decoder 版数）。
    assert!(
        src.contains("hayate_core::wire::PROTOCOL_VERSION"),
        "the host must bake hayate_core::wire::PROTOCOL_VERSION as its decoder version (#530/#533)"
    );
    // 突き合わせは boot_runtime 経由（#530 と共有の純ロジックを呼ぶ）。
    assert!(
        src.contains("boot_runtime("),
        "boot must run through miharashi_reload::boot_runtime (fetch → build/eval → handshake)"
    );
    // バンドルが立てた version global を読む（C++ JSI 経由）。
    assert!(
        src.contains("read_bundle_protocol_version") && src.contains("protocol_version()"),
        "the host must read the bundle's __miharashiProtocolVersion via the JSI reader"
    );
}

#[test]
fn version_mismatch_is_reported_explicitly_without_crashing() {
    let src = app_tsubame_src();
    // 不一致／取得失敗は明示エラーにして pump に進めない（current=None のまま回す）。
    assert!(
        src.contains("BootError::ProtocolMismatch") && src.contains("report_boot_error"),
        "a protocol mismatch must surface as an explicit error, not a crash (#530)"
    );
}

#[test]
fn host_subscribes_to_ws_reload_and_rebuilds_the_runtime() {
    let src = app_tsubame_src();
    // WS reload を購読し（device connect は reload_socket）、受信で再 boot（= Hermes 再構築）。
    assert!(
        src.contains("subscribe_reload(") && src.contains("connect_reload_ws"),
        "the host must subscribe to the dev-server reload WS (#533)"
    );
    // reload 受信フラグを拾って boot() を呼び直す（full reload。tree ごと作り直す）。
    assert!(
        src.contains("reload_requested") && src.contains("ElementTree::new()"),
        "a reload must rebuild the runtime and a fresh tree (full reload, state is dropped)"
    );
}

#[test]
fn ws_reconnect_backoff_uses_a_named_constant() {
    // インラインのマジックナンバー無し：backoff は miharashi_reload の名前付き定数（#533 / 実値は #8）。
    let reload_src = read_relative("src/miharashi_reload.rs");
    assert!(
        reload_src.contains("WS_RECONNECT_BACKOFF"),
        "the WS reconnect backoff must be a named constant (no inline magic number)"
    );
}

#[test]
fn jsi_bridge_exposes_the_bundle_protocol_version_reader() {
    // C++ JSI ホストが globalThis.__miharashiProtocolVersion を読み、cxx ブリッジが Rust へ橋渡す。
    let cpp = read_relative("cpp/hermes_app.cpp");
    assert!(
        cpp.contains("__miharashiProtocolVersion"),
        "the C++ host must read the bundle's __miharashiProtocolVersion global (#533)"
    );
    let bridge = read_relative("src/hermes_bridge.rs");
    assert!(
        bridge.contains("fn protocol_version(self: &HermesApp)"),
        "the cxx bridge must expose HermesApp::protocol_version to Rust (#533)"
    );
}
