# iOS Packaging: Xcode Project + cargo staticlib Over cargo-mobile/SPM

**Status: Accepted**

**Date: 2026-06-22**

## Context

ADR-0114 made iOS the second native Platform Adapter with a thin Swift host that links a
Rust staticlib. ADR-0094 settled Android's packaging as a Gradle project that builds the
Rust cdylib into the APK, with the Gradle/Manifest files as the single source of truth and
host-readable contract tests (`tests/apk_packaging.rs`) standing in for the SDK-less
sandbox. iOS needs the equivalent decision: how the app is packaged, how the Rust library
is built and linked, and where the bundle id / capabilities / scene manifest live.

Options considered:

1. **Xcode project + a cargo Run-Script build phase** — a committed `.xcodeproj` with a
   shell build phase that runs `cargo build --target aarch64-apple-ios[-sim]` and links the
   resulting staticlib. Standard Apple toolchain, the most direct analogue to Android's
   Gradle project, and the clearest "single source of truth in Info.plist/pbxproj".
2. **`cargo-mobile2` / generated project** — generates the Xcode project from a template.
   Useful, but adds a generator dependency and obscures the packaging contract we want to
   pin in the repo.
3. **Swift Package Manager** — SPM doesn't produce an app bundle on its own; still needs an
   Xcode app target. No win over option 1 for an app.

## Decision

- Adopt **option 1**: a committed Xcode project (`crates/platform/mobile/ios/ios-app/Hayate.xcodeproj`)
  with a "Cargo Build" Run-Script phase that compiles the Rust staticlib for the active
  SDK/arch and links it. This is the iOS analogue of ADR-0094's Gradle project +
  `rust-android-gradle`.
- **crate-type = `["staticlib", "rlib"]`.** iOS links a *static* library into the app binary
  (unlike Android's `dlopen`-ed cdylib), so `staticlib` replaces `cdylib`; `rlib` keeps the
  host-testable seams runnable under `cargo test` on the dev machine (same reason as Android).
- **`Info.plist` is the single source of truth** for the bundle id
  (`com.hayateprojects.hayate.adapter_ios_demo`), the `UIApplicationSceneManifest` (the
  UIScene lifecycle the `surface_lifecycle` machine consumes), and the Metal requirement
  (`UIRequiredDeviceCapabilities` → `metal`) — the iOS analogue of Android's
  `android.hardware.vulkan.level`.
- **Hand-written Swift is kept thin**: `AppDelegate`/`SceneDelegate`/`HayateView` only host
  the view and relay lifecycle/touch/IME to Rust (ADR-0114); a bridging header declares the
  `hayate_ios_*` FFI.
- **The packaging contract is pinned by `tests/ios_packaging.rs`** (reads the source files,
  no Mac needed): staticlib crate-type, the Swift host owning the `CAMetalLayer`/UITextInput,
  the Rust glue selecting the Metal backend via `CoreAnimationLayer`, the Info.plist bundle
  id / Metal capability / scene manifest, and the Xcode project linking
  `libhayate_adapter_ios.a`. `tests/ime_api_encapsulation.rs` confines the keyboard-control
  FFI (`hayate_ios_set_keyboard_visible`) to `ime_bridge.rs`.

## Consequences

### Positive

- Single source of truth for packaging (Info.plist + pbxproj) mirrors Android's Gradle/Manifest.
- Standard Apple toolchain (Xcode/xcodebuild) — a clear path to a signed, distributable build.
- The contract is reviewable and CI-checkable without a Mac, matching ADR-0094.

### Negative

- A hand-authored `.pbxproj` is verbose and easy to drift; it should be opened/normalised in
  Xcode. It is committed so the contract is reviewable, but the real build needs a Mac.
- The Rust sandbox has no Xcode/SDK/linker, so the Xcode build, the Swift host, and a device
  run are validated on a local Mac/simulator/device — the verification gap noted in
  ADR-0114/0087/0094 (issue analogous to #195).
- `IPHONEOS_DEPLOYMENT_TARGET` (currently 13.0) is a packaging choice to confirm on device.
