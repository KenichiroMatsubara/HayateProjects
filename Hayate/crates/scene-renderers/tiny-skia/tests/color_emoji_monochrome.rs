//! tiny-skia は CPU フォールバックで COLR/CBDT のカラーグリフを描けず、アウトライン
//! のみを描く（ADR-0101）。Vello がカラー描画する同じ COLR グリフは、ここでは単一の
//! 色相に収束しなければならない（モノクロ維持の回帰ガード）。

use hayate_scene_test_support::cases::{color_glyph_tree, render_tree_to_scene};
use hayate_scene_test_support::pixel::{distinct_saturated_hues, CANVAS_H, CANVAS_W};
use hayate_scene_test_support::tiny_skia;

#[test]
fn tiny_skia_renders_colr_glyph_monochrome() {
    let pixels = tiny_skia::render_scene_to_pixels(&render_tree_to_scene(color_glyph_tree()));
    let hues = distinct_saturated_hues(&pixels, CANVAS_W, CANVAS_H, 60);
    assert!(
        hues <= 1,
        "tiny-skia must not paint colour glyphs (expected <=1 saturated hue, got {hues})"
    );
}
