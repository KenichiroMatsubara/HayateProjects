# iOS as the Second Native Platform Adapter (UIKit/Metal via a Thin Swift Host, Direct-API Over winit)

**Status: Accepted**

**Date: 2026-06-22**

## Context

ADR-0087 made Android the first native Platform Adapter (direct `android-activity`
binding, no `winit`), and ADR-0012 records that all platforms are equal-tier with the
order Web → desktop → iOS/Android. `hayate-core` is platform-agnostic, and the Android
adapter established a de-risk pattern that works without the platform SDK in this
sandbox: host-testable pure seams (`surface_lifecycle` / `touch_input` / `ime_input` /
`scene_demo`) plus `#[cfg(target_os="android")]` native glue plus packaging contract
tests that read source files (`tests/apk_packaging.rs`, `tests/ime_api_encapsulation.rs`).

This round brings up iOS as the second native adapter to the same staged scope, mirroring
that pattern. We need to decide how UIKit/Metal/UITextInput are bound, and how the surface
is created, without a Mac/iOS SDK available here.

## Decision

- iOS is the second native Platform Adapter (`hayate-adapter-ios`), reusing Android's
  staged scope: (A) render smoke test → (B) touch wired into the Element Document Runtime
  → (C) full parity with `hayate-adapter-web`/`-android` (IME bridge, AccessKit, clipboard).
- **No `winit`** (consistent with ADR-0087's general principle): IME (`UITextInput`) and
  AccessKit are platform-specific regardless, so a generic windowing abstraction would only
  hide window creation while leaving the adapter's real surface area untouched.
- **Thin Swift host owns UIKit; Rust stays ObjC-free (shape 1).** A small Swift
  `AppDelegate`/`SceneDelegate`/`HayateView` owns the `UIWindow`, the `CAMetalLayer`, the
  `CADisplayLink` frame loop, `UITouch`, and `UITextInput` conformance, and forwards
  everything to the Rust staticlib over a small C FFI (`hayate_ios_*`). This mirrors
  Android's "thin Kotlin host (`MainActivity : GameActivity()`) + logic in Rust", with
  Swift playing Kotlin's role. `UITextInput` protocol conformance is far more ergonomic in
  Swift than via objc2, which is why the binding lives there.
  - **Considered and deferred: Rust-owned UIKit via `objc2`** (`define_class!` a `UIView`,
    `UIApplicationMain` in Rust). It keeps the binding "pure Rust" like Android's
    `android-activity`, but it is much heavier objc glue, harder to debug without a Mac, and
    makes `UITextInput` conformance painful. Revisit only if the Swift host proves limiting.
- **wgpu Metal surface via `CAMetalLayer` + `SurfaceTargetUnsafe::CoreAnimationLayer`.**
  Swift owns the `CAMetalLayer` (the view's `layerClass`) and sets `drawableSize`; it hands
  the layer pointer to Rust, which builds the surface with
  `wgpu::Instance{ backends: METAL }` and `SurfaceTargetUnsafe::CoreAnimationLayer`. This
  keeps the Rust adapter free of any Apple-only crate (no `objc2`, no `raw-window-metal`),
  so it has no iOS-specific Rust dependencies at all — the cleanest analogue to
  `hayate-core` staying platform-agnostic — and sidesteps the `objc2`/`raw-window-metal` ⇄
  `wgpu` version-alignment risk. Vello (vendored) is wgpu-backend-agnostic and runs on
  Metal unchanged; `VelloSceneRenderer`/`VelloRenderTarget`/`create_blitter`/
  `create_target_view` are reused as-is.
- **IME model asymmetry vs Android — the one genuine divergence.** Android GameTextInput
  *reports an absolute full-buffer state* that the adapter diffs (`translate_text_input`).
  iOS UITextInput is *a protocol the adapter implements*: UIKit *pushes incremental
  callbacks* (`insertText:` / `deleteBackward` / `setMarkedText:selectedRange:` /
  `unmarkText`). So `ime_input` reuses Android's output half verbatim (`ImeAction`,
  `apply_ime_action`, 1:1 with core's `element_set_text_content`/`element_set_preedit`,
  ADR-0069) but the input half is a new command-driven model (`ImeCommand` → `ImeBuffer` →
  `Vec<ImeAction>`). `insertText:` *replaces* marked text (candidate commit), `unmarkText`
  confirms it, `setMarkedText:` sets the preedit, `deleteBackward` pops the marked tail or
  the committed tail (UTF-8 char-boundary safe). The same core contract is host-tested
  through a Japanese composition, exactly as the Android seam is.
- **Content scale becomes real.** Android fixes `content_scale = 1.0`; iOS threads the real
  `UIScreen.scale` (2.0/3.0) through `hayate_core::ViewportMetrics::from_physical_size`, so
  iOS is the first adapter to exercise the `content_scale > 1.0` path (renderers already
  support it). Layout/hit-test run in logical points and `UITouch` reports points, so the
  pointer/viewport spaces stay aligned; only the GPU surface works in pixels.
- This is a parallel track alongside ADR-0051 (Tsubame-first); the Tsubame JS path on iOS
  is policy-only this round (ADR-0115).

## Consequences

### Positive

- Symmetric de-risk: the host-testable seams prove `hayate-core` has no iOS-hostile
  assumptions before any Mac is involved (37 host tests cover the state machine, touch,
  the IME command model incl. Japanese composition driven against a real `ElementTree`,
  and the packaging contract).
- The Rust adapter has zero Apple-only dependencies; all platform binding is Swift, the
  native language for UIKit/UITextInput.
- Vello/wgpu reuse means no renderer work for Metal.

### Negative

- An adapter whose platform binding is Swift, not Rust — an asymmetry with Android's
  Rust-side `android-activity` binding (justified by UITextInput ergonomics, recorded above).
- No shared windowing/event-loop code between adapters (same trade-off as ADR-0087).
- The Rust sandbox has no Mac/iOS SDK/linker, so `src/app.rs` (the `#[cfg(target_os="ios")]`
  glue), the Swift host, and the Xcode build cannot be built/run here; they are verified by
  host-readable contract tests and validated on a local Mac/simulator/device — the same
  verification gap as ADR-0087/0094 (track a device-verification issue analogous to #195).
  `aarch64-apple-ios` is not installed in this sandbox and a full build needs the Apple SDK
  regardless, so even a target compile-check is deferred to a Mac.
