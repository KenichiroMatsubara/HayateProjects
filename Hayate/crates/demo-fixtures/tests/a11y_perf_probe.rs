//! 環境変数ゲート付き perf プローブ（Android Chrome vello モードのカクつき診断）。
//!
//! Web Canvas Accessibility Mirror（ADR-0124）は独立 rAF ループで毎 vsync
//! `poll_accessibility()` を呼ぶ。その中身は「AccessKit ツリーの全ツリー walk 再構築
//! （`accessibility_update`）＋ serde_json 全量シリアライズ」で、dirty ゲートが無い。
//! このプローブは、その毎フレーム CPU コストを実アプリ相当の `tasks_tree` fixture で
//! ホスト分解計測する（scene-renderers の `perf_probe.rs` と同型のループ）。
//!
//! 実行: HAYATE_PERF_PROBE=1 cargo test --release -p hayate-demo-fixtures \
//!        --test a11y_perf_probe -- --nocapture

use std::time::Instant;

use hayate_demo_fixtures::{tasks_tree, TASKS_VIEWPORT};

fn ms(d: std::time::Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn bench<F: FnMut()>(label: &str, iters: u32, mut f: F) {
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
    println!("[perf-probe] {label:<52} p50 {p50:8.3}ms  p95 {p95:8.3}ms  min {min:8.3}ms");
}

#[test]
fn a11y_perf_probe() {
    if std::env::var_os("HAYATE_PERF_PROBE").is_none() {
        return;
    }
    let (vw, vh) = TASKS_VIEWPORT;
    println!("[perf-probe] fixture viewport {vw}x{vh} (logical px)");

    let mut tree = tasks_tree("vello");
    let _ = tree.render(0.0);

    // ミラー tick の Rust 側全量（毎 vsync・変更の有無に関わらず走る）。
    let update = tree.accessibility_update().expect("a11y update");
    let node_count = update.nodes.len();
    let json_len = serde_json::to_string(&update).map(|s| s.len()).unwrap_or(0);
    println!("[perf-probe] a11y nodes {node_count}, JSON {json_len} bytes");

    bench("accessibility_update (full-tree walk)", 200, || {
        let u = tree.accessibility_update();
        std::hint::black_box(&u);
    });

    bench("serde_json::to_string(TreeUpdate)", 200, || {
        let u = tree.accessibility_update().unwrap();
        let s = serde_json::to_string(&u).unwrap();
        std::hint::black_box(&s);
    });

    // 参考: レンダラ側のアイドルフレーム（差分追跡が効く経路）との対比。
    let mut ts = 16.0f64;
    bench("tree.render idle (dirty-gated, for contrast)", 200, || {
        ts += 16.0;
        let _ = tree.render(ts);
    });
}
