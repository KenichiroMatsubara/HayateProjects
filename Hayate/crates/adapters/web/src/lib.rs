#[cfg(target_arch = "wasm32")]
mod backend;
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
