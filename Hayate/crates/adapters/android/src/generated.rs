//! `proto/spec/*.json` から生成した中立プロトコルコーデック（ADR-0112）。
//!
//! Android は wasm-bindgen / js_sys に依存しないため、中立化した `protocol.rs`
//! （`decode_style_packet` 等が `Result<_, String>` を返す）だけを include する。
//! Web 専用の `dom_style_mapper.rs`（web_sys）・`event_encode_web.rs`（js_sys）は
//! 取り込まない。
#![allow(dead_code)]

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../proto/generated/protocol.rs"
));
