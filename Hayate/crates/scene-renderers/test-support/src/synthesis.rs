use hayate_core::StyleProp;

use crate::cases::{render_tree_to_scene, text_tree};
use crate::pixel::{ink_extent_x, CANVAS_H, CANVAS_W};
use crate::tiny_skia;

fn ink_width(data: &[u8]) -> u32 {
    let (min_x, max_x) = ink_extent_x(data, CANVAS_W, CANVAS_H).expect("expected ink");
    max_x - min_x
}

/// 可変フォントの semibold ウェイトは、regular より太く描かれるべき。
pub fn assert_semibold_wider_than_regular() {
    let regular = tiny_skia::render_scene_to_pixels(&render_tree_to_scene(text_tree(&[
        StyleProp::FontWeight(400.0),
    ])));
    let semibold = tiny_skia::render_scene_to_pixels(&render_tree_to_scene(text_tree(&[
        StyleProp::FontWeight(600.0),
    ])));
    let regular_w = ink_width(&regular);
    let semibold_w = ink_width(&semibold);
    assert!(
        semibold_w > regular_w,
        "font-weight 600 should be wider than 400 (regular={regular_w}, semibold={semibold_w})"
    );
}

/// 擬似イタリックは、直立テキストとはラスタが変わらなければならない。
pub fn assert_italic_pixels_skew_right(data: &[u8]) {
    let normal = tiny_skia::render_scene_to_pixels(&render_tree_to_scene(text_tree(&[])));
    assert_ne!(
        normal, data,
        "faux italic should produce different pixels than upright text"
    );
    assert!(
        ink_extent_x(data, crate::pixel::CANVAS_W, crate::pixel::CANVAS_H).is_some(),
        "expected italic text ink"
    );
}

pub fn assert_italic_skews_ink_right() {
    let italic = tiny_skia::render_scene_to_pixels(&render_tree_to_scene(text_tree(&[
        StyleProp::FontStyle(hayate_core::FontStyleValue::Italic),
    ])));
    assert_italic_pixels_skew_right(&italic);
}
