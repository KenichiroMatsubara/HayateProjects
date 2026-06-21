//! `proto/spec/*.json` から生成したプロトコル定数とコーデック。
//!
//! `OP_*` / `TAG_*` / `EVENT_KIND_*` 定数は TypeScript 向けのミラー。
//! Rust 側は代わりに `Op`・`StyleTag`・生成パーサを使う。
#![allow(dead_code)]

// Web 専用の生成ファイル（dom_style_mapper / event_encode_web）は JsValue を
// モジュールスコープで参照する。以前は中立化前の protocol.rs が
// `use wasm_bindgen::prelude::*` を持ち込んでいたが、protocol.rs を wasm 非依存に
// したため（ADR-0112）、この include モジュール側で明示する。
use wasm_bindgen::prelude::*;

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../proto/generated/protocol.rs"
));
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../proto/generated/codec.rs"
));
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../proto/generated/dom_style_mapper.rs"
));
// js_sys ベースのイベントエンコーダ（Web 専用 / ADR-0112）。中立な protocol.rs
// から分離したため、Web 側で明示的に include して `encode_*` を取り込む。
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../proto/generated/event_encode_web.rs"
));
