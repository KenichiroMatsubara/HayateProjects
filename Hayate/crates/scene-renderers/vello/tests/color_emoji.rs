//! Colour glyph (COLR/CPAL) rendering on the Vello backend (issue #332).
//!
//! The web adapter routes emoji to the colour `Noto Color Emoji` build only on
//! the Vello (WebGPU) path (`hayate-adapter-web`'s `font_url_for_renderer`).
//! This test proves the other half: that Vello actually *paints* a COLR glyph
//! in colour — `draw_glyphs().draw()` detects COLR/CPAL and routes through
//! `try_draw_colr`. A monochrome painter (tiny-skia) would collapse the same
//! glyph to a single ink colour; that path is covered by the tiny-skia tests.
//!
//! Uses a tiny bundled COLRv1 test font (provenance in
//! `test-support/assets/PROVENANCE.md`), not the 24 MB production emoji font,
//! so it needs no network. Skips when no wgpu adapter is available, like the
//! other Vello visual tests.

use hayate_scene_test_support::cases::{color_glyph_tree, render_tree_to_scene};
use hayate_scene_test_support::pixel::{distinct_saturated_hues, CANVAS_H, CANVAS_W};
use hayate_scene_test_support::{try_vello_harness, vello};

#[test]
fn vello_paints_colr_glyph_in_multiple_hues() {
    let Some(mut harness) = try_vello_harness() else {
        eprintln!("color_emoji (vello): skipped — no wgpu adapter");
        return;
    };

    let pixels =
        vello::render_scene_to_pixels(&mut harness, &render_tree_to_scene(color_glyph_tree()))
            .expect("render COLR glyph");

    let hues = distinct_saturated_hues(&pixels, CANVAS_W, CANVAS_H, 60);
    assert!(
        hues >= 2,
        "Vello must paint the COLR glyph in colour (expected >=2 distinct hues, got {hues}); \
         a single hue means COLR was ignored and the glyph fell back to a flat colour"
    );
}
