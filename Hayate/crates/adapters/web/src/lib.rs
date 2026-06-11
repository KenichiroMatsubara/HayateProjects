#[cfg(target_arch = "wasm32")]
mod backend;
#[cfg(any(target_arch = "wasm32", test))]
mod builtin_fonts;
#[cfg(any(target_arch = "wasm32", test))]
mod generated;
#[cfg(test)]
mod delivery_codec_fixtures;
#[cfg(test)]
mod wire_codec_roundtrip;
#[cfg(target_arch = "wasm32")]
mod apply_mutations_dispatch;
#[cfg(any(target_arch = "wasm32", test))]
mod resize_observer;
#[cfg(target_arch = "wasm32")]
mod canvas;
#[cfg(target_arch = "wasm32")]
mod html;
#[cfg(target_arch = "wasm32")]
mod shared;
#[cfg(any(target_arch = "wasm32", test))]
mod ime_bridge;
#[cfg(any(target_arch = "wasm32", test))]
mod html_delivery;
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
