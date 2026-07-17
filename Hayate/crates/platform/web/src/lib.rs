#[cfg(target_arch = "wasm32")]
mod backend;
#[cfg(any(target_arch = "wasm32", test))]
mod builtin_fonts;
#[cfg(test)]
mod delivery_codec_fixtures;
#[cfg(any(target_arch = "wasm32", test))]
mod edit_keymap;
mod generated;
#[cfg(any(target_arch = "wasm32", test))]
mod pointer_input;
pub mod pseudo_style_dom;
#[cfg(any(target_arch = "wasm32", test))]
mod resize_observer;
#[cfg(any(target_arch = "wasm32", test))]
mod tuning;
pub mod user_select;
#[cfg(test)]
mod wire_codec_roundtrip;
// 画像デコードの共通経路（#643）。純粋（image + core のみ）なのでホストでテストできる。
mod image_decode;
// render() の resize→present 順序（#666）。DOM/GPU 非依存の純粋モジュールでホストでテストできる。
#[cfg(target_arch = "wasm32")]
mod canvas;
#[cfg(any(target_arch = "wasm32", test))]
mod edit_context;
#[cfg(test)]
mod frame_surface;
#[cfg(target_arch = "wasm32")]
mod html;
#[cfg(any(target_arch = "wasm32", test))]
mod html_delivery;
#[cfg(any(target_arch = "wasm32", test))]
mod ime_bridge;
#[cfg(target_arch = "wasm32")]
mod shared;
#[cfg(any(target_arch = "wasm32", test))]
mod style_packet;
#[cfg(target_arch = "wasm32")]
mod wasm_impl;

#[cfg(target_arch = "wasm32")]
pub use canvas::*;
#[cfg(target_arch = "wasm32")]
pub use html::*;
#[cfg(target_arch = "wasm32")]
pub use shared::*;
