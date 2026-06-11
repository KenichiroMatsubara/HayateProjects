//! Font synthesis behavior (ADR-0085) for the vello backend.

use hayate_core::StyleProp;
use hayate_scene_test_support::cases::{render_tree_to_scene, text_tree};
use hayate_scene_test_support::pixel::{ink_extent_x, CANVAS_H, CANVAS_W};
use hayate_scene_test_support::{synthesis, try_vello_harness, vello};

#[test]
fn font_weight_600_is_wider_than_400() {
    let Some(mut harness) = try_vello_harness() else {
        eprintln!("font_synthesis (vello): skipped — no wgpu adapter");
        return;
    };
    let regular = vello::render_scene_to_pixels(
        &mut harness,
        &render_tree_to_scene(text_tree(&[StyleProp::FontWeight(400.0)])),
    )
    .expect("regular render");
    let semibold = vello::render_scene_to_pixels(
        &mut harness,
        &render_tree_to_scene(text_tree(&[StyleProp::FontWeight(600.0)])),
    )
    .expect("semibold render");
    let regular_w = ink_extent_x(&regular, CANVAS_W, CANVAS_H)
        .map(|(a, b)| b - a)
        .expect("regular ink");
    let semibold_w = ink_extent_x(&semibold, CANVAS_W, CANVAS_H)
        .map(|(a, b)| b - a)
        .expect("semibold ink");
    assert!(
        semibold_w > regular_w,
        "font-weight 600 should be wider than 400 (regular={regular_w}, semibold={semibold_w})"
    );
}

#[test]
fn font_style_italic_skews_glyphs() {
    let Some(mut harness) = try_vello_harness() else {
        eprintln!("font_synthesis (vello): skipped — no wgpu adapter");
        return;
    };
    let italic = vello::render_scene_to_pixels(
        &mut harness,
        &render_tree_to_scene(text_tree(&[StyleProp::FontStyle(
            hayate_core::FontStyleValue::Italic,
        )])),
    )
    .expect("italic render");
    synthesis::assert_italic_pixels_skew_right(&italic);
}
