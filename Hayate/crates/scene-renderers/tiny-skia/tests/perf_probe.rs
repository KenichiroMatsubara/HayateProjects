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

use hayate_core::{
    ElementId, LayerRasterBounds, Node, NodeId, NodeKind, SceneRead, SceneResources, SceneSnapshot,
};
use hayate_demo_fixtures::{tasks_tree, TASKS_VIEWPORT};
use hayate_layer_compositor::layer_scene::{
    collect_layer_placements, extract_layer_scene, extract_root_scene,
};
use hayate_layer_compositor::{CompositeQuad, LayerCompositor, LayerRasterizer};
use hayate_scene_renderer_tiny_skia::{
    premultiplied_to_straight, TinySkiaCompositeTarget, TinySkiaLayerCompositor,
    TinySkiaLayerRasterizer, TinySkiaLayerTexture, TinySkiaSceneRenderer,
};
use tiny_skia::Pixmap;

struct WithoutText<'a>(&'a SceneSnapshot);

impl SceneRead for WithoutText<'_> {
    fn get(&self, id: NodeId) -> Option<&Node> {
        self.0
            .get(id)
            .filter(|node| !matches!(node.kind, NodeKind::TextRun { .. }))
    }

    fn roots(&self) -> &[NodeId] {
        self.0.roots()
    }

    fn resources(&self) -> &SceneResources {
        self.0.resources()
    }
}

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
    // Promote the app bar into a representative non-root layer so this probe measures the
    // bounded-cache path rather than the fixture's otherwise single-root full-surface path.
    tree.element_set_transform(ElementId::from_u64(2), Some([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]));
    let _ = tree.render(0.0);
    let graph = tree.committed_frame().snapshot().clone();
    let (
        mut rects,
        mut rings,
        mut texts,
        mut glyphs,
        mut groups,
        mut clips,
        mut images,
        mut anchors,
        mut dashed,
        mut blurred,
    ) = (
        0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0usize,
    );
    for (_, node) in graph.iter() {
        use hayate_core::NodeKind::*;
        match &node.kind {
            Rect { .. } => rects += 1,
            RoundedRing { .. } => rings += 1,
            BlurredRoundedRect { .. } => blurred += 1,
            InsetBlurredRoundedRect { .. } => blurred += 1,
            DashedBorder { .. } => dashed += 1,
            TextRun { text_run, .. } => {
                texts += 1;
                glyphs += graph
                    .resources()
                    .text_run(*text_run)
                    .expect("text run resource")
                    .glyphs
                    .len();
            }
            Group { .. } => groups += 1,
            Clip { .. } => clips += 1,
            Image { .. } => images += 1,
            // fixture ツリーは draw を使わない。集計対象外（#724）。
            DrawList { .. } => {}
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
    let no_text = WithoutText(&graph);
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
        bench(&format!("blit copy+unpremultiply {w}x{h}"), 20, || {
            let mut data = pixmap.data().to_vec();
            premultiplied_to_straight(&mut data);
            std::hint::black_box(&data);
        });
    }

    // #636: composite-only フレーム（スクロール/transform）の CPU コストを全面ラスタと並べて計測する。
    // 配線前は毎フレーム全面 render_scene（上の scale ループ）。配線後はキャッシュ Pixmap の draw_pixmap
    // 合成だけ。差がそのままスクロール中フレームの短縮分（診断 原因 2 の効き）。
    for scale in [1.0f32, 2.0, 3.0] {
        let w = (vw * scale) as u32;
        let h = (vh * scale) as u32;
        let boundaries: HashSet<ElementId> = tree.frame_layers().iter().copied().collect();
        let root = tree.frame_layers()[0];
        let raster_bounds: std::collections::HashMap<ElementId, LayerRasterBounds> = tree
            .committed_frame()
            .layer_raster_bounds()
            .iter()
            .map(|bounds| (bounds.layer, *bounds))
            .collect();
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
            if layer == root {
                rasterizer.rasterize(layer, &scene, None).unwrap();
            } else {
                rasterizer
                    .rasterize_with_bounds(layer, &scene, raster_bounds[&layer], None)
                    .unwrap();
            }
        }
        let placements = collect_layer_placements(&graph, root, &boundaries);
        let cached_source_px: u64 = placements
            .iter()
            .filter_map(|placement| rasterizer.texture(placement.layer))
            .map(|texture| u64::from(texture.width()) * u64::from(texture.height()))
            .sum();
        let full_surface_source_px = u64::from(w) * u64::from(h) * placements.len() as u64;
        println!(
            "tiny-skia layer cache source px: {cached_source_px} / {full_surface_source_px} ({:.1}% reduction)",
            (1.0 - cached_source_px as f64 / full_surface_source_px as f64) * 100.0
        );
        if let Some(&layer) = tree.frame_layers().iter().find(|&&layer| layer != root) {
            let layer_scene = extract_layer_scene(&graph, layer, &boundaries).unwrap();
            let mut full_rasterizer = TinySkiaLayerRasterizer::new(w, h, scale);
            bench(
                &format!("tiny-skia RASTER full-surface layer {w}x{h}"),
                20,
                || {
                    full_rasterizer
                        .rasterize(layer, &layer_scene, None)
                        .unwrap()
                },
            );
            let mut bounded_rasterizer = TinySkiaLayerRasterizer::new(w, h, scale);
            bench(
                &format!(
                    "tiny-skia RASTER bounded layer {}x{}",
                    rasterizer.texture(layer).unwrap().width(),
                    rasterizer.texture(layer).unwrap().height()
                ),
                20,
                || {
                    bounded_rasterizer
                        .rasterize_with_bounds(layer, &layer_scene, raster_bounds[&layer], None)
                        .unwrap()
                },
            );
        }
        let mut compositor = TinySkiaLayerCompositor::new(scale);
        let mut target = TinySkiaCompositeTarget {
            pixmap: Pixmap::new(w, h).expect("pixmap"),
            clear: [1.0, 1.0, 1.0, 1.0],
        };
        bench(
            &format!(
                "tiny-skia COMPOSITE-ONLY {w}x{h} (scale {scale}, {} layers, {cached_source_px}/{full_surface_source_px} source px)",
                placements.len(),
            ),
            20,
            || {
                let quads: Vec<CompositeQuad<'_, TinySkiaLayerTexture>> = placements
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
