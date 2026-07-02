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
    // present は planner の per-layer 計画を通してから raster する（#632/#633）。
    assert!(
        src.contains("PresentPlanner"),
        "GpuSurface must own a hayate_layer_compositor::PresentPlanner (#632)"
    );
    assert!(
        src.contains("plan_layers("),
        "render_frame must plan per-layer rasters via plan_layers (#633)"
    );
    assert!(
        src.contains("note_layer_rasterized"),
        "each completed layer raster must be recorded (sized, for the GPU budget slice)"
    );
}

#[test]
fn present_composites_cached_layer_textures_with_a_dedicated_compositor() {
    // 合成は専用 wgpu compositor（vello を使わない・ADR-0125 Decision 4）。placement
    // （transform/clip）は保持シーンから毎フレーム導出し、transform のみのフレームは
    // raster ゼロで quad 合成だけになる（#633）。
    let src = app_src();
    assert!(
        src.contains("WgpuQuadCompositor") && src.contains(".composite("),
        "present must composite cached layer textures with the dedicated wgpu compositor (#633)"
    );
    assert!(
        src.contains("collect_layer_placements") && src.contains("extract_root_scene")
            && src.contains("extract_layer_scene"),
        "layer decomposition must come from hayate_layer_compositor::layer_scene (#633)"
    );
}

#[test]
fn compositor_pipelines_are_warmed_up_at_init() {
    // ADR-0130a: init 時に全パイプライン variant を前倒し生成し、初回合成の遅延生成スパイクを消す。
    let src = app_src();
    assert!(
        src.contains(".warmup()"),
        "init_gpu_surface must warm up all compositor pipeline variants (ADR-0130a)"
    );
}

#[test]
fn present_consumes_core_captured_frame_layers() {
    // 判定入力は core が render() 内で捕捉した frame_layers / frame_layer_dirty。render 前の
    // スナップショットではカーソル点滅等の in-render 継続を取りこぼす（stale frame になる）。
    // #635 で present は handoff を組む `frame_handoff` に集約された（scene の owned スナップショット
    // ＋捕捉レイヤ）。app.rs がその捕捉を握り、両経路が frame_handoff を通す。
    let src = app_src();
    assert!(
        src.contains("frame_layers()") && src.contains("frame_layer_dirty()"),
        "the present path must consume tree.frame_layers()/frame_layer_dirty() (#632)"
    );
    let tsubame = read_relative("src/app_tsubame.rs");
    assert!(
        tsubame.contains("frame_handoff("),
        "the tsubame-js present path must build a handoff from the captured frame layers (#635)"
    );
}

#[test]
fn present_runs_raster_on_a_dedicated_thread() {
    // #635/ADR-0128: raster/composite は専用 Raster スレッドが所有する GpuSurface で走り、UI
    // スレッドは owned handoff を送るだけ（raster 完了を待たない）。両経路が RasterThread へ配線される。
    let src = app_src();
    assert!(
        src.contains("RasterThread::spawn") && src.contains("RasterCommand::Frame"),
        "app.rs must move the surface onto a RasterThread and present via RasterCommand::Frame (#635)"
    );
    assert!(
        src.contains("scene: tree.scene_graph().clone()"),
        "the handoff must carry an owned SceneGraph snapshot across the thread boundary (#635)"
    );
    for (name, path) in [("app.rs", "src/app.rs"), ("app_tsubame.rs", "src/app_tsubame.rs")] {
        let s = read_relative(path);
        assert!(
            s.contains("spawn_raster_thread(") && s.contains(".send("),
            "{name} present loop must produce onto the Raster thread (non-blocking, ADR-0128) (#635)"
        );
    }
}

#[test]
fn surface_teardown_stops_the_raster_thread() {
    // surface 破棄（TerminateWindow）で Raster スレッドを安全に停止する＝ハンドルを None にして
    // drop → 送信済みを処理して join。両経路で DestroySurface が raster を畳む。
    for (name, path) in [("app.rs", "src/app.rs"), ("app_tsubame.rs", "src/app_tsubame.rs")] {
        let s = read_relative(path);
        assert!(
            s.contains("DestroySurface => raster = None"),
            "{name} must stop the Raster thread on surface teardown (#635)"
        );
    }
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
    // 全面 `render_scene` は present 経路から消えた（レイヤ raster は rasterizer の中で
    // plan_layers 配下のみ）。直接呼び出しが復活したら raster gating の迂回。
    let src = app_src();
    assert_eq!(
        src.matches(".render_scene(").count(),
        0,
        "app.rs must not call render_scene directly (rasters go through the layer rasterizer)"
    );
}
