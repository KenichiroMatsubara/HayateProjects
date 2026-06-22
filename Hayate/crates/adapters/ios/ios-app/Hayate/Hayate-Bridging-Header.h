// Rust → Swift FFI for the hayate-adapter-ios staticlib (ADR-0114).
//
// These symbols are implemented in Rust (`src/app.rs`, `#[no_mangle] extern "C"`) and
// linked into the app from `libhayate_adapter_ios.a`. The reverse direction (Swift →
// Rust) is a single `@_cdecl("hayate_ios_set_keyboard_visible")` in HayateView.swift,
// declared `extern` on the Rust side in `ime_bridge.rs`.
#ifndef HAYATE_BRIDGING_HEADER_H
#define HAYATE_BRIDGING_HEADER_H

#include <stdint.h>

// One-time launch hook (logger init). Analogue of the head of Android's `android_main`.
void ios_main(void);

// Create the per-view app: builds the wgpu Metal surface from the CAMetalLayer and the
// demo ElementTree. `scale` is UIScreen.scale (Retina). Returns an opaque handle.
void *hayate_ios_app_new(void *metal_layer, float scale);
void hayate_ios_app_free(void *app);

// Drawable resized (points * scale = pixels). Reconfigures the surface + viewport.
void hayate_ios_resize(void *app, int32_t width, int32_t height, float scale);

// Single-pointer touch in view points. phase: 0=Down 1=Move 2=Up 3=Cancel.
void hayate_ios_touch(void *app, int32_t phase, float x, float y);

// UITextInput command. kind: 0=Insert 1=DeleteBackward 2=SetMarked 3=Unmark.
// `text` is UTF-8 (NULL for DeleteBackward/Unmark).
void hayate_ios_ime(void *app, int32_t kind, const char *text);

// Render + present one frame (CADisplayLink tick). `timestamp_ms` is a monotonic clock.
void hayate_ios_render(void *app, double timestamp_ms);

#endif /* HAYATE_BRIDGING_HEADER_H */
