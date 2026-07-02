//! present 経路の FramePlan 駆動配線契約（#632・ADR-0125 backend 半分の入口）。
//!
//! raster gating の判定ロジック（`PresentPlanner` → `LayerCache::plan_raster` → `FramePlan`）と
//! work-count 契約（clean フレーム raster 0 回 / dirty フレーム 1 回）は `hayate-layer-compositor`
//! のホストテストで緑。一方 `app.rs` / `app_tsubame.rs` の実ループは device 専用でホストには
//! コンパイルされない（ADR-0112）。そこで reload_wiring.rs と同じく、ソースを読んで「present が
//! 毎フレーム FramePlan を通し、無条件 `render_scene` が残っていない」配線を固定する。

use std::fs;
use std::path::PathBuf;

fn read_relative(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn app_src() -> String {
    read_relative("src/app.rs")
}

#[test]
fn present_gates_raster_behind_a_frame_plan() {
    let src = app_src();
    // present は planner の FramePlan を通してから raster する（#632）。
    assert!(
        src.contains("PresentPlanner"),
        "GpuSurface must own a hayate_layer_compositor::PresentPlanner (#632)"
    );
    assert!(
        src.contains(".plan(") && src.contains("needs_raster"),
        "render_frame must consult plan(...).needs_raster before invoking render_scene"
    );
    assert!(
        src.contains("note_full_raster"),
        "a completed full raster must be recorded so clean frames become composite-only"
    );
}

#[test]
fn present_consumes_core_captured_frame_layers() {
    // 判定入力は core が render() 内で捕捉した frame_layers / frame_layer_dirty。render 前の
    // スナップショットではカーソル点滅等の in-render 継続を取りこぼす（stale frame になる）。
    let src = app_src();
    assert!(
        src.contains("frame_layers()") && src.contains("frame_layer_dirty()"),
        "the present path must consume tree.frame_layers()/frame_layer_dirty() (#632)"
    );
    let tsubame = read_relative("src/app_tsubame.rs");
    assert!(
        tsubame.contains("frame_layers()") && tsubame.contains("frame_layer_dirty()"),
        "the tsubame-js present path must consume the captured frame layers too (#632)"
    );
}

#[test]
fn resize_invalidates_the_cached_target() {
    // resize は target_view を作り直す＝キャッシュ面は失われた。invalidate しないと clean フレームが
    // 古いサイズの内容を blit し続ける。
    let src = app_src();
    assert!(
        src.contains("invalidate()"),
        "GpuSurface::resize must invalidate the present planner cache (#632)"
    );
}

#[test]
fn no_unconditional_render_scene_remains_in_present() {
    // `render_scene` の呼び出しは render_frame 内の needs_raster 分岐配下の 1 箇所だけ。
    let src = app_src();
    let calls = src.matches(".render_scene(").count();
    assert_eq!(
        calls, 1,
        "app.rs must call render_scene exactly once (inside the needs_raster branch)"
    );
}
