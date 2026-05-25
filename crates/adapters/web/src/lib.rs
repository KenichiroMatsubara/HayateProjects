#[cfg(target_arch = "wasm32")]
mod element_renderer;
#[cfg(target_arch = "wasm32")]
mod gpu_surface;
#[cfg(target_arch = "wasm32")]
mod style_packet;
#[cfg(target_arch = "wasm32")]
mod wasm_impl;

#[cfg(target_arch = "wasm32")]
pub use element_renderer::*;
#[cfg(target_arch = "wasm32")]
pub use wasm_impl::*;
