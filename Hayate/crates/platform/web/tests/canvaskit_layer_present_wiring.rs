//! CanvasKit dirty-layer present wiring contract (#834).
//!
//! The backend is wasm-only, so host CI fixes its observable planning/bridge path by source wiring;
//! the planner behavior is covered by `hayate-layer-compositor`, and bridge replay/composite behavior
//! is covered by `@torimi/hayate-host` tests plus the real-Chromium performance harness.

use std::fs;
use std::path::PathBuf;

fn backend_src() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/backend/canvaskit.rs");
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

#[test]
fn canvaskit_uses_present_planner_and_dirty_layer_bridge_methods() {
    let src = backend_src();
    assert!(src.contains("planner: PresentPlanner"));
    assert!(src.contains("plan_layers(&non_scroll_layers, layer_dirty)"));
    assert!(src.contains("scroll_layer_needs_raster"));
    assert!(src.contains("note_scroll_rasterized"));
    assert!(src.contains("geometry.screen_top_for_band(cached_band)"));
    assert!(src.contains("extract_root_scene") && src.contains("extract_layer_scene"));
    assert!(src.contains("REPLAY_LAYER_METHOD") && src.contains("COMPOSITE_LAYERS_METHOD"));
    assert!(src.contains("fn supports_layer_present(&self) -> bool"));
    assert!(src.contains("self.layer_present_enabled"));
}

#[test]
fn resize_and_topology_changes_invalidate_canvaskit_layer_state() {
    let src = backend_src();
    assert!(src.contains("self.planner.invalidate()"));
    assert!(src.contains("self.layer_payloads.remove(&stale)"));
    assert!(src.contains("self.prev_layers = boundaries.clone()"));
}
