//! tiny-skia is the CPU fallback and cannot paint COLR/CBDT colour glyphs — it
//! draws outlines only (issue #332, ADR-0101). Rendering the same COLR test
//! glyph the Vello test renders in colour must collapse to a single hue here.
//! This is the regression guard for "tiny-skia stays monochrome", and it also
//! shows the hue metric the Vello test relies on actually discriminates.

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
