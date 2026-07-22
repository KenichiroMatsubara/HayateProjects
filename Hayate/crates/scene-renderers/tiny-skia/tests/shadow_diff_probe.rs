//! 環境変数ゲート付き差分プローブ: Tasks 画面の box-shadow 税を、影を剥がした
//! バリアント（CSS Gallery のカードは影ゼロ）と比較して定量化する。box-shadow が
//! Tasks を重く / Gallery を軽くしている構造差の裏取り（render 時間・overdraw・rect 数）。
//! Run: HAYATE_PERF_PROBE=1 cargo test --release -p hayate-scene-renderer-tiny-skia \
//!        --test shadow_diff_probe -- --nocapture

use std::time::Instant;

use hayate_core::{ElementTree, NodeKind, StyleProp};
use hayate_demo_fixtures::{tasks_tree, TASKS_VIEWPORT};
use hayate_scene_renderer_tiny_skia::TinySkiaSceneRenderer;
use tiny_skia::Pixmap;

fn ms(d: std::time::Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn bench<F: FnMut()>(label: &str, iters: u32, mut f: F) {
    for _ in 0..2 {
        f();
    }
    let mut s = Vec::with_capacity(iters as usize);
    for _ in 0..iters {
        let t = Instant::now();
        f();
        s.push(ms(t.elapsed()));
    }
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "{label:<40} p50 {:8.3}ms  min {:8.3}ms",
        s[s.len() / 2],
        s[0]
    );
}

fn strip_shadows(tree: &mut ElementTree) {
    let root = tree.root().expect("root");
    for id in tree.subtree_element_ids(root) {
        tree.element_set_style(id, &[StyleProp::BoxShadow(vec![])]);
    }
}

fn stats(tree: &mut ElementTree, vw: f64, vh: f64) -> (usize, f64) {
    let graph = tree.render(0.0);
    let mut rects = 0usize;
    let mut area = 0f64;
    for (_, n) in graph.iter() {
        if let NodeKind::Rect { width, height, .. } = &n.kind {
            rects += 1;
            area += *width as f64 * *height as f64;
        }
    }
    (rects, area / (vw * vh))
}

#[test]
fn shadow_diff_probe() {
    if std::env::var_os("HAYATE_PERF_PROBE").is_none() {
        return;
    }
    let (vw, vh) = TASKS_VIEWPORT;
    let (w, h) = (vw as u32, vh as u32);

    let mut with = tasks_tree("tiny-skia");
    let (r_with, od_with) = stats(&mut with, vw as f64, vh as f64);

    let mut without = tasks_tree("tiny-skia");
    strip_shadows(&mut without);
    let (r_wo, od_wo) = stats(&mut without, vw as f64, vh as f64);

    println!("WITH shadows : rect nodes {r_with}, overdraw {od_with:.2} viewports");
    println!("NO   shadows : rect nodes {r_wo}, overdraw {od_wo:.2} viewports");
    println!(
        "shadow tax   : +{} rect nodes, +{:.2} viewports overdraw",
        r_with - r_wo,
        od_with - od_wo
    );

    // 影レイヤの面積分布: 大パネル影(blur40, 巨大レイヤ11枚) vs 行影(blur6, 小レイヤ55枚)
    // のどちらが overdraw を支配するかを、rect 面積の降順で覗く。
    with.render(0.0);
    let gg = with.committed_frame().snapshot().clone();
    let mut areas: Vec<f64> = gg
        .iter()
        .filter_map(|(_, n)| match &n.kind {
            NodeKind::Rect { width, height, .. } => Some(*width as f64 * *height as f64),
            _ => None,
        })
        .collect();
    areas.sort_by(|a, b| b.partial_cmp(a).unwrap());
    let vp = vw as f64 * vh as f64;
    let top12: f64 = areas.iter().take(12).sum();
    println!(
        "top-12 rect areas (viewports): {:?}",
        areas
            .iter()
            .take(12)
            .map(|a| (a / vp * 100.0).round() / 100.0)
            .collect::<Vec<_>>()
    );
    println!(
        "top-12 rects sum = {:.2} vp of {:.2} vp total overdraw",
        top12 / vp,
        areas.iter().sum::<f64>() / vp
    );

    with.render(0.0);
    let g_with = with.committed_frame().snapshot().clone();
    without.render(0.0);
    let g_wo = without.committed_frame().snapshot().clone();
    for (label, g) in [("WITH-shadows", &g_with), ("NO-shadows", &g_wo)] {
        let mut px = Pixmap::new(w, h).expect("px");
        let mut r = TinySkiaSceneRenderer::new();
        bench(&format!("render {label} {w}x{h}"), 20, || {
            r.render_scene(g, &mut px, [1.0, 1.0, 1.0, 1.0], 1.0);
        });
    }
}
