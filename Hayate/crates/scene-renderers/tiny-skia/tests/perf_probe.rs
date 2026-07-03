//! 環境変数ゲート付き perf プローブ（Android/モバイル描画遅延診断のフィードバックループ）。
//!
//! web CPU モード（Android Chrome の `?renderer=tiny-skia`）で毎フレーム無条件に走る
//! 仕事を host ネイティブで計測する（実機 WASM はさらに 2〜4 倍遅い前提で読む）:
//!   1. tiny-skia `render_scene` 全面ラスタ（scale 1.0 / 2.0 / 3.0 ≒ DPR）
//!   2. blit 前処理: `pixmap.data().to_vec()` + `premultiplied_to_straight`
//!
//! 実行: HAYATE_PERF_PROBE=1 cargo test --release -p hayate-scene-renderer-tiny-skia \
//!        --test perf_probe -- --nocapture

use std::collections::HashSet;
use std::time::Instant;

use hayate_core::ElementId;
use hayate_demo_fixtures::{tasks_tree, TASKS_VIEWPORT};
use hayate_layer_compositor::layer_scene::{
    collect_layer_placements, extract_layer_scene, extract_root_scene,
};
use hayate_layer_compositor::{CompositeQuad, LayerCompositor, LayerRasterizer};
use hayate_scene_renderer_tiny_skia::{
    premultiplied_to_straight, TinySkiaCompositeTarget, TinySkiaLayerCompositor,
    TinySkiaLayerRasterizer, TinySkiaSceneRenderer,
};
use tiny_skia::Pixmap;

fn ms(d: std::time::Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn bench<F: FnMut()>(label: &str, iters: u32, mut f: F) {
    for _ in 0..2 {
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
    println!("[perf-probe] {label:<52} p50 {p50:8.3}ms  p95 {p95:8.3}ms  min {min:8.3}ms");
}

#[test]
fn perf_probe() {
    if std::env::var_os("HAYATE_PERF_PROBE").is_none() {
        return;
    }
    let (vw, vh) = TASKS_VIEWPORT;
    let mut tree = tasks_tree("tiny-skia");
    let graph = tree.render(0.0).clone();
    let (mut rects, mut rings, mut texts, mut glyphs, mut groups, mut clips, mut images, mut anchors, mut dashed, mut blurred) =
        (0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
    for (_, node) in graph.iter() {
        use hayate_core::NodeKind::*;
        match &node.kind {
            Rect { .. } => rects += 1,
            RoundedRing { .. } => rings += 1,
            BlurredRoundedRect { .. } => blurred += 1,
            DashedBorder { .. } => dashed += 1,
            TextRun { data, .. } => {
                texts += 1;
                glyphs += data.glyphs.len();
            }
            Group { .. } => groups += 1,
            Clip { .. } => clips += 1,
            Image { .. } => images += 1,
            ElementAnchor { .. } => anchors += 1,
        }
    }
    println!(
        "[perf-probe] fixture {vw}x{vh}: nodes {} (rect {rects}, ring {rings}, blurred {blurred}, dashed {dashed}, textrun {texts} / glyphs {glyphs}, group {groups}, clip {clips}, image {images}, anchor {anchors})",
        graph.iter().count()
    );
    let rect_area: f64 = graph
        .iter()
        .filter_map(|(_, n)| match &n.kind {
            hayate_core::NodeKind::Rect { width, height, .. } => {
                Some(*width as f64 * *height as f64)
            }
            _ => None,
        })
        .sum();
    println!(
        "[perf-probe] rect fill area total = {:.1} viewports (overdraw factor)",
        rect_area / (vw as f64 * vh as f64)
    );

    // テキスト run のグリフを空にしたバリアントで、グリフラスタのコストを切り分ける。
    let mut no_text = graph.clone();
    let ids: Vec<_> = no_text.iter().map(|(id, _)| id).collect();
    for id in ids {
        if let Some(node) = no_text.get_mut(id) {
            if let hayate_core::NodeKind::TextRun { data, .. } = &mut node.kind {
                let mut d = (**data).clone();
                d.glyphs.clear();
                d.decorations.clear();
                *data = std::sync::Arc::new(d);
            }
        }
    }
    {
        let w = vw as u32;
        let h = vh as u32;
        let mut pixmap = Pixmap::new(w, h).expect("pixmap");
        let mut renderer = TinySkiaSceneRenderer::new();
        bench("render_scene NO-TEXT 980x1060 (scale 1)", 20, || {
            renderer.render_scene(&no_text, &mut pixmap, [1.0, 1.0, 1.0, 1.0], 1.0);
        });
    }

    for scale in [1.0f32, 2.0, 3.0] {
        let w = (vw * scale) as u32;
        let h = (vh * scale) as u32;
        let mut pixmap = Pixmap::new(w, h).expect("pixmap");
        let mut renderer = TinySkiaSceneRenderer::new();
        bench(
            &format!("tiny-skia render_scene {w}x{h} (scale {scale})"),
            20,
            || {
                renderer.render_scene(&graph, &mut pixmap, [1.0, 1.0, 1.0, 1.0], scale);
            },
        );
        bench(
            &format!("blit copy+unpremultiply {w}x{h}"),
            20,
            || {
                let mut data = pixmap.data().to_vec();
                premultiplied_to_straight(&mut data);
                std::hint::black_box(&data);
            },
        );
    }

    // #636: composite-only フレーム（スクロール/transform）の CPU コストを全面ラスタと並べて計測する。
    // 配線前は毎フレーム全面 render_scene（上の scale ループ）。配線後はキャッシュ Pixmap の draw_pixmap
    // 合成だけ。差がそのままスクロール中フレームの短縮分（診断 原因 2 の効き）。
    for scale in [1.0f32, 2.0, 3.0] {
        let w = (vw * scale) as u32;
        let h = (vh * scale) as u32;
        let boundaries: HashSet<ElementId> = tree.frame_layers().iter().copied().collect();
        let root = tree.frame_layers()[0];
        // 全レイヤを一度 raster してキャッシュを温める（composite-only フレームの前提）。
        let mut rasterizer = TinySkiaLayerRasterizer::new(w, h, scale);
        for &layer in tree.frame_layers() {
            let scene = if layer == root {
                extract_root_scene(&graph, root, &boundaries)
            } else {
                match extract_layer_scene(&graph, layer, &boundaries) {
                    Some(s) => s,
                    None => continue,
                }
            };
            rasterizer.rasterize(layer, &scene).unwrap();
        }
        let placements = collect_layer_placements(&graph, root, &boundaries);
        let mut compositor = TinySkiaLayerCompositor::new(scale);
        let mut target = TinySkiaCompositeTarget {
            pixmap: Pixmap::new(w, h).expect("pixmap"),
            clear: [1.0, 1.0, 1.0, 1.0],
        };
        bench(
            &format!("tiny-skia COMPOSITE-ONLY {w}x{h} (scale {scale}, {} layers)", placements.len()),
            20,
            || {
                let quads: Vec<CompositeQuad<'_, Pixmap>> = placements
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
                compositor.composite(&mut target, &quads).unwrap();
                std::hint::black_box(&target.pixmap);
            },
        );
    }
}
