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

// 中立 wire decode/encode（protocol.rs）は core が単一所有する（hayate_core::wire /
// Hayate Protocol Contract）。ここでは再 include せず core から取り込む。glob により
// 後続の include（codec / dom_style_mapper / event_encode_web）が `Op`・`TAG_*`・
// `decode_style_packet` 等の protocol シンボルと core スタイル型を同一スコープで
// 参照でき、外部の `crate::generated::*` 参照も従来どおり解決する。
use hayate_core::*;
pub use hayate_core::wire::*;

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
