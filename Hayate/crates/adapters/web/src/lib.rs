#[cfg(target_arch = "wasm32")]
mod backend;
#[cfg(any(target_arch = "wasm32", test))]
mod generated;
#[cfg(target_arch = "wasm32")]
mod apply_mutations_dispatch;
#[cfg(target_arch = "wasm32")]
mod element_renderer;
#[cfg(target_arch = "wasm32")]
mod renderer_event_state;
#[cfg(any(target_arch = "wasm32", test))]
mod style_packet;
#[cfg(target_arch = "wasm32")]
mod wasm_impl;

#[cfg(target_arch = "wasm32")]
pub use element_renderer::*;
#[cfg(target_arch = "wasm32")]
pub use wasm_impl::*;
