# Android as the First Native Platform Adapter, Direct-API Over winit

**Status: Accepted**

**Date: 2026-06-12**

## Context

Hayate has only one Platform Adapter so far (`hayate-adapter-web`), built directly on `web-sys`/`wasm-bindgen` rather than a cross-platform windowing abstraction. `hayate-core` has no wasm/web-specific dependencies and is already platform-agnostic. We want to start native mobile work, and need to decide which platform first, what "done" looks like initially, and whether to adopt `winit` as a shared windowing layer across adapters.

## Decision

- Android is the first native Platform Adapter target; iOS follows later and is out of scope for this round.
- Staged scope: (A) rendering smoke test (`hayate-adapter-android` crate + example app, wgpu/Vulkan surface, no input/IME/AccessKit) → (B) touch input wired into the Element Document Runtime → (C) full adapter parity with `hayate-adapter-web` (IME bridge, AccessKit, clipboard).
- Use `android-activity` directly for window/surface lifecycle, mirroring `hayate-adapter-web`'s "talk to the platform's native API directly" approach.
- General principle: Platform Adapters do not use `winit` (or any generic windowing abstraction). IME (`EditContext` on web, `UITextInput`/`InputConnection` on iOS/Android) and AccessKit integration are platform-specific regardless, so `winit` would only abstract window creation while leaving the adapter's actual surface area untouched — at the cost of an extra vendored dependency.
- This is a parallel track alongside ADR-0051 (Tsubame-first priority), not a supersession; Tsubame/Hayate CSS work continues concurrently.
- The Rust sandbox used for this work lacks the Android NDK; verification here is limited to `cargo build --target aarch64-linux-android` compile checks, with emulator/device runs done locally.

## Consequences

### Positive

- Symmetric adapter architecture: each Platform Adapter is a thin, direct binding to its platform's native APIs, with `hayate-core` staying dependency-free of any of them.
- The (A) smoke test cheaply surfaces any accidental web-only assumptions in `hayate-core` before committing to a full adapter.

### Negative

- No shared windowing/event-loop code between adapters; each new platform adapter re-implements lifecycle/surface plumbing.
- Mobile work proceeds without emulator verification in this environment until run-time checks happen locally.
