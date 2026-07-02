//! rubber バウンド（オーバースクロールのスプリングバック）1 フレームあたりのコスト分解
//! プローブ。「Android Chrome で rubber バウンドが vello モードでも重い」のフィードバック
//! ループ（perf_probe.rs の姉妹。同じく env ゲート付きで常設）。
//!
//! 実バウンス経路（`start_scroll_momentum` → `render` 内の `advance_scroll_motion` →
//! ばね積分 → scroll group アフィン再 lowering）をホストで 1 フレームずつ駆動し、
//! フレームごとに走る仕事を分解する:
//!   1. core 側: `tree.render`（差分 lowering。walk_count で再 lowering 要素数を確認）
//!   2. present 側: SceneGraph → vello `Scene` フルエンコード
//!   3. present 側: vello フル GPU render（wgpu アダプタがあれば。llvmpipe 参考値）
//!      — バウンス 1 フレーム（scroll group の translate が変わっただけ）と
//!        コールドフル フレームの GPU 時間を比べ、present コストが「変更の大きさに
//!        依存しない」ことを数値で出す。
//!
//! 実行: HAYATE_PERF_PROBE=1 cargo test --release -p hayate-scene-renderer-vello \
//!        --test perf_probe_rubber_bounce -- --nocapture
//!
//! env ゲートなしでも repro の力学だけは常に検証する（バウンスが実際にアニメーション
//! すること・バウンス中の再 lowering が scroll-view 1 要素に閉じること）。

use std::time::Instant;

use hayate_core::{
    Color, Dimension, ElementKind, ElementTree, FlexDirectionValue, StyleProp,
};
use hayate_demo_fixtures::TreeBuilder;
use hayate_scene_renderer_vello::debug_encode_scene;

/// Android 実機の代表的な論理ビューポート（CSS px）。DPR 3 で 1170x2532 物理 px 相当。
const VIEWPORT: (f32, f32) = (390.0, 844.0);
const ROW_COUNT: usize = 80;
const ROW_HEIGHT: f32 = 56.0;
const FRAME_MS: f64 = 1000.0 / 60.0;
/// スプリングバックが収束しない場合の安全上限。
const MAX_BOUNCE_FRAMES: usize = 600;

fn ms(d: std::time::Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn p50(samples: &mut Vec<f64>) -> f64 {
    samples.sort_by(|a, b| a.partial_cmp(b).unwrap());
    samples[samples.len() / 2]
}

/// スクロール可能な縦リスト（Android の todo/フィード画面相当）。scroll-view 直下に
/// `ROW_COUNT` 行（背景付き view + テキスト）で、コンテンツ高がビューポートを大きく
/// 超えるので `max_y > 0` になり縦の rubber バウンドが成立する。
fn scrollable_list_tree() -> (ElementTree, hayate_core::ElementId) {
    let (vw, vh) = VIEWPORT;
    let mut b = TreeBuilder::new();
    let root = b.view(&[
        StyleProp::Width(Dimension::percent(100.0)),
        StyleProp::Height(Dimension::percent(100.0)),
        StyleProp::Display(hayate_core::DisplayValue::Flex),
        StyleProp::FlexDirection(FlexDirectionValue::Column),
        StyleProp::BackgroundColor(Color::new(0.95, 0.93, 0.89, 1.0)),
        StyleProp::DefaultColor(Color::new(0.2, 0.17, 0.25, 1.0)),
        StyleProp::DefaultFontSize(14.0),
        StyleProp::DefaultFontFamily("Inter".to_string()),
    ]);
    b.tree.set_root(root);
    b.tree.set_viewport(vw, vh);

    let sv = b.el(
        ElementKind::ScrollView,
        &[
            StyleProp::Width(Dimension::percent(100.0)),
            StyleProp::Height(Dimension::percent(100.0)),
            StyleProp::Display(hayate_core::DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Column),
        ],
    );
    b.child(root, sv);

    for i in 0..ROW_COUNT {
        let row = b.view(&[
            StyleProp::Height(Dimension::px(ROW_HEIGHT)),
            StyleProp::Display(hayate_core::DisplayValue::Flex),
            StyleProp::FlexDirection(FlexDirectionValue::Row),
            StyleProp::AlignItems(hayate_core::AlignValue::Center),
            StyleProp::PaddingLeft(Dimension::px(16.0)),
            StyleProp::BackgroundColor(if i % 2 == 0 {
                Color::new(0.99, 0.99, 0.98, 1.0)
            } else {
                Color::new(0.93, 0.90, 0.85, 1.0)
            }),
            StyleProp::BorderRadius(8.0),
        ]);
        let label = b.text(&format!("タスク {i}: rubber バウンド計測行"), &[]);
        b.child(row, label);
        b.child(sv, row);
    }
    (b.tree, sv)
}

/// オーバースクロール位置から指を離した状態を作り（速度 0・端の外 → スプリングバック
/// が必ず起動する）、`render` を 60fps 刻みで進めて収束までの各フレームを観測する。
struct BounceRun {
    /// フレームごとの (tree.render 所要 ms, 再 lowering 要素数, スクロール y オフセット)。
    frames: Vec<(f64, usize, f32)>,
    max_y: f32,
}

/// `ts` は直近 `render` に渡したクロックの続きであること（`advance_scroll_motion` は
/// フレーム間 dt で積分するので、クロックが飛ぶとばねが 1 ステップで収束してしまう）。
fn run_bounce(
    tree: &mut ElementTree,
    sv: hayate_core::ElementId,
    overshoot: f32,
    mut ts: f64,
) -> BounceRun {
    let (_, max_y) = tree.element_scroll_max_offset(sv);
    assert!(max_y > 0.0, "fixture must be vertically scrollable");
    // 下端を越えてドラッグした位置で指を離した直後を再現する。
    tree.element_set_scroll_offset(sv, 0.0, max_y + overshoot);
    tree.start_scroll_momentum(sv, 0.0, 0.0);
    assert!(
        tree.has_pending_visual_work(),
        "release in overscroll must start spring-back animation"
    );

    let mut frames = Vec::new();
    while tree.has_pending_visual_work() && frames.len() < MAX_BOUNCE_FRAMES {
        ts += FRAME_MS;
        let t = Instant::now();
        let _ = tree.render(ts);
        let dt = ms(t.elapsed());
        let (_, oy) = tree.element_get_scroll_offset(sv);
        frames.push((dt, tree.test_scene_lowering_walk_count(), oy));
    }
    BounceRun { frames, max_y }
}

/// repro の力学（env ゲートなしで常に検証）:
///   - オーバースクロールで離すとスプリングバックが複数フレームにわたりアニメーションし、
///     端（max_y）へ収束する = ユーザーが見る「rubber バウンド」そのもの。
///   - バウンス中の各フレームで再 lowering されるのは scroll-view 1 要素だけ
///     （SelfOnly reach）。core の差分追跡は無実、という前提を固定する。
#[test]
fn rubber_bounce_mechanics() {
    let (mut tree, sv) = scrollable_list_tree();
    // コールドフレームでレイアウト＋全 lowering を済ませる。
    let _ = tree.render(0.0);
    let run = run_bounce(&mut tree, sv, 120.0, 0.0);

    assert!(
        run.frames.len() >= 10,
        "spring-back should animate over many frames, got {}",
        run.frames.len()
    );
    assert!(
        run.frames.len() < MAX_BOUNCE_FRAMES,
        "spring-back must settle"
    );
    let (_, _, final_y) = *run.frames.last().unwrap();
    assert!(
        (final_y - run.max_y).abs() < 1.0,
        "offset must settle at the edge: final {final_y} vs max {}",
        run.max_y
    );
    // バウンス中フレーム（初回以外）は scroll-view 1 要素の再 lowering に閉じる。
    for (i, (_, walk, _)) in run.frames.iter().enumerate() {
        assert!(
            *walk <= 1,
            "bounce frame {i} re-lowered {walk} elements (expected <= 1: SelfOnly reach)"
        );
    }
}

/// フレームごとのコスト分解（env ゲート付き。数値レポート用）。
#[test]
fn perf_probe_rubber_bounce() {
    if std::env::var_os("HAYATE_PERF_PROBE").is_none() {
        return;
    }
    let (vw, vh) = VIEWPORT;
    let (mut tree, sv) = scrollable_list_tree();
    let t = Instant::now();
    let node_count = tree.render(0.0).iter().count();
    println!(
        "[bounce-probe] viewport {vw}x{vh} logical px, rows {ROW_COUNT}, cold render {:.3}ms, scene nodes {node_count}",
        ms(t.elapsed())
    );

    // ── 1. core 側: バウンス中の tree.render ─────────────────────────────────
    let run = run_bounce(&mut tree, sv, 120.0, 0.0);
    let n = run.frames.len();
    let mut render_ms: Vec<f64> = run.frames.iter().map(|f| f.0).collect();
    let walk_max = run.frames.iter().map(|f| f.1).max().unwrap_or(0);
    println!(
        "[bounce-probe] spring-back frames {n} ({:.0}ms of animation), tree.render p50 {:.3}ms, re-lowered elements per frame <= {walk_max}",
        n as f64 * FRAME_MS,
        p50(&mut render_ms)
    );

    // ── 2. present 側: フルエンコード（web backend は毎 render で無条件に実行）──
    let mut ts = (run.frames.len() + 2) as f64 * FRAME_MS;
    let graph = tree.render(ts).clone();
    for scale in [1.0f32, 3.0] {
        let mut samples = Vec::new();
        for _ in 0..100 {
            let t = Instant::now();
            let s = debug_encode_scene(&graph, scale);
            std::hint::black_box(&s);
            samples.push(ms(t.elapsed()));
        }
        println!(
            "[bounce-probe] vello Scene full encode scale={scale}: p50 {:.3}ms",
            p50(&mut samples)
        );
    }

    // ── 3. present 側: バウンス 1 フレームの GPU コスト vs コールドフル ─────────
    // scroll group の translate 1 本が変わっただけのフレームでも、web backend は
    // `render_to_texture` フルパイプラインを物理解像度で回す。両者の GPU 時間比が
    // ~1.0 なら「present コストは変更の大きさに依存しない」が確定する。
    match hayate_scene_test_support::vello::try_vello_harness() {
        None => println!("[bounce-probe] wgpu adapter なし → GPU render 計測はスキップ"),
        Some(mut h) => {
            for scale in [1.0f32, 3.0] {
                let w = (vw * scale) as u32;
                let hgt = (vh * scale) as u32;

                // コールドフル フレーム基準値（全画面初回描画に相当）。
                let mut full = Vec::new();
                for _ in 0..10 {
                    let t = Instant::now();
                    let px = hayate_scene_test_support::vello::render_scene_to_pixels_scaled(
                        &mut h, &graph, w, hgt, scale,
                    );
                    assert!(px.is_some());
                    full.push(ms(t.elapsed()));
                }
                let full_p50 = p50(&mut full);

                // バウンス中の連続 12 フレーム: 毎フレーム物理を 1 ステップ進めて
                // scroll group アフィンだけ変えた graph を都度フル render する
                // （web backend の present 経路と同型）。
                tree.element_set_scroll_offset(sv, 0.0, tree.element_scroll_max_offset(sv).1 + 120.0);
                tree.start_scroll_momentum(sv, 0.0, 0.0);
                let mut bounce = Vec::new();
                for _ in 0..12 {
                    ts += FRAME_MS;
                    let g = tree.render(ts).clone();
                    let t = Instant::now();
                    let px = hayate_scene_test_support::vello::render_scene_to_pixels_scaled(
                        &mut h, &g, w, hgt, scale,
                    );
                    assert!(px.is_some());
                    bounce.push(ms(t.elapsed()));
                }
                let bounce_p50 = p50(&mut bounce);
                println!(
                    "[bounce-probe] GPU {w}x{hgt} (scale {scale}): cold-full p50 {full_p50:.3}ms, bounce-frame p50 {bounce_p50:.3}ms, ratio {:.2}",
                    bounce_p50 / full_p50
                );
            }
        }
    }
}
