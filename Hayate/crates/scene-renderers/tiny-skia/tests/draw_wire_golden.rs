//! wire 入口 → ピクセルの draw golden テスト（#724 / ADR-0141）。
//!
//! 本機能の主検証面は「`apply_mutations`（新 `draws` チャネル込み）に display list を
//! 入れたら正しいピクセルが出る」こと。lowering の内部構造は検証しない。ops /
//! styles / draws バッファを wire 形式で手組みし、中立 `apply_mutations` →
//! `render` → tiny-skia ラスタ → golden 比較まで一気通貫で通す。
//!
//! golden 更新: `HAYATE_UPDATE_GOLDEN=1 cargo test -p hayate-scene-renderer-tiny-skia --test draw_wire_golden`

use std::path::PathBuf;

use hayate_core::wire::{
    apply_mutations, DRAW_OP_CLOSE, DRAW_OP_FILL, DRAW_OP_LINE_TO, DRAW_OP_MOVE_TO,
    DRAW_PAINT_COLOR, ELEMENT_KIND_VIEW, OP_APPEND_CHILD, OP_CREATE, OP_SET_DRAW, OP_SET_ROOT,
    OP_SET_STYLE, TAG_BACKGROUND_COLOR, TAG_HEIGHT, TAG_OVERFLOW, TAG_WIDTH,
};
use hayate_core::ElementTree;
use hayate_scene_test_support::golden::assert_pixels_match_golden;
use hayate_scene_test_support::pixel::{assert_clear, assert_near, pixel, CANVAS_H, CANVAS_W};
use hayate_scene_test_support::tiny_skia::render_scene_to_pixels;

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(format!("{name}.png"))
}

/// 論理 px の dimension スタイルスロット（`[TAG, value, unit=px]`）。
fn dim(tag: u32, v: f32) -> [f32; 3] {
    [tag as f32, v, 0.0]
}

fn render_wire(ops: &[f64], styles: &[f32], draws: &[f32]) -> Vec<u8> {
    let mut tree = ElementTree::new();
    tree.set_viewport(CANVAS_W as f32, CANVAS_H as f32);
    apply_mutations(&mut tree, ops, styles, &[], draws).expect("apply_mutations");
    tree.render(0.0);
    render_scene_to_pixels(tree.scene_graph())
}

// wire 入口から投入した多角形（三角形）fill が期待ピクセルに出る。
#[test]
fn wire_polygon_fill_matches_golden() {
    // styles: root は 100x100 の白背景 view。
    let mut styles: Vec<f32> = Vec::new();
    styles.extend(dim(TAG_WIDTH, 100.0));
    styles.extend(dim(TAG_HEIGHT, 100.0));
    let style_len = styles.len();

    // draws: 赤い三角形 (10,10)-(90,10)-(50,70)。
    let draws: Vec<f32> = vec![
        DRAW_OP_MOVE_TO as f32,
        10.0,
        10.0,
        DRAW_OP_LINE_TO as f32,
        90.0,
        10.0,
        DRAW_OP_LINE_TO as f32,
        50.0,
        70.0,
        DRAW_OP_CLOSE as f32,
        DRAW_OP_FILL as f32,
        5.0,
        DRAW_PAINT_COLOR as f32,
        1.0,
        0.0,
        0.0,
        1.0,
    ];

    let ops: Vec<f64> = vec![
        OP_CREATE as f64,
        1.0,
        ELEMENT_KIND_VIEW as f64,
        OP_SET_ROOT as f64,
        1.0,
        OP_SET_STYLE as f64,
        1.0,
        0.0,
        style_len as f64,
        OP_SET_DRAW as f64,
        1.0,
        0.0,
        draws.len() as f64,
    ];

    let pixels = render_wire(&ops, &styles, &draws);

    // 三角形内部は赤、外は白のまま。
    assert_near(
        pixel(&pixels, CANVAS_W, 50, 20),
        [255, 0, 0, 255],
        2,
        "inside the triangle",
    );
    assert_clear(pixel(&pixels, CANVAS_W, 5, 50), "left of the triangle");
    assert_clear(pixel(&pixels, CANVAS_W, 50, 85), "below the apex");

    assert_pixels_match_golden(
        &golden_path("draw_wire_polygon_fill"),
        &pixels,
        CANVAS_W,
        CANVAS_H,
    );
}

/// overflow テスト共通の wire ストリーム: root(100x100) の下に 40x40 の子 view を
/// 置き、box を大きくはみ出す 5..80 の青い正方形を draw する。`child_styles` で
/// 子のスタイル（overflow の有無）だけ差し替える。
fn overflow_case(child_styles: &[f32]) -> Vec<u8> {
    let mut styles: Vec<f32> = Vec::new();
    styles.extend(dim(TAG_WIDTH, 100.0));
    styles.extend(dim(TAG_HEIGHT, 100.0));
    let root_len = styles.len();
    styles.extend_from_slice(child_styles);

    let draws: Vec<f32> = vec![
        DRAW_OP_MOVE_TO as f32,
        5.0,
        5.0,
        DRAW_OP_LINE_TO as f32,
        80.0,
        5.0,
        DRAW_OP_LINE_TO as f32,
        80.0,
        80.0,
        DRAW_OP_LINE_TO as f32,
        5.0,
        80.0,
        DRAW_OP_CLOSE as f32,
        DRAW_OP_FILL as f32,
        5.0,
        DRAW_PAINT_COLOR as f32,
        0.0,
        0.0,
        1.0,
        1.0,
    ];

    let ops: Vec<f64> = vec![
        OP_CREATE as f64,
        1.0,
        ELEMENT_KIND_VIEW as f64,
        OP_SET_ROOT as f64,
        1.0,
        OP_SET_STYLE as f64,
        1.0,
        0.0,
        root_len as f64,
        OP_CREATE as f64,
        2.0,
        ELEMENT_KIND_VIEW as f64,
        OP_APPEND_CHILD as f64,
        1.0,
        2.0,
        OP_SET_STYLE as f64,
        2.0,
        root_len as f64,
        child_styles.len() as f64,
        OP_SET_DRAW as f64,
        2.0,
        0.0,
        draws.len() as f64,
    ];

    render_wire(&ops, &styles, &draws)
}

// `overflow` 既定（visible）: 40x40 の box 外への描画がそのまま描かれる
// （Flutter CustomPaint 既定と一致）。
#[test]
fn wire_draw_overflows_box_when_overflow_is_default_visible() {
    let mut child_styles: Vec<f32> = Vec::new();
    child_styles.extend(dim(TAG_WIDTH, 40.0));
    child_styles.extend(dim(TAG_HEIGHT, 40.0));
    // 子の box を可視化する薄いグレー背景（box 境界が golden で判読できるように）。
    child_styles.extend_from_slice(&[TAG_BACKGROUND_COLOR as f32, 0.9, 0.9, 0.9, 1.0]);

    let pixels = overflow_case(&child_styles);

    assert_near(
        pixel(&pixels, CANVAS_W, 20, 20),
        [0, 0, 255, 255],
        2,
        "inside the box",
    );
    assert_near(
        pixel(&pixels, CANVAS_W, 70, 70),
        [0, 0, 255, 255],
        2,
        "outside the 40x40 box (overflow: visible keeps the ink)",
    );

    assert_pixels_match_golden(
        &golden_path("draw_wire_overflow_visible"),
        &pixels,
        CANVAS_W,
        CANVAS_H,
    );
}

// 対照: `overflow: hidden` は draw を children ともども border box でクリップする。
#[test]
fn wire_draw_is_clipped_by_overflow_hidden() {
    let mut child_styles: Vec<f32> = Vec::new();
    child_styles.extend(dim(TAG_WIDTH, 40.0));
    child_styles.extend(dim(TAG_HEIGHT, 40.0));
    child_styles.extend_from_slice(&[TAG_OVERFLOW as f32, 1.0]); // hidden

    let pixels = overflow_case(&child_styles);

    assert_near(
        pixel(&pixels, CANVAS_W, 20, 20),
        [0, 0, 255, 255],
        2,
        "inside the box still paints",
    );
    assert_clear(
        pixel(&pixels, CANVAS_W, 70, 70),
        "outside the box is clipped by overflow: hidden",
    );
}
