# Android Packaging: GameActivity + Gradle Over cargo-apk/NativeActivity

**Status: Accepted**

**Date: 2026-06-14**

## Context

ADR-0087 made Android the first native Platform Adapter and chose `android-activity`
as the direct platform binding, with a staged scope ending at (C) full parity with
`hayate-adapter-web` — IME bridge, AccessKit, clipboard. Stages A (render smoke test)
and B (touch + element-tree rendering) were built on `android-activity`'s
`native-activity` backend and packaged with `cargo-apk`, which needs no Java/Kotlin:
it reuses the framework's built-in `android.app.NativeActivity` and auto-generates the
manifest.

Stage C forces a decision ADR-0087 deferred. Android soft-keyboard text input requires
the host `Activity` to vend an `InputConnection` (`onCreateInputConnection`).
`NativeActivity` exposes no hook for this, and `cargo-apk` cannot compile Kotlin/Java,
so the IME bridge cannot be built on the current packaging path. Any path that gives us
a real `InputConnection` introduces Kotlin/Java and therefore a Gradle build.

Options considered:

1. **GameActivity + Gradle** — switch `android-activity` to its `game-activity` backend.
   `GameActivity` (androidx `games-activity` AAR) bundles `GameTextInput`, which surfaces
   soft-keyboard text into native code as input events, so the IME data path needs only a
   few lines of hand-written Kotlin (a `GameActivity` subclass). Requires Gradle + the AAR;
   drops `cargo-apk`.
2. **NativeActivity + custom Kotlin Activity** — keep `native-activity`, add a Kotlin
   `Activity` overriding `onCreateInputConnection` and bridge composition over JNI by hand.
   No AAR, but the most bespoke Kotlin/JNI code, and still needs Gradle to compile Kotlin.
3. **Defer IME** — stay on `cargo-apk`/`native-activity`, do clipboard + AccessKit first via
   JNI from Rust, postpone this decision.

## Decision

- Adopt **GameActivity + Gradle** (option 1) as the canonical Android packaging and Activity
  backend for `hayate-adapter-android`.
- `android-activity` switches from the `native-activity` feature to `game-activity`. The Rust
  entry point (`android_main(app: AndroidApp)`) is unchanged — `android-activity` abstracts both
  backends behind the same API.
- Packaging moves from `cargo-apk` to a Gradle project (`crates/adapters/android/android-app/`)
  using the `rust-android-gradle` plugin to build the Rust cdylib into the APK. The Gradle/Manifest
  files become the single source of truth for the package id, target ABI, SDK levels, and the
  `android.hardware.vulkan.level` feature; the `[package.metadata.android]` `cargo-apk` block in
  `Cargo.toml` is removed.
- Hand-written Kotlin is kept to a thin `class MainActivity : GameActivity()`; all app logic stays
  in Rust. GameActivity is chosen specifically to minimise hand-written Kotlin/JNI for the stage C
  IME bridge, not for any game-loop semantics.
- This refines ADR-0087's packaging assumption (it noted `cargo apk build` and arm64-only); it does
  not change the "direct `android-activity` binding, no `winit`" decision.

## Consequences

### Positive

- Stage C IME has a supported text-input path (`GameTextInput`) with minimal Kotlin, instead of a
  hand-rolled `InputConnection`/JNI bridge.
- Single source of truth for packaging (Gradle/Manifest) replaces the parallel `cargo-apk` metadata.
- Standard Android toolchain (Gradle/AGP) — easier path to a Play-distributable build than `cargo-apk`.

### Negative

- Introduces Gradle, AGP, Kotlin, and the `games-activity` AAR into the build — heavier than the
  pure-Rust `cargo-apk` path used for stages A/B.
- The Rust sandbox has no Android SDK/NDK/Gradle, so the Gradle project, Kotlin stub, and Manifest
  cannot be built or run here; they are verified by host-readable contract tests (`tests/apk_packaging.rs`)
  and validated on a local device/emulator (the verification gap noted in ADR-0087 and issue #195).
- `minSdk` is raised to GameActivity's supported floor (24), above the stage A `cargo-apk` value (21).
