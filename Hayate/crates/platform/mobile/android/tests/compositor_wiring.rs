//! Android retained Layer Presentation wiring contract.
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
fn vello_present_uses_the_shared_retained_transaction() {
    let src = app_src();
    assert!(
        src.contains("LayerPresentation") && src.contains("LayerPresentationFrame"),
        "GpuSurface must own the shared retained presentation transaction"
    );
    assert!(
        src.contains("VelloLayerRasterizer") && src.contains("WgpuQuadCompositor"),
        "Android Vello must consume LayerScene through the common raster/composite interface"
    );
    assert!(
        !src.contains("PresentPlanner") && !src.contains("note_full_raster"),
        "the retired full-frame ledger must not survive the cutover"
    );
}

#[test]
fn present_has_no_copied_extraction_or_full_scene_fallback() {
    let src = app_src();
    for gone in [
        "plan_layers(",
        "collect_layer_placements",
        "extract_layer_scene",
        "extract_root_scene",
        "prev_layers",
        "render_scene_with_offset(",
    ] {
        assert!(
            !src.contains(gone),
            "app.rs must no longer reference the retired per-layer path: {gone} (#687)"
        );
    }
}

#[test]
fn present_applies_safe_area_at_composite_time() {
    let src = app_src();
    assert!(
        src.contains("compose(translation, plane.transform)")
            && src.contains("x + self.scene_origin.0"),
        "safe-area origin must move retained placements and clips together"
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
        src.contains("frame_handoff(frame: &CommittedFrame)")
            && src.contains("RasterHandoff::from_committed_frame(frame)"),
        "the present path must capture scene, layer order, dirty sets, and scroll facts from one Core commit (#855)"
    );
    let tsubame = read_relative("src/app_tsubame.rs");
    assert!(
        tsubame.contains("frame_handoff("),
        "the tsubame-js present path must build a handoff from the captured frame layers (#635)"
    );
}

#[test]
fn both_android_skia_surfaces_enable_the_shared_layer_presenter() {
    for (name, path) in [
        ("CPU raster", "src/skia_window.rs"),
        ("Ganesh/EGL GL", "src/skia_gl_window.rs"),
    ] {
        let src = read_relative(path);
        assert!(
            src.contains("SkiaLayerPresenter")
                && (src.contains(".present(")
                    || src.contains(".present_with_layer_surface_factory(")),
            "{name} skia-safe path must use the shared per-layer presenter"
        );
        assert!(
            src.contains("LayerTopology"),
            "{name} skia-safe path must consume Core layer topology"
        );
        assert!(
            src.contains("scroll_layer_geometry_from_inputs"),
            "{name} skia-safe path must reuse scroll overscan geometry"
        );
    }
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
        !src.contains("scene: tree.scene_graph().clone()")
            && src.contains("RasterHandoff::from_committed_frame"),
        "native handoff must freeze one CommittedFrame without deep-cloning the SceneGraph (#855)"
    );
    // issue #802/#804: surface 初期化とスレッド起動の選択(skia→vello の Renderer Selection Policy
    // loop)は `init_and_spawn_raster`(app.rs)に一本化された。両スポナー自体は app.rs が持つ。
    assert!(
        src.contains("spawn_raster_thread(") && src.contains("spawn_skia_raster_thread("),
        "app.rs must own both raster-thread spawners (vello and skia, issue #802) behind the \
         Renderer Selection Policy loop in init_and_spawn_raster"
    );
    for (name, path) in [
        ("app.rs", "src/app.rs"),
        ("app_tsubame.rs", "src/app_tsubame.rs"),
    ] {
        let s = read_relative(path);
        assert!(
            s.contains("init_and_spawn_raster(")
                && s.contains(".send(")
                && s.contains("frame_handoff(&frame)"),
            "{name} present loop must produce onto the Raster thread via the shared Renderer \
             Selection Policy entry point from the same committed-frame interface (non-blocking, \
             ADR-0128; issue #855)"
        );
    }
}

#[test]
fn surface_teardown_stops_the_raster_thread() {
    // surface 破棄（TerminateWindow）で Raster スレッドを安全に停止する＝ハンドルを None にして
    // drop → 送信済みを処理して join。両経路で DestroySurface が raster を畳む。
    for (name, path) in [
        ("app.rs", "src/app.rs"),
        ("app_tsubame.rs", "src/app_tsubame.rs"),
    ] {
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
