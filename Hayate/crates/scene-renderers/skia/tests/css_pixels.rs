//! Hayate の CSS カタログ全プロパティに対するピクセル単位の回帰テスト（skia）。
//! `hayate-scene-test-support` の renderer 非依存フィクスチャ（tiny-skia/vello と共有）を
//! この crate の公開 API（`SkiaSceneRenderer::render_scene`）で raster する。

mod support;

use hayate_scene_test_support::cases::{render_tree_to_scene, CssPixelCase, BORDER_RASTER_CASES, CSS_PIXEL_CASES};
use support::render_scene_to_pixels;

fn run(case: &CssPixelCase) {
    let sg = render_tree_to_scene((case.build)());
    let pixels = render_scene_to_pixels(&sg);
    (case.check)(&pixels);
}

fn run_all(cases: &[CssPixelCase]) {
    for case in cases {
        let prop = case.css_property;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(case)));
        if let Err(payload) = result {
            std::panic::resume_unwind(match payload.downcast::<String>() {
                Ok(s) => Box::new(format!("{prop}: {s}")),
                Err(p) => p,
            });
        }
    }
}

#[test]
fn all_catalog_css_properties_skia() {
    // "color" は tiny-skia/vello の AA/hinting に合わせて調律された単一ピクセル
    // しきい値で、Skia は同じグリフ縁で数ポイント異なるカバレッジを出す（機能欠落では
    // ない — `color_reaches_the_glyph` が同じケースをより頑健なサンプリングで検証する）。
    // レンダラ間ピクセル比較はしない（ADR-0146 §8）ので、ここでは除外する。
    for case in CSS_PIXEL_CASES.iter().filter(|c| c.css_property != "color") {
        let prop = case.css_property;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(case)));
        if let Err(payload) = result {
            std::panic::resume_unwind(match payload.downcast::<String>() {
                Ok(s) => Box::new(format!("{prop}: {s}")),
                Err(p) => p,
            });
        }
    }
}

/// "color" の代替: 特定ピクセルのしきい値ではなく、グリフ領域内で最も彩度の高い
/// ピクセルの色相を見る（AA 縁の 1px サンプリングより頑健）。
#[test]
fn color_reaches_the_glyph() {
    use hayate_scene_test_support::pixel::pixel;
    let sg = render_tree_to_scene(hayate_scene_test_support::cases::text_tree(&[
        hayate_core::StyleProp::Color(hayate_core::Color::new(1.0, 0.0, 0.0, 1.0)),
    ]));
    let pixels = render_scene_to_pixels(&sg);
    let mut reddest = [255u8, 255, 255, 255];
    for y in 0..30 {
        for x in 0..20 {
            let px = pixel(&pixels, 100, x, y);
            if px[0] > reddest[0].saturating_sub(1) && px[1] < reddest[1] {
                reddest = px;
            }
        }
    }
    assert!(reddest[0] > 150, "glyph ink should be red-dominant, got {reddest:?}");
    assert!(reddest[1] < 100, "glyph ink should not be washed out green, got {reddest:?}");
}

/// 1px ボーダーは不透明な列として描かれ、フォーカスリングは重ねたコンテンツを
/// 決して消さない（skia）。
#[test]
fn border_raster_regressions_skia() {
    run_all(BORDER_RASTER_CASES);
}
