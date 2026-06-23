# Android Runs Tsubame JS via Embedded Hermes Over a Native RawHayate Bridge

**Status: Draft**

**Date: 2026-06-21**

## Context

ADR-0087/0094 made Android the first native Platform Adapter and settled its packaging.
`crates/platform/mobile/android/src/app.rs` already renders, takes touch, and runs the GameTextInput
IME bridge on a device, owning the vsync loop, a wgpu/Vulkan surface, and an `ElementTree`. But
it lowers a Rust-side demo tree (`scene_demo::build_demo_tree`) directly; the `apply_mutations`
path that Tsubame drives is not exercised.

This round runs the actual Tsubame JS (`tsubame-solid` + `@tsubame/renderer-canvas` + the Todo
example) on a device, with the JS calling **native** Hayate. Not a WebView, not WASM — the cdylib
exists and `wasm-pkgs/` is Web-only. What is missing is a JS engine embedded in the cdylib plus a
bridge that lets the JS satisfy `RawHayate` (`Tsubame/packages/renderer-canvas/src/hayate.ts`)
against the native `ElementTree`.

The Tsubame↔Hayate seam is deliberately coarse (ADR-0052): per frame the JS crosses the boundary
only a few times with batched arrays — `apply_mutations(ops: Float64Array, styles: Float32Array,
texts: string[])`, `render`, `poll_events`. The apply logic (`apply_mutations_batch`,
`crates/platform/web/src/apply_mutations_dispatch.rs`) is platform-independent and reused. So the
host bridge is a fixed, one-time surface (~15 methods), not a per-frame cost — which is what makes
the engine choice turn on tooling/longevity rather than per-call marshalling ergonomics.

Engine options considered:

1. **Hermes (JSI)** — purpose-built for embedding a JS engine to drive native rendering (the React
   Native model). Bytecode AOT via `hermesc`, a Chrome DevTools debugger, and the clearest path to
   richer JS / RN interop later. Cost: outside RN it is a second build graph — link `libhermes.so`,
   write a small C++ translation unit using JSI `<jsi/jsi.h>`, and bridge to Rust (e.g. `cxx` over a
   flat C ABI).
2. **QuickJS (`rquickjs`)** — pure-Rust binding that folds into the existing cargo cdylib with no
   C++/CMake/AAR. Lower up-front plumbing, but interpreter-only and a dead end for tooling/longevity.
3. **V8 (`rusty_v8`)** — JIT, but 10 MB+/ABI and a heavy arm64 build; overkill here.

The coarse boundary neutralises Hermes' main downside (the JSI/C++ host is written once), leaving
its longevity advantages decisive. QuickJS's only real edge is up-front build simplicity — a
front-loaded convenience, not an architectural one.

## Decision

- Embed **Hermes** in the `hayate-adapter-android` cdylib and drive it over **JSI**. A thin C++
  TU creates the runtime and installs a `RawHayate` `jsi::HostObject`; Rust↔C++ crosses a flat C
  ABI (`cxx`). Ship the bundle as Hermes bytecode (`hermesc` → `.hbc`) in APK assets.
- The `ElementTree` is **shared single-threaded** between the native loop and the host
  (`Rc<RefCell<ElementTree>>`, per ADR-0003); the C++ host reaches it via extern-"C" Rust
  functions. The `RawHayate` contract (`hayate.ts`) is not modified — Float64/Float32Array and
  string[] marshalling is the target, not a renegotiation.
- **Reusing the apply path requires neutralising the proto codegen first.** The generated
  dispatch (`proto/generated/dispatch.rs`, emitted by `proto/generator/src/lib.rs`) is currently
  wasm-bindgen-typed — `apply_mutations_batch`/`decode_style_packet` take `&js_sys::Array` and
  return `Result<_, JsValue>` (the core `parse_next_op`/`parse_next_style_tag` are already
  neutral). The generator must emit a platform-neutral dispatch (`texts: &[String]`,
  `Result<_, String>`) so Android reuses it; the Web adapter then adapts at its boundary
  (`js_sys::Array` → `&[String]`, `String` → `JsValue`). This realises ADR-0055's
  wire-codec-single-source despite the extra step.
- **Native owns vsync + GPU present; JS owns app logic + mutations.** Input stays native→tree
  direct (unchanged); tree→JS deliveries flow through `poll_events`. The JS frame is invoked by
  the native loop via an injected `requestFrame`/`cancelFrame` (CanvasRenderer option) — it does
  not self-schedule. The Android `CanvasRenderer` is built with `canvas: null` + `autoResize:
  false`, so the existing `canvas !== null` guards skip the browser EditContext/ResizeObserver
  wiring and native GameTextInput stays the IME owner — no fork of shared Tsubame code. `init.ts`
  is unused; a new `init-android.ts` + `examples/todo/src/main.android.tsx` inject the host.
- **Non-destructive:** the JS path is added behind a Cargo feature (`tsubame-js`); `build_demo_tree`
  stays the default fallback so the verified native render asset is kept (ADR-0087).
- Gradle gains a `bundleTsubameJs` task (esbuild→`hermesc`→assets) and links the Hermes AAR; the
  existing `merge*JniLibFolders → cargoBuild` ordering is kept. `ndkVersion` is no longer hardcoded
  — it is sourced from `local.properties` / a Gradle property so CI and other developers resolve
  their own NDK.
- **Staged scope:** (A) Hermes-in-cdylib spike (eval round-trip) → (B) one static frame through
  `apply_mutations` → (C) full Solid + Todo, render-only → (D) touch via `poll_events` deliveries →
  (E) IME via the existing GameTextInput bridge (`on_text_input`/`on_composition_*`). AccessKit,
  clipboard, and scroll physics stay deferred (ADR-0087/0096/0046).

## Consequences

### Positive

- Longevity: Hermes is built for this use case — bytecode AOT, a real debugger, and a path to RN
  interop / heavier JS, none of which QuickJS offers.
- Reuses the native cdylib, `ElementTree`, touch, and IME plumbing; the JS layer slots into the
  middle of the frame with no fork of shared Tsubame code (`canvas: null` + injected `requestFrame`
  are first-class options already).
- Once the proto codegen is neutralised, `apply_mutations_batch` is shared across Web and Android,
  keeping the Tsubame↔Hayate contract one seam (ADR-0055).
- Non-destructive: the verified `build_demo_tree` path stays behind the default feature.

### Negative

- A second build graph: standalone Hermes embedding adds `libhermes.so`, a JSI/C++ TU, the `cxx`
  bridge, and an esbuild→`hermesc` bundling step — heavier than a pure-Rust engine. The JSI host
  (~15 methods) and the `main.android.tsx`/`init-android.ts` pair must track the browser entry.
- The Rust sandbox has no Android SDK/NDK/Gradle/device; like ADR-0087/0094 this is verified by
  host-readable contract tests (bridge marshalling, batch apply) and validated on a local device.
- A small Hayate-side accessor may be needed so the native loop can present the retained
  `SceneGraph` after the JS frame without re-running layout (to confirm against ADR-0086).
