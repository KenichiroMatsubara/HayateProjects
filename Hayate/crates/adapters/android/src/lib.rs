//! Android Platform Adapter (ADR-0087).
//!
//! Stage A confirmed `hayate-core` + `hayate-scene-renderer-vello` build and
//! present a frame on Android. Stage B drives an interactive element tree: a
//! demo button (`scene_demo`) is lowered to a `SceneGraph` and rendered each
//! frame, and touch `MotionEvent`s flow through `translate_touch` into the
//! coordinate-based pointer API so a tap flips the button's `:active` color
//! on screen. Stage C (ADR-0094) begins the IME bridge: tapping the demo
//! text-input shows the soft keyboard, and GameTextInput's absolute buffer is
//! diffed by `ime_input` into core edit calls. AccessKit/clipboard are next.
//!
//! This crate is a no-op on non-Android targets so it can stay in the
//! workspace without affecting `cargo build`/`cargo check` on the host; the
//! platform-independent seams (`surface_lifecycle`, `touch_input`,
//! `scene_demo`, `ime_input`) still compile and are unit-tested there.

mod ime_input;
mod scene_demo;
mod surface_lifecycle;
mod touch_input;

#[cfg(target_os = "android")]
mod app;
#[cfg(target_os = "android")]
mod ime_bridge;

/// RGBA clear color for the stage A on-device smoke test (issue #195).
pub const STAGE_A_CLEAR_COLOR: [f32; 4] = [0.1, 0.1, 0.12, 1.0];

#[cfg(target_os = "android")]
pub use app::android_main;
