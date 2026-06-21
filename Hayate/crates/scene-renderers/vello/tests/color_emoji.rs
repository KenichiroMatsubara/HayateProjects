//! Vello バックエンドでのカラーグリフ（COLR/CPAL）描画。
//!
//! Vello が COLR グリフを実際にカラーで描くことを検証する。
//! `draw_glyphs().draw()` が COLR/CPAL を検出して `try_draw_colr` へ振り分ける。
//! モノクロ描画（tiny-skia）は同じグリフを単色に潰すため、別途 tiny-skia 側で検証する。
//!
//! 24MB の本番絵文字フォントではなく、バンドルされた小さな COLRv1 テストフォントを使うため
//! ネットワーク不要（出自は `test-support/assets/PROVENANCE.md`）。他の Vello 視覚テストと
//! 同様、wgpu アダプタが無ければスキップする。

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
