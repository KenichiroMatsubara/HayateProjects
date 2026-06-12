//! Android Platform Adapter (ADR-0087).
//!
//! Stage A: render an empty `SceneGraph` to a GPU surface to confirm
//! `hayate-core` + `hayate-scene-renderer-vello` build and run on Android.
//! Touch input (stage B) and IME/AccessKit/clipboard (stage C) are not
//! implemented yet.
//!
//! This crate is a no-op on non-Android targets so it can stay in the
//! workspace without affecting `cargo build`/`cargo check` on the host.

#[cfg(target_os = "android")]
mod app;

#[cfg(target_os = "android")]
pub use app::android_main;
