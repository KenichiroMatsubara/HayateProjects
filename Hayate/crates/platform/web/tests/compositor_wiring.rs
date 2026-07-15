//! web Canvas Mode present 経路の FramePlan 駆動配線契約（#632）。
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

#[test]
fn render_gates_raster_behind_a_frame_plan() {
    let src = canvas_src();
    assert!(
        src.contains("PresentPlanner"),
        "HayateElementRenderer must own a hayate_layer_compositor::PresentPlanner (#632)"
    );
    assert!(
        src.contains(".plan(") && src.contains("needs_raster"),
        "render() must consult plan(...).needs_raster before backend.render_scene"
    );
    assert!(
        src.contains("note_full_raster"),
        "a completed raster must be recorded so clean frames become composite-only"
    );
    assert!(
        src.contains("frame.layers()") && src.contains("frame.content_dirty_layers()"),
        "the present path must consume the CommittedFrame layer view (#824)"
    );
    // 単一 root 経路は per-layer quad 合成を持たないので、transform 係数だけの変化（#633 で
    // content dirty から分離された）も保守的に raster トリガへ含めないと stale frame になる。
    assert!(
        src.contains("frame.transform_dirty_layers()"),
        "the single-root present path must union committed transform dirty into its raster trigger (#633)"
    );
}

#[test]
fn resize_invalidates_the_cached_surface() {
    let src = canvas_src();
    assert!(
        src.contains("invalidate()"),
        "apply_resize must invalidate the present planner cache (#632)"
    );
}

#[test]
fn no_unconditional_render_scene_remains() {
    let src = canvas_src();
    let calls = src.matches(".render_scene(").count();
    assert_eq!(
        calls, 1,
        "canvas.rs must call backend.render_scene exactly once (inside the needs_raster branch)"
    );
}
