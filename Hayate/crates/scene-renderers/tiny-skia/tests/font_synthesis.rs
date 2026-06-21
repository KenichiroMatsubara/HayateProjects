//! tiny-skia バックエンドのフォント合成挙動（ADR-0085）。

use hayate_scene_test_support::synthesis;

#[test]
fn font_weight_600_is_wider_than_400() {
    synthesis::assert_semibold_wider_than_regular();
}

#[test]
fn font_style_italic_skews_glyphs() {
    synthesis::assert_italic_skews_ink_right();
}
