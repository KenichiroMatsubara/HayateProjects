//! web Canvas Mode retained Layer Presentation wiring contract.
//!
//! raster gating 判定と work-count 契約は `hayate-layer-compositor` のホストテストで緑。
//! `canvas.rs` は wasm 専用でホストにはコンパイルされないため、Android アダプタの
//! wiring テスト群と同じくソースを読んで配線を固定する（実描画は parity / e2e が守る）。

use std::fs;
use std::path::PathBuf;

fn canvas_src() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/canvas.rs");
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn vello_backend_src() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/backend/vello.rs");
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

#[test]
fn render_submits_the_owned_snapshot_and_topology() {
    let src = canvas_src();
    assert!(
        src.contains("frame.snapshot()") && src.contains("frame.layer_topology()"),
        "the present seam must consume one committed snapshot and topology"
    );
    assert!(
        src.contains(".present_layers(") && !src.contains("supports_layer_present"),
        "Layer Presentation must be unconditional"
    );
    assert!(
        !src.contains("PresentPlanner") && !src.contains("note_full_raster"),
        "the retired full-frame planner path must be absent"
    );
}

#[test]
fn resize_is_owned_by_the_backend_presentation() {
    let src = canvas_src();
    assert!(
        !src.contains("planner.invalidate()"),
        "canvas must not retain a second logical cache ledger"
    );
}

#[test]
fn no_full_scene_fallback_remains() {
    let src = canvas_src();
    assert_eq!(src.matches("backend.render_scene(").count(), 0);
}

#[test]
fn web_vello_gates_scroll_chrome_with_the_committed_dirty_fact() {
    let canvas = canvas_src();
    assert!(
        canvas.contains("frame.layer_topology()"),
        "CommittedFrame topology must carry chrome dirty across the seam"
    );

    let vello = vello_backend_src();
    assert!(
        vello.contains("topology") && vello.contains("ScrollChrome"),
        "Web Vello must gate chrome raster by committed dirty state and cache state"
    );
    assert!(
        !vello.contains(".rasterize_scroll_chrome("),
        "Web Vello must not unconditionally raster scroll chrome"
    );
}

#[test]
fn web_vello_consumes_committed_layer_raster_bounds() {
    let canvas = canvas_src();
    assert!(
        canvas.contains("frame.layer_topology()"),
        "CommittedFrame topology must carry raster bounds across the seam"
    );

    let vello = vello_backend_src();
    assert!(
        vello.contains("rasterize_in_bounds") && vello.contains("update_scroll_chrome_in_bounds"),
        "Web Vello must apply Core bounds to content and scroll chrome textures"
    );
}
