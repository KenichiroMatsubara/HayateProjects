//! Protocol constants and codecs generated from `proto/spec/*.json`.
//!
//! `OP_*` / `TAG_*` / `EVENT_KIND_*` constants are mirrored for TypeScript;
//! Rust uses `Op`, `StyleTag`, and generated parsers instead.
#![allow(dead_code)]

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../proto/generated/protocol.rs"
));
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../proto/generated/dom_style_mapper.rs"
));
