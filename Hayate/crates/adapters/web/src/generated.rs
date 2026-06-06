//! Protocol constants and codecs generated from `proto/protocol.yaml`.
//!
//! `OP_*` / `TAG_*` / `EVENT_KIND_*` constants are mirrored for TypeScript;
//! Rust uses `Op`, `StyleTag`, and generated parsers instead.
#![allow(dead_code)]
include!(concat!(env!("OUT_DIR"), "/protocol.rs"));
include!(concat!(env!("OUT_DIR"), "/dom_style_mapper.rs"));
