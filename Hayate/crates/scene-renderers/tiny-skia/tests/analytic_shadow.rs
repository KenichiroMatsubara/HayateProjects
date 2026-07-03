//! tiny-skia の解析ぼかしシャドウ（issue #658）。ぼかし角丸矩形（#657 のプリミティブ）を、
//! default の erf シェル近似フォールバックではなく **per-pixel の解析被覆**で塗ることを、
//! 実ラスタ出力から検証する。σ を大きく取り縁の外側で影が **滑らかに単調フェード** し、かつ
//! 中間被覆（部分アルファ）が多段で現れることを確認する——シェルフォールバックの離散帯とは
//! 異なる連続プロファイル。背景は不透明白（`CLEAR_COLOR`）なので、黒影の premultiplied 出力
//! `rgb = 255·(1 − α_final)` から被覆を復元できる。

use hayate_core::{Color, Dimension, ElementKind, ElementTree, Shadow, StyleProp};
use hayate_scene_test_support::pixel::pixel;
use hayate_scene_test_support::tiny_skia::render_scene_to_pixels;

const CANVAS_W: u32 = 100;

fn shadow_scene(blur: f32) -> Vec<u8> {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(100.0, 100.0);
    // 中央付近に不透明ボックス。オフセット 0 で四方に影が広がる。
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(40.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::BackgroundColor(Color::new(1.0, 1.0, 1.0, 1.0)),
            StyleProp::BoxShadow(vec![Shadow {
                offset_x: 0.0,
                offset_y: 0.0,
                blur,
                spread: 0.0,
                color: Color::new(0.0, 0.0, 0.0, 0.8),
                inset: false,
            }]),
        ],
    );
    render_scene_to_pixels(&tree.render(0.0).clone())
}

/// premultiplied（不透明白背景）出力から黒影の被覆を復元する: rgb0 = 255·(1 − 0.8·cov)。
fn coverage_at(data: &[u8], x: u32, y: u32) -> f32 {
    let px = pixel(data, CANVAS_W, x, y);
    (1.0 - px[0] as f32 / 255.0) / 0.8
}

#[test]
fn drop_shadow_fades_smoothly_and_monotonically_outside_the_box() {
    // ボックスはレイアウト上 (0,0)-(40,40)。右辺 x=40 から外へ y=20（縦中心）で走査する。
    // σ=15（blur 30）で裾を広く取り、解析パスの連続プロファイルを観測する。
    let data = shadow_scene(30.0);

    // 縁の外側 x=41..=88 の被覆列。単調非増加（外へ薄くなる）で、隣接段差は小さい
    // （per-pixel の滑らかフェード。離散シェル塗りなら段差が大きくなる）。
    let mut prev = 2.0_f32;
    let mut samples = Vec::new();
    for x in 41..=88u32 {
        let cov = coverage_at(&data, x, 20).clamp(0.0, 1.0);
        assert!(
            cov <= prev + 0.02,
            "coverage must not rise outward at x={x}: {cov} > {prev}"
        );
        if !samples.is_empty() {
            let jump = (prev - cov).abs();
            assert!(
                jump < 0.08,
                "adjacent coverage step at x={x} is too large ({jump:.3}); expected a smooth analytic ramp"
            );
        }
        prev = cov;
        samples.push(cov);
    }

    // 縁近くは実質的な影、遠方は消える。
    assert!(samples[0] > 0.2, "just outside the edge should be shadowed, got {}", samples[0]);
    assert!(
        *samples.last().unwrap() < 0.05,
        "far from the box the shadow should vanish, got {}",
        samples.last().unwrap()
    );

    // 解析パスは連続プロファイル: シェルフォールバック（11 段）を超える中間被覆レベルが出る。
    // 0.05..0.75 の帯に入るサンプル数で「多段の中間フェード」を捉える。
    let mid = samples.iter().filter(|&&c| c > 0.05 && c < 0.75).count();
    assert!(
        mid >= 14,
        "an analytic falloff should show many intermediate coverage levels, got {mid}"
    );
}

#[test]
fn wider_blur_spreads_the_shadow_farther() {
    // σ が大きいほど裾は遠くまで届く。ボックス縁（x=40）から 12px 外の x=52 では、
    // 狭い blur の影は消えているが広い blur は残る。
    let near = coverage_at(&shadow_scene(8.0), 52, 20).clamp(0.0, 1.0);
    let far = coverage_at(&shadow_scene(30.0), 52, 20).clamp(0.0, 1.0);
    assert!(
        far > near + 0.05,
        "a larger blur must spread the shadow farther (x=52): wide={far} vs narrow={near}"
    );
}
