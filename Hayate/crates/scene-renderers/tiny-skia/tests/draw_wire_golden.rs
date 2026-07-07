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
    apply_mutations, DRAW_OP_ARC_TO, DRAW_OP_CIRCLE, DRAW_OP_CLOSE, DRAW_OP_CUBIC_TO, DRAW_OP_FILL,
    DRAW_OP_LINE_TO, DRAW_OP_MOVE_TO, DRAW_OP_RECT, DRAW_OP_RRECT, DRAW_OP_STROKE, DRAW_PAINT_CAP,
    DRAW_PAINT_COLOR, DRAW_PAINT_DASH, DRAW_PAINT_FILL_RULE, DRAW_PAINT_JOIN, DRAW_PAINT_MITER_LIMIT,
    DRAW_PAINT_STROKE_WIDTH, ELEMENT_KIND_VIEW, OP_APPEND_CHILD, OP_CREATE, OP_SET_DRAW,
    OP_SET_ROOT, OP_SET_STYLE, TAG_BACKGROUND_COLOR, TAG_HEIGHT, TAG_OVERFLOW, TAG_WIDTH,
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

/// root 100x100 view に `draws` の display list を載せて 1 枚描く（#726 の曲線/形状/
/// fill rule golden 共通の足場）。
fn single_view_draw(draws: &[f32]) -> Vec<u8> {
    let mut styles: Vec<f32> = Vec::new();
    styles.extend(dim(TAG_WIDTH, 100.0));
    styles.extend(dim(TAG_HEIGHT, 100.0));
    let style_len = styles.len();
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
    render_wire(&ops, &styles, draws)
}

// 3 次ベジェで縁取った塗り（#726）。曲線動詞が painter まで通ることの golden。
#[test]
fn wire_cubic_fill_matches_golden() {
    let draws: Vec<f32> = vec![
        DRAW_OP_MOVE_TO as f32, 20.0, 50.0,
        DRAW_OP_CUBIC_TO as f32, 20.0, 10.0, 80.0, 10.0, 80.0, 50.0,
        DRAW_OP_CUBIC_TO as f32, 80.0, 90.0, 20.0, 90.0, 20.0, 50.0,
        DRAW_OP_CLOSE as f32,
        DRAW_OP_FILL as f32, 5.0, DRAW_PAINT_COLOR as f32, 1.0, 0.0, 0.0, 1.0,
    ];
    let pixels = single_view_draw(&draws);
    assert_near(pixel(&pixels, CANVAS_W, 50, 50), [255, 0, 0, 255], 2, "inside the cubic blob");
    assert_clear(pixel(&pixels, CANVAS_W, 3, 3), "outside the blob");
    assert_pixels_match_golden(&golden_path("draw_wire_cubic_fill"), &pixels, CANVAS_W, CANVAS_H);
}

// evenOdd の fill rule（#726）: 入れ子の矩形で内側が穴になる。同じジオメトリを
// 既定 nonZero で塗ると穴は塞がる — その差が evenOdd の観測点。
#[test]
fn wire_even_odd_fill_leaves_a_hole_where_nonzero_fills() {
    // 外 (20,20,60,60) + 内 (40,40,20,20) を evenOdd で塗る → 内側は穴。
    let even_odd: Vec<f32> = vec![
        DRAW_OP_RECT as f32, 20.0, 20.0, 60.0, 60.0,
        DRAW_OP_RECT as f32, 40.0, 40.0, 20.0, 20.0,
        DRAW_OP_FILL as f32, 7.0,
        DRAW_PAINT_COLOR as f32, 0.0, 0.0, 1.0, 1.0,
        DRAW_PAINT_FILL_RULE as f32, 1.0,
    ];
    let pixels = single_view_draw(&even_odd);
    assert_near(pixel(&pixels, CANVAS_W, 25, 25), [0, 0, 255, 255], 2, "the ring is filled");
    assert_clear(pixel(&pixels, CANVAS_W, 50, 50), "evenOdd punches a hole in the middle");
    assert_pixels_match_golden(&golden_path("draw_wire_even_odd_hole"), &pixels, CANVAS_W, CANVAS_H);

    // 対照: 同じジオメトリ・既定 nonZero では穴が塞がる。
    let non_zero: Vec<f32> = vec![
        DRAW_OP_RECT as f32, 20.0, 20.0, 60.0, 60.0,
        DRAW_OP_RECT as f32, 40.0, 40.0, 20.0, 20.0,
        DRAW_OP_FILL as f32, 5.0,
        DRAW_PAINT_COLOR as f32, 0.0, 0.0, 1.0, 1.0,
    ];
    let solid = single_view_draw(&non_zero);
    assert_near(pixel(&solid, CANVAS_W, 50, 50), [0, 0, 255, 255], 2, "nonZero fills the middle");
}

// 便宜形状（#726）: 角丸矩形と円を 1 呼び出しずつで塗る。角丸コーナー・円外は背景。
#[test]
fn wire_convenience_shapes_match_golden() {
    let draws: Vec<f32> = vec![
        // 左: 角丸矩形 (5,5,45,90) r=18 緑
        DRAW_OP_RRECT as f32, 5.0, 5.0, 45.0, 90.0, 18.0, 18.0,
        DRAW_OP_FILL as f32, 5.0, DRAW_PAINT_COLOR as f32, 0.0, 0.6, 0.0, 1.0,
        // 右: 円 中心 (75,50) r=20 赤
        DRAW_OP_CIRCLE as f32, 75.0, 50.0, 20.0,
        DRAW_OP_FILL as f32, 5.0, DRAW_PAINT_COLOR as f32, 1.0, 0.0, 0.0, 1.0,
    ];
    let pixels = single_view_draw(&draws);
    assert_near(pixel(&pixels, CANVAS_W, 27, 50), [0, 153, 0, 255], 3, "inside the rounded rect");
    assert_clear(pixel(&pixels, CANVAS_W, 6, 6), "the rounded corner is cut away");
    assert_near(pixel(&pixels, CANVAS_W, 75, 50), [255, 0, 0, 255], 2, "inside the circle");
    assert_clear(pixel(&pixels, CANVAS_W, 58, 33), "outside the circle");
    assert_pixels_match_golden(&golden_path("draw_wire_convenience_shapes"), &pixels, CANVAS_W, CANVAS_H);
}

// arcTo（#726）: 上辺と右辺に接する円弧で右上コーナーを丸めた正方形。内部は塗られ、
// 丸められたコーナーの外側は背景。
#[test]
fn wire_arc_to_rounds_a_corner_matches_golden() {
    let draws: Vec<f32> = vec![
        DRAW_OP_MOVE_TO as f32, 10.0, 90.0,
        DRAW_OP_LINE_TO as f32, 10.0, 10.0,
        DRAW_OP_ARC_TO as f32, 90.0, 10.0, 90.0, 90.0, 30.0,
        DRAW_OP_LINE_TO as f32, 90.0, 90.0,
        DRAW_OP_CLOSE as f32,
        DRAW_OP_FILL as f32, 5.0, DRAW_PAINT_COLOR as f32, 0.0, 0.4, 0.8, 1.0,
    ];
    let pixels = single_view_draw(&draws);
    assert_near(pixel(&pixels, CANVAS_W, 50, 50), [0, 102, 204, 255], 3, "inside the shape");
    assert_near(pixel(&pixels, CANVAS_W, 15, 15), [0, 102, 204, 255], 3, "the sharp top-left corner is filled");
    assert_clear(pixel(&pixels, CANVAS_W, 86, 14), "the arc-rounded top-right corner is cut away");
    assert_pixels_match_golden(&golden_path("draw_wire_arc_to_corner"), &pixels, CANVAS_W, CANVAS_H);
}

/// [STROKE, paint_len, ...paint] を組む（#727）。
fn stroke_cmd(paint: &[f32]) -> Vec<f32> {
    let mut v = vec![DRAW_OP_STROKE as f32, paint.len() as f32];
    v.extend_from_slice(paint);
    v
}

// cap 3 種（#727）: butt は端点で止まり、square / round は width/2 分だけ端点の外へ伸びる。
#[test]
fn wire_stroke_caps_match_golden() {
    let black = [DRAW_PAINT_COLOR as f32, 0.0, 0.0, 0.0, 1.0];
    let mut draws: Vec<f32> = Vec::new();
    for (row_y, cap) in [(20.0_f32, 0.0_f32), (50.0, 2.0), (80.0, 1.0)] {
        // 30..70 の太さ 12 の水平線。
        draws.extend([DRAW_OP_MOVE_TO as f32, 30.0, row_y, DRAW_OP_LINE_TO as f32, 70.0, row_y]);
        let mut paint = black.to_vec();
        paint.extend([DRAW_PAINT_STROKE_WIDTH as f32, 12.0, DRAW_PAINT_CAP as f32, cap]);
        draws.extend(stroke_cmd(&paint));
    }
    let pixels = single_view_draw(&draws);
    // 線の本体はどれも塗られる。
    assert_near(pixel(&pixels, CANVAS_W, 50, 20), [0, 0, 0, 255], 2, "butt line body");
    // 端点 (70) の 3px 外側: butt は無し、square / round は張り出す。
    assert_clear(pixel(&pixels, CANVAS_W, 73, 20), "butt cap stops at the endpoint");
    assert_near(pixel(&pixels, CANVAS_W, 73, 50), [0, 0, 0, 255], 2, "square cap extends past the endpoint");
    assert_near(pixel(&pixels, CANVAS_W, 72, 80), [0, 0, 0, 255], 2, "round cap extends past the endpoint");
    assert_pixels_match_golden(&golden_path("draw_wire_stroke_caps"), &pixels, CANVAS_W, CANVAS_H);
}

// join 3 種（#727）: 直角の外側コーナーで miter は尖り、bevel は削られる。
#[test]
fn wire_stroke_joins_match_golden() {
    let black = [DRAW_PAINT_COLOR as f32, 0.0, 0.0, 0.0, 1.0];
    let mut draws: Vec<f32> = Vec::new();
    // (miter, bevel, round) を x でずらして 3 つの L 字を描く。
    for (cx, join) in [(30.0_f32, 0.0_f32), (55.0, 2.0), (80.0, 1.0)] {
        draws.extend([
            DRAW_OP_MOVE_TO as f32, cx - 15.0, 40.0,
            DRAW_OP_LINE_TO as f32, cx, 40.0,
            DRAW_OP_LINE_TO as f32, cx, 55.0,
        ]);
        let mut paint = black.to_vec();
        paint.extend([
            DRAW_PAINT_STROKE_WIDTH as f32, 10.0,
            DRAW_PAINT_JOIN as f32, join,
            DRAW_PAINT_MITER_LIMIT as f32, 10.0,
        ]);
        draws.extend(stroke_cmd(&paint));
    }
    let pixels = single_view_draw(&draws);
    // 外側コーナー (cx+4, 36): miter は塗られ、bevel は削れて背景。
    assert_near(pixel(&pixels, CANVAS_W, 34, 36), [0, 0, 0, 255], 2, "miter join fills the sharp corner");
    assert_clear(pixel(&pixels, CANVAS_W, 59, 36), "bevel join cuts the corner");
    assert_pixels_match_golden(&golden_path("draw_wire_stroke_joins"), &pixels, CANVAS_W, CANVAS_H);
}

// dash（破線）+ 曲線パス（#727）: 上段は直線破線（on/off を厳密に検証）、下段は
// 破線を載せた 3 次ベジェ曲線（golden で模様を固定）。
#[test]
fn wire_stroke_dash_on_line_and_curve_matches_golden() {
    let mut draws: Vec<f32> = Vec::new();
    // dash [12,8]: on 12 / off 8。stroke width 5、黒。
    let dash_paint = |extra: &[f32]| -> Vec<f32> {
        let mut p = vec![
            DRAW_PAINT_COLOR as f32, 0.0, 0.0, 0.0, 1.0,
            DRAW_PAINT_STROKE_WIDTH as f32, 5.0,
            DRAW_PAINT_DASH as f32, 2.0, 12.0, 8.0,
        ];
        p.extend_from_slice(extra);
        p
    };
    // 上段: 直線破線 y=25, x 10..90。
    draws.extend([DRAW_OP_MOVE_TO as f32, 10.0, 25.0, DRAW_OP_LINE_TO as f32, 90.0, 25.0]);
    draws.extend(stroke_cmd(&dash_paint(&[])));
    // 下段: 破線を載せた曲線。
    draws.extend([
        DRAW_OP_MOVE_TO as f32, 10.0, 70.0,
        DRAW_OP_CUBIC_TO as f32, 35.0, 45.0, 65.0, 95.0, 90.0, 70.0,
    ]);
    draws.extend(stroke_cmd(&dash_paint(&[])));

    let pixels = single_view_draw(&draws);
    // 直線: on(10..22) / off(22..30) / on(30..42)。
    assert_near(pixel(&pixels, CANVAS_W, 14, 25), [0, 0, 0, 255], 2, "first dash is inked");
    assert_clear(pixel(&pixels, CANVAS_W, 26, 25), "the gap between dashes is empty");
    assert_near(pixel(&pixels, CANVAS_W, 36, 25), [0, 0, 0, 255], 2, "second dash is inked");
    // 曲線: 先頭の dash が塗られている。
    assert_near(pixel(&pixels, CANVAS_W, 11, 70), [0, 0, 0, 255], 3, "the dashed curve starts inked");
    assert_pixels_match_golden(&golden_path("draw_wire_stroke_dash"), &pixels, CANVAS_W, CANVAS_H);
}
