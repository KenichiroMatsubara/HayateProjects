//! `proto/spec/*.json` から生成したプロトコル定数とコーデック。
//!
//! `OP_*` / `TAG_*` / `EVENT_KIND_*` 定数は TypeScript 向けのミラー。
//! Rust 側は代わりに `Op`・`StyleTag`・生成パーサを使う。
#![allow(dead_code)]

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
