# hayate-adapter-ios — Xcode app skeleton

Thin Swift host that links the `hayate_adapter_ios` staticlib and drives Hayate on iOS
(ADR-0113 / ADR-0114). This is **groundwork**: the host-testable Rust seams are verified by
`cargo test -p hayate-adapter-ios` on any machine, but building/running this app needs a
**Mac with Xcode** (the verification gap recorded in ADR-0113, mirroring Android's #195).

## Layout

- `Hayate.xcodeproj/` — app target with a "Cargo Build" Run-Script phase that compiles the
  Rust staticlib for the active SDK/arch and links `libhayate_adapter_ios.a`.
- `Hayate/AppDelegate.swift` / `SceneDelegate.swift` — thin lifecycle host; folds the UIScene
  lifecycle into the four logical `surface_lifecycle` events.
- `Hayate/HayateView.swift` — owns the `CAMetalLayer`, the `CADisplayLink` frame loop,
  `UITouch`, and keyboard input (`UIKeyInput`); forwards to the `hayate_ios_*` FFI.
- `Hayate/Hayate-Bridging-Header.h` — the Rust → Swift FFI surface.
- `Hayate/Info.plist` — bundle id, Metal capability, UIScene manifest (the packaging
  contract, pinned by `tests/ios_packaging.rs`).

## Building on a Mac

```sh
# one-time: add the iOS Rust targets
rustup target add aarch64-apple-ios aarch64-apple-ios-sim

# the Xcode "Cargo Build" phase runs cargo automatically; or build the lib by hand:
cargo build -p hayate-adapter-ios --target aarch64-apple-ios-sim

# then open and run the app (simulator or device):
open ios-app/Hayate.xcodeproj
```

The hand-authored `project.pbxproj` is committed so the packaging contract is reviewable
without a Mac; open and re-save it in Xcode to normalise.

## Known-deferred (stage C / device)

- Full `UITextInput` conformance (marked-text geometry) — the Swift host currently wires the
  commit path via `UIKeyInput`; the Rust `ime_input` already models `SetMarked`/`Unmark`.
- AccessKit (`UIAccessibility`), clipboard, scroll physics (ADR-0046).
