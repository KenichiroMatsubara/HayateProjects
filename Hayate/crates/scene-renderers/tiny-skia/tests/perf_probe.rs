//! 環境変数ゲート付き perf プローブ（Android/モバイル描画遅延診断のフィードバックループ）。
//!
//! web CPU モード（Android Chrome の `?renderer=tiny-skia`）で毎フレーム無条件に走る
//! 仕事を host ネイティブで計測する（実機 WASM はさらに 2〜4 倍遅い前提で読む）:
//!   1. tiny-skia `render_scene` 全面ラスタ（scale 1.0 / 2.0 / 3.0 ≒ DPR）
//!   2. blit 前処理: `pixmap.data().to_vec()` + `premultiplied_to_straight`
//!
//! 実行: HAYATE_PERF_PROBE=1 cargo test --release -p hayate-scene-renderer-tiny-skia \
//!        --test perf_probe -- --nocapture

use std::time::Instant;

use hayate_demo_fixtures::{tasks_tree, TASKS_VIEWPORT};
use hayate_scene_renderer_tiny_skia::{premultiplied_to_straight, TinySkiaSceneRenderer};
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
    let (mut rects, mut rings, mut texts, mut glyphs, mut groups, mut clips, mut images, mut anchors, mut dashed) =
        (0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
    for (_, node) in graph.iter() {
        use hayate_core::NodeKind::*;
        match &node.kind {
            Rect { .. } => rects += 1,
            RoundedRing { .. } => rings += 1,
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
        "[perf-probe] fixture {vw}x{vh}: nodes {} (rect {rects}, ring {rings}, dashed {dashed}, textrun {texts} / glyphs {glyphs}, group {groups}, clip {clips}, image {images}, anchor {anchors})",
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
}
