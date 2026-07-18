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

use hayate_core::{ElementId, LayerRasterBounds};
use hayate_demo_fixtures::{
    dual_transition_tree, tasks_tree, DUAL_TRANSITION_VIEWPORT, TASKS_VIEWPORT,
};
use hayate_layer_compositor::layer_scene::{
    collect_layer_placements, extract_layer_scene, extract_root_scene,
};
use hayate_layer_compositor::{CompositeQuad, LayerCompositor, LayerRasterizer};
use hayate_scene_renderer_vello::debug_encode_scene;
use hayate_scene_renderer_vello::layer_compositor::{
    CompositeTarget, VelloLayerRasterizer, WgpuQuadCompositor,
};
use hayate_scene_test_support::vello::{
    readback_rgba8, render_scene_to_pixels_scaled, VelloHarness,
};

/// #690/#692: 1 フレーム分のレイヤ raster + wgpu quad 合成（persistent キャッシュ）。呼び出し側が
/// `cached`（既に raster 済みのレイヤ集合）を跨フレームで持ち回すことで、`present_layers()` の実運用
/// 挙動（dirty / 未キャッシュのレイヤだけ再 raster）を計測でも再現する。
fn layered_gpu_frame(
    h: &mut VelloHarness,
    rasterizer: &mut VelloLayerRasterizer,
    compositor: &mut WgpuQuadCompositor,
    cached: &mut HashSet<ElementId>,
    graph: &hayate_core::SceneGraph,
    layers: &[ElementId],
    layer_raster_bounds: &[LayerRasterBounds],
    layer_dirty: &HashSet<ElementId>,
    w: u32,
    hgt: u32,
) -> Option<Vec<u8>> {
    let Some(&root) = layers.first() else {
        return None;
    };
    let boundaries: HashSet<ElementId> = layers.iter().copied().collect();
    for &layer in layers {
        if cached.contains(&layer) && !layer_dirty.contains(&layer) {
            continue;
        }
        let extracted = if layer == root {
            Some(extract_root_scene(graph, root, &boundaries))
        } else {
            extract_layer_scene(graph, layer, &boundaries)
        };
        if let Some(extracted) = extracted {
            if layer == root {
                rasterizer.rasterize(layer, &extracted, None).ok()?;
            } else if let Some(&bounds) = layer_raster_bounds
                .iter()
                .find(|bounds| bounds.layer == layer)
            {
                rasterizer
                    .rasterize_in_bounds(layer, &extracted, bounds, None)
                    .ok()?;
            } else {
                rasterizer.rasterize(layer, &extracted, None).ok()?;
            }
            cached.insert(layer);
        }
    }
    let placements = collect_layer_placements(graph, root, &boundaries);
    let quads: Vec<CompositeQuad<'_, _>> = placements
        .iter()
        .filter_map(|p| {
            rasterizer.texture(p.layer).map(|texture| CompositeQuad {
                layer: p.layer,
                transform: p.transform,
                opacity: 1.0,
                clip: p.clip,
                texture,
            })
        })
        .collect();
    let target_texture = h.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("perf_probe_layered_target"),
        size: wgpu::Extent3d {
            width: w,
            height: hgt,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let mut target = CompositeTarget {
        view: target_texture.create_view(&wgpu::TextureViewDescriptor::default()),
        width: w,
        height: hgt,
        format: wgpu::TextureFormat::Rgba8Unorm,
        clear: [1.0, 1.0, 1.0, 1.0],
    };
    compositor.composite(&mut target, &quads).ok()?;
    readback_rgba8(&h.device, &h.queue, &target_texture, w, hgt)
}

fn percentiles(samples: &mut [f64]) -> (f64, f64) {
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p50 = samples[samples.len() / 2];
    let p95 = samples[((samples.len() as f64 * 0.95) as usize).min(samples.len() - 1)];
    (p50, p95)
}

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
    bench(
        "layer placements collect (composite-only frame CPU)",
        200,
        || {
            let p = collect_layer_placements(&graph, root, &boundaries);
            std::hint::black_box(&p);
        },
    );
    // dirty レイヤ 1 枚だけを再 raster するフレームのエンコードコスト（full encode との対比）。
    let layer_scene = if tree.frame_layers().len() > 1 {
        extract_layer_scene(&graph, tree.frame_layers()[1], &boundaries)
    } else {
        Some(extract_root_scene(&graph, root, &boundaries))
    }
    .unwrap_or_else(|| extract_root_scene(&graph, root, &boundaries));
    bench(
        "vello single-layer encode scale=1.0 (dirty-layer reraster)",
        100,
        || {
            let s = debug_encode_scene(&layer_scene, 1.0);
            std::hint::black_box(&s);
        },
    );

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

            // ── #692: 全面描画（feature layer-present OFF 相当）vs レイヤ raster+合成
            //    （ON 相当）の raster 所要時間比較。tasks_tree はレイヤが root 1 枚だけ
            //    （scroll コンテナ/transition が起きていない）ので、per-layer 経路の
            //    オーバーヘッドが単一レイヤの度数分布としてそのまま出る。
            let w = vw as u32;
            let hgt = vh as u32;
            let layers = tree.frame_layers().to_vec();
            let layer_raster_bounds = tree.committed_frame().layer_raster_bounds().to_vec();
            let dirty: HashSet<ElementId> = HashSet::new();
            let mut rasterizer =
                VelloLayerRasterizer::new(h.device.clone(), h.queue.clone(), w, hgt, 1.0).unwrap();
            let mut compositor = WgpuQuadCompositor::new(h.device.clone(), h.queue.clone());
            compositor.warmup();
            let mut cached = HashSet::new();
            let mut full_samples = Vec::new();
            let mut layered_samples = Vec::new();
            for _ in 0..10 {
                let t = Instant::now();
                let px = render_scene_to_pixels_scaled(&mut h, &graph, w, hgt, 1.0);
                assert!(px.is_some());
                full_samples.push(ms(t.elapsed()));

                let t = Instant::now();
                let px = layered_gpu_frame(
                    &mut h,
                    &mut rasterizer,
                    &mut compositor,
                    &mut cached,
                    &graph,
                    &layers,
                    &layer_raster_bounds,
                    &dirty,
                    w,
                    hgt,
                );
                assert!(px.is_some());
                layered_samples.push(ms(t.elapsed()));
            }
            let (full_p50, full_p95) = percentiles(&mut full_samples);
            let (layered_p50, layered_p95) = percentiles(&mut layered_samples);
            println!(
                "[perf-probe] tasks_tree {w}x{hgt} full-raster (OFF)   p50 {full_p50:8.3}ms  p95 {full_p95:8.3}ms"
            );
            println!(
                "[perf-probe] tasks_tree {w}x{hgt} layered-present (ON) p50 {layered_p50:8.3}ms  p95 {layered_p95:8.3}ms"
            );
        }
    }

    // ── #692: #680 型フィクスチャ（2 要素同時 transition）の全面描画 vs per-layer 比較 ──
    match hayate_scene_test_support::vello::try_vello_harness() {
        None => println!("[perf-probe] wgpu adapter なし → dual-transition GPU 計測はスキップ"),
        Some(mut h) => {
            let (dvw, dvh) = DUAL_TRANSITION_VIEWPORT;
            let w = dvw as u32;
            let hgt = dvh as u32;
            let mut dtree = dual_transition_tree();
            println!("[perf-probe] dual-transition fixture viewport {dvw}x{dvh} (logical px)");

            let mut rasterizer =
                VelloLayerRasterizer::new(h.device.clone(), h.queue.clone(), w, hgt, 1.0).unwrap();
            let mut compositor = WgpuQuadCompositor::new(h.device.clone(), h.queue.clone());
            compositor.warmup();
            let mut cached = HashSet::new();
            let mut full_samples = Vec::new();
            let mut layered_samples = Vec::new();
            let mut full_dispatch_px = 0u64;
            let mut bounded_dispatch_px = 0u64;

            // 実際の transition が続く限り、毎フレーム 2 要素とも dirty（背景色補間中）——
            // `present_layers()` が実運用で受け取る dirty 集合と同じものを都度計測に渡す。
            let mut t = 16.0;
            let mut frames = 0;
            while dtree.has_pending_visual_work() && frames < 60 {
                let graph = dtree.render(t).clone();
                let layers = dtree.frame_layers().to_vec();
                let layer_raster_bounds = dtree.committed_frame().layer_raster_bounds().to_vec();
                let dirty = dtree.frame_layer_dirty().clone();

                let bt = Instant::now();
                let px = render_scene_to_pixels_scaled(&mut h, &graph, w, hgt, 1.0);
                assert!(px.is_some());
                full_samples.push(ms(bt.elapsed()));

                let bt = Instant::now();
                let px = layered_gpu_frame(
                    &mut h,
                    &mut rasterizer,
                    &mut compositor,
                    &mut cached,
                    &graph,
                    &layers,
                    &layer_raster_bounds,
                    &dirty,
                    w,
                    hgt,
                );
                assert!(px.is_some());
                layered_samples.push(ms(bt.elapsed()));
                for &layer in &layers {
                    if !dirty.contains(&layer) {
                        continue;
                    }
                    full_dispatch_px += u64::from(w) * u64::from(hgt);
                    if let Some(texture) = rasterizer.texture(layer) {
                        bounded_dispatch_px += u64::from(texture.width) * u64::from(texture.height);
                    }
                }

                t += 16.0;
                frames += 1;
            }
            println!("[perf-probe] dual-transition animated for {frames} frames before settling");
            let actual_cache_bytes: u64 = cached
                .iter()
                .filter_map(|&layer| rasterizer.texture_bytes(layer))
                .sum();
            let full_cache_bytes = cached.len() as u64
                * u64::from(w)
                * u64::from(hgt)
                * hayate_layer_compositor::tunables::BYTES_PER_PIXEL;
            println!(
                "[perf-probe] layer bounds dispatch px {full_dispatch_px} -> {bounded_dispatch_px}; cache bytes {full_cache_bytes} -> {actual_cache_bytes}"
            );
            assert!(
                bounded_dispatch_px < full_dispatch_px,
                "Core layer bounds must reduce dirty-layer dispatch pixels"
            );
            assert!(
                actual_cache_bytes < full_cache_bytes,
                "Core layer bounds must reduce retained layer cache bytes"
            );
            let (full_p50, full_p95) = percentiles(&mut full_samples);
            let (layered_p50, layered_p95) = percentiles(&mut layered_samples);
            println!(
                "[perf-probe] dual-transition {w}x{hgt} full-raster (OFF)   p50 {full_p50:8.3}ms  p95 {full_p95:8.3}ms"
            );
            println!(
                "[perf-probe] dual-transition {w}x{hgt} layered-present (ON) p50 {layered_p50:8.3}ms  p95 {layered_p95:8.3}ms"
            );
        }
    }
}
