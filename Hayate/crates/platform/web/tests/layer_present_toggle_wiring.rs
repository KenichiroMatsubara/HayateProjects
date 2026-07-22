//! Retained Layer Presentation is a hard cutover: no runtime escape hatch remains.
//!
//! `canvas.rs`・`backend/tiny_skia_backend.rs` は wasm 専用で
//! ホストにはコンパイルされないため、`compositor_wiring.rs` と同じくソースを読んで配線を
//! 固定する（実描画は e2e が守る）。

use std::fs;
use std::path::PathBuf;

fn read(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

#[test]
fn canvas_init_has_no_runtime_layer_present_toggle() {
    let src = read("src/canvas.rs");
    assert!(
        !src.contains("layer_present_enabled") && !src.contains("set_layer_present_enabled"),
        "the production canvas API must not expose a legacy layer-present toggle"
    );
}

#[test]
fn production_web_backends_have_only_the_retained_present_path() {
    let canvas = read("src/canvas.rs");
    assert!(
        canvas.contains(".present_layers(") && !canvas.contains("supports_layer_present"),
        "canvas must unconditionally submit the committed snapshot and topology"
    );
    for backend in ["src/backend/vello.rs", "src/backend/tiny_skia_backend.rs"] {
        let src = read(backend);
        assert!(!src.contains("supports_layer_present"));
        assert!(!src.contains("layer_present_enabled"));
        assert!(src.contains("fn present_layers("));
    }
}
