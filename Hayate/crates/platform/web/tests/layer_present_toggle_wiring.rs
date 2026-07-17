//! tiny-skia の per-layer 比較用トグル配線契約（ADR-0138・#710）。
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
fn canvas_init_takes_and_forwards_the_runtime_toggle() {
    let src = read("src/canvas.rs");
    assert!(
        src.contains("layer_present_enabled: Option<bool>"),
        "HayateElementRenderer::init must accept an Option<bool> layer_present_enabled param"
    );
    assert!(
        src.contains("backend.set_layer_present_enabled(layer_present_enabled.unwrap_or(true))"),
        "init must forward the flag to the backend, defaulting to ON when unset (ADR-0138)"
    );
}

#[test]
fn tiny_skia_backend_reads_a_runtime_field_instead_of_a_hardcoded_true() {
    let src = read("src/backend/tiny_skia_backend.rs");
    assert!(
        src.contains("layer_present_enabled: bool"),
        "SelectedBackend must own a settable layer_present_enabled field"
    );
    assert!(
        src.contains(
            "fn supports_layer_present(&self) -> bool {\n        self.layer_present_enabled\n    }"
        ),
        "supports_layer_present must read the runtime field, not return a hardcoded true"
    );
    assert!(
        src.contains("fn set_layer_present_enabled(&mut self, enabled: bool) {\n        self.layer_present_enabled = enabled;\n    }"),
        "SelectedBackend must implement the SceneRenderer::set_layer_present_enabled setter"
    );
}
