//! 環境変数ゲート付き perf プローブ（Android/モバイル描画遅延診断のフィードバックループ）。
//!
//! Android/web で毎フレーム無条件に走る CPU 仕事を host で計測する:
//!   1. `tree.render`（コールド / アイドル / visual-dirty 1 要素）
//!   2. SceneGraph → vello `Scene` フルエンコード（present ごとに毎回走る）
//!   3. （wgpu アダプタがあれば）vello GPU render + readback
//!
//! 実行: HAYATE_PERF_PROBE=1 cargo test --release -p hayate-scene-renderer-vello \
//!        --test perf_probe -- --nocapture

use std::collections::HashSet;
use std::time::Instant;

use hayate_core::ElementId;
use hayate_demo_fixtures::{tasks_tree, TASKS_VIEWPORT};
use hayate_layer_compositor::layer_scene::{
    collect_layer_placements, extract_layer_scene, extract_root_scene,
};
use hayate_scene_renderer_vello::debug_encode_scene;

fn ms(d: std::time::Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn bench<F: FnMut()>(label: &str, iters: u32, mut f: F) {
    // ウォームアップ 3 回
    for _ in 0..3 {
        f();
    }
    let mut samples = Vec::with_capacity(iters as usize);
    for _ in 0..iters {
        let t = Instant::now();
        f();
        samples.push(ms(t.elapsed()));
    }
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p50 = samples[samples.len() / 2];
    let p95 = samples[((samples.len() as f64 * 0.95) as usize).min(samples.len() - 1)];
    let min = samples[0];
    println!("[perf-probe] {label:<44} p50 {p50:8.3}ms  p95 {p95:8.3}ms  min {min:8.3}ms");
}

#[test]
fn perf_probe() {
    if std::env::var_os("HAYATE_PERF_PROBE").is_none() {
        return;
    }
    let (vw, vh) = TASKS_VIEWPORT;
    println!("[perf-probe] fixture viewport {vw}x{vh} (logical px)");

    // ── 1. tree.render ────────────────────────────────────────────────────────
    let mut tree = tasks_tree("vello");
    let t = Instant::now();
    let node_count = tree.render(0.0).iter().count();
    println!(
        "[perf-probe] tree.render COLD: {:.3}ms (scene nodes {node_count})",
        ms(t.elapsed())
    );

    let mut ts = 16.0f64;
    bench("tree.render idle (no mutation)", 200, || {
        ts += 16.0;
        let _ = tree.render(ts);
    });

    // ── 2. vello Scene フルエンコード（present ごとに毎回走る）────────────────
    let graph = tree.render(ts + 16.0).clone();
    bench("vello Scene full encode scale=1.0", 100, || {
        let s = debug_encode_scene(&graph, 1.0);
        std::hint::black_box(&s);
    });
    bench("vello Scene full encode scale=3.0", 100, || {
        let s = debug_encode_scene(&graph, 3.0);
        std::hint::black_box(&s);
    });

    // ── #636: composite-only フレームの CPU 仕事 ────────────────────────────────
    // 配線前は上の full encode が毎フレーム走る（present ごと）。配線後、キャッシュ texture を
    // 再利用する composite-only フレームでは vello エンコードは走らず、CPU 仕事は placement 収集
    // （保持シーンから quad 配置を導出）だけ。差がスクロール/transform フレームの短縮分。
    let boundaries: HashSet<ElementId> = tree.frame_layers().iter().copied().collect();
    let root = tree.frame_layers()[0];
    bench("layer placements collect (composite-only frame CPU)", 200, || {
        let p = collect_layer_placements(&graph, root, &boundaries);
        std::hint::black_box(&p);
    });
    // dirty レイヤ 1 枚だけを再 raster するフレームのエンコードコスト（full encode との対比）。
    let layer_scene = if tree.frame_layers().len() > 1 {
        extract_layer_scene(&graph, tree.frame_layers()[1], &boundaries)
    } else {
        Some(extract_root_scene(&graph, root, &boundaries))
    }
    .unwrap_or_else(|| extract_root_scene(&graph, root, &boundaries));
    bench("vello single-layer encode scale=1.0 (dirty-layer reraster)", 100, || {
        let s = debug_encode_scene(&layer_scene, 1.0);
        std::hint::black_box(&s);
    });

    // ── 3. GPU render（アダプタがあれば。llvmpipe なら CPU 実行だが per-frame の
    //      パイプライン異常（atlas 肥大・バッファ churn・フレーム毎の成長）を検出できる）──
    match hayate_scene_test_support::vello::try_vello_harness() {
        None => println!("[perf-probe] wgpu adapter なし → GPU render 計測はスキップ"),
        Some(mut h) => {
            for scale in [1.0f32, 3.0] {
                let w = (vw * scale) as u32;
                let hgt = (vh * scale) as u32;
                bench(
                    &format!("vello full render+readback {w}x{hgt} (scale {scale})"),
                    10,
                    || {
                        let px = hayate_scene_test_support::vello::render_scene_to_pixels_scaled(
                            &mut h, &graph, w, hgt, scale,
                        );
                        assert!(px.is_some(), "vello render failed");
                    },
                );
            }
            // フレーム毎の成長検出：同一 renderer で 40 フレーム回し、前半/後半の平均を比較。
            let w = vw as u32;
            let hgt = vh as u32;
            let mut times = Vec::new();
            for _ in 0..40 {
                let t = Instant::now();
                let px = hayate_scene_test_support::vello::render_scene_to_pixels_scaled(
                    &mut h, &graph, w, hgt, 1.0,
                );
                assert!(px.is_some());
                times.push(ms(t.elapsed()));
            }
            let first: f64 = times[..20].iter().sum::<f64>() / 20.0;
            let last: f64 = times[20..].iter().sum::<f64>() / 20.0;
            println!("[perf-probe] 40 frames same renderer: avg first-half {first:.3}ms / second-half {last:.3}ms");
        }
    }
}
