//! present 経路の FramePlan 駆動配線契約（#632・#687 で per-layer レイヤキャッシュ経路を撤去し
//! Web 現行実装（全面 Vello 再描画）と同型の単一 root 経路に揃えた）。
//!
//! raster gating の判定ロジック（`PresentPlanner` → `LayerCache::plan_raster` → `FramePlan`）と
//! work-count 契約（clean フレーム raster 0 回 / dirty フレーム 1 回）は `hayate-layer-compositor`
//! のホストテストで緑。一方 `app.rs` / `app_tsubame.rs` の実ループは device 専用でホストには
//! コンパイルされない（ADR-0112）。そこで reload_wiring.rs と同じく、ソースを読んで「present が
//! 毎フレーム FramePlan を通し、単一 root の `render_scene` 1 回だけで present する」配線を固定する。

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
    // present は単一 root の FramePlan を通してから raster する（#632、#687 で plan_layers から
    // Web と同じ plan に揃えた）。
    assert!(
        src.contains("PresentPlanner"),
        "GpuSurface must own a hayate_layer_compositor::PresentPlanner (#632)"
    );
    assert!(
        src.contains(".plan(") && src.contains("needs_raster"),
        "render_frame must consult plan(...).needs_raster before rendering the scene (#687)"
    );
    assert!(
        src.contains("note_full_raster"),
        "a completed full-scene raster must be recorded so clean frames become composite-only (#687)"
    );
    // transform 係数だけの変化（#633）も、単一 root 経路では quad 合成が無いため保守的に
    // raster トリガへ含める必要がある（Web の canvas.rs と同じ理由、#687）。
    assert!(
        src.contains("frame_layer_transform_dirty"),
        "the single-root present path must union transform_dirty into its raster trigger (#687)"
    );
}

#[test]
fn present_no_longer_uses_the_per_layer_cache_path() {
    // #687: per-layer レイヤキャッシュ＋専用 wgpu compositor 経路は Android から撤去し、Web の
    // 現行実装（毎 dirty フレーム全面再描画）に揃える。撤去対象の実装コード自体は
    // hayate-scene-renderer-vello / hayate-layer-compositor 側に残すが、app.rs はもう呼ばない。
    let src = app_src();
    for gone in [
        "VelloLayerRasterizer",
        "WgpuQuadCompositor",
        "plan_layers(",
        "GpuBudget",
        "collect_layer_placements",
        "extract_layer_scene",
        "extract_root_scene",
        "prev_layers",
    ] {
        assert!(
            !src.contains(gone),
            "app.rs must no longer reference the retired per-layer path: {gone} (#687)"
        );
    }
}

#[test]
fn present_renders_the_whole_scene_with_the_vello_scene_renderer() {
    // #687: 単一 root 経路は Web の SelectedBackend/VelloSurfaceHost と同型——オフスクリーン
    // target_view へ VelloSceneRenderer::render_scene を 1 回呼び、TextureBlitter でサーフェスへ blit する。
    let src = app_src();
    assert!(
        src.contains("VelloSceneRenderer"),
        "GpuSurface must own a VelloSceneRenderer, matching the web backend (#687)"
    );
    assert!(
        src.contains("create_target_view") && src.contains("create_blitter"),
        "GpuSurface must build its offscreen target_view/blitter the same way the web backend does (#687)"
    );
    // b2（edge-to-edge, issue #794・ADR-0144）: 全画面 raster をシーンの安全領域平行移動込みで
    // 1 回だけ呼ぶ（`render_scene_with_offset`。インセット push が無いフレームは offset 0）。
    assert_eq!(
        src.matches(".render_scene_with_offset(").count(),
        1,
        "app.rs must raster the whole scene exactly once via render_scene_with_offset \
         (needs_raster branch, #687 + b2 safe-area shift #794)"
    );
}

#[test]
fn compositor_pipelines_are_warmed_up_at_init() {
    // ADR-0130a: init 時にパイプラインを前倒し生成し、初回フレームの遅延生成スパイクを消す。
    let src = app_src();
    assert!(
        src.contains(".warmup("),
        "init_gpu_surface must warm up the vello scene renderer (ADR-0130a)"
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
    assert!(
        src.contains("frame_layer_transform_dirty()"),
        "frame_handoff must also carry tree.frame_layer_transform_dirty() (#687)"
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
    // issue #802: surface 初期化とスレッド起動の選択(vello→skia の Renderer Selection Policy
    // loop)は `init_and_spawn_raster`(app.rs)に一本化された。両スポナー自体は app.rs が持つ。
    assert!(
        src.contains("spawn_raster_thread(") && src.contains("spawn_skia_raster_thread("),
        "app.rs must own both raster-thread spawners (vello and skia, issue #802) behind the \
         Renderer Selection Policy loop in init_and_spawn_raster"
    );
    for (name, path) in [("app.rs", "src/app.rs"), ("app_tsubame.rs", "src/app_tsubame.rs")] {
        let s = read_relative(path);
        assert!(
            s.contains("init_and_spawn_raster(") && s.contains(".send("),
            "{name} present loop must produce onto the Raster thread via the shared Renderer \
             Selection Policy entry point (non-blocking, ADR-0128; issue #802 centralizes vello/skia \
             surface init behind init_and_spawn_raster)"
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
