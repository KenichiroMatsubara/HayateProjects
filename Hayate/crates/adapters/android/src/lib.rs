//! Android Platform Adapter (ADR-0087).
//!
//! Stage A: render an empty `SceneGraph` to a GPU surface to confirm
//! `hayate-core` + `hayate-scene-renderer-vello` build and run on Android.
//! Touch input (stage B) and IME/AccessKit/clipboard (stage C) are not
//! implemented yet.
//!
//! This crate is a no-op on non-Android targets so it can stay in the
//! workspace without affecting `cargo build`/`cargo check` on the host.

mod surface_lifecycle;
mod touch_input;

#[cfg(target_os = "android")]
mod app;

/// RGBA clear color for the stage A on-device smoke test (issue #195).
pub const STAGE_A_CLEAR_COLOR: [f32; 4] = [0.1, 0.1, 0.12, 1.0];

#[cfg(target_os = "android")]
pub use app::android_main;
