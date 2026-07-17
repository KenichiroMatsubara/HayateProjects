//! Skia バックエンドでのカラー絵文字グリフ描画（ADR-0146 §4）。
//!
//! `TextRun` が `SkTextBlob` 経由で描画され、Skia の scaler がカラーグリフを検出して
//! カラーで塗ることを検証する。tiny-skia（アウトラインのみ・単色に潰れる）とは対照的に、
//! Skia は Vello に続きカラーグリフを描けるレンダラになる（ADR-0146 の主動機）。
//!
//! フォントは `sbix`（埋め込み PNG ビットマップストライク）の小さなテストフォントを使う
//! （出自は `tests/assets/PROVENANCE.md`）。共有 `hayate-scene-test-support::cases::
//! color_glyph_tree` の COLRv1 グラデーションフォントではなく本フォントを使う理由も
//! 同ファイルに記録した — crates.io の skia-safe `=0.99.0` プリビルドは CPU raster
//! canvas で COLRv1 のペイントグラフを評価せず単色アウトラインへ後退することを実機確認した
//! （テーブル自体は検出できる）。ビットマップカラーグリフ（sbix・CBDT/CBLC）は同じ
//! painter コードパスでカラーのまま出る。
//!
//! 判定は「支配的色相が何種類あるか」ではなく「インクに彩度（グレースケールからの
//! 乖離）があるか」——本フォントの絵文字は黄〜橙の単色顔（複数色相の虹ではない）なので、
//! 色相バケット数ではなく彩度の有無がカラーグリフ判定の正しい芯になる。

mod support;

use hayate_core::{Color, Dimension, ElementKind, ElementTree, StyleProp};
use support::{pixel, render_scene_to_pixels, CANVAS_H, CANVAS_W};

static SBIX_EMOJI_BYTES: &[u8] = include_bytes!("assets/twemoji_smiley_sbix.ttf");
const FAMILY: &str = "Twemoji Smiley Sbix";
/// U+1F601 😁 — 本フォントの cmap が持つ実グリフ（`PROVENANCE.md` 参照）。
const EMOJI_CODEPOINT: char = '\u{1F601}';

fn emoji_tree() -> ElementTree {
    let mut tree = ElementTree::new();
    tree.register_font(FAMILY, SBIX_EMOJI_BYTES.to_vec());
    let root = tree.element_create(1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(CANVAS_W as f32, CANVAS_H as f32);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(CANVAS_W as f32)),
            StyleProp::Height(Dimension::px(CANVAS_H as f32)),
            StyleProp::BackgroundColor(Color::new(1.0, 1.0, 1.0, 1.0)),
        ],
    );
    let text = tree.element_create(2, ElementKind::Text);
    tree.element_append_child(root, text);
    tree.element_set_style(
        text,
        &[
            StyleProp::FontFamily(FAMILY.to_string()),
            StyleProp::FontSize(80.0),
            // ビットマップグリフはカラー版だけを持つ (paint 色はカラーグリフには使われない
            // ことの回帰確認も兼ねる: 塗り色を黒にしても出力は黒くならないはず)。
            StyleProp::Color(Color::new(0.0, 0.0, 0.0, 1.0)),
        ],
    );
    tree.element_set_text(text, &EMOJI_CODEPOINT.to_string());
    tree
}

/// ピクセルが「無彩色」でない最大の彩度（RGB の max-min 幅）。グレー/黒/白は 0 に近く、
/// 彩度のあるインクは大きい。
fn max_saturation(data: &[u8]) -> u8 {
    let mut best = 0u8;
    for y in 0..CANVAS_H {
        for x in 0..CANVAS_W {
            let px = pixel(data, CANVAS_W, x, y);
            let max = px[0].max(px[1]).max(px[2]);
            let min = px[0].min(px[1]).min(px[2]);
            best = best.max(max.saturating_sub(min));
        }
    }
    best
}

#[test]
fn skia_paints_color_emoji_glyph_with_saturated_ink() {
    let mut tree = emoji_tree();
    let graph = tree.render(0.0).clone();
    let pixels = render_scene_to_pixels(&graph);
    let sat = max_saturation(&pixels);
    assert!(
        sat > 100,
        "skia must paint the sbix emoji glyph in colour (expected saturated ink, got max R-B spread {sat}); \
         a near-zero spread means the colour glyph fell back to the (black) paint colour"
    );
}

/// 対照: 通常の白黒テキストは無彩色のまま（回帰の芯 — 彩度判定ロジック自体が常に
/// 彩度ありを返すバグを検出する）。
#[test]
fn skia_does_not_fabricate_saturation_for_plain_black_text() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(CANVAS_W as f32, CANVAS_H as f32);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(CANVAS_W as f32)),
            StyleProp::Height(Dimension::px(CANVAS_H as f32)),
            StyleProp::BackgroundColor(Color::new(1.0, 1.0, 1.0, 1.0)),
        ],
    );
    let text = tree.element_create(2, ElementKind::Text);
    tree.element_append_child(root, text);
    tree.element_set_style(
        text,
        &[
            StyleProp::FontSize(60.0),
            StyleProp::Color(Color::new(0.0, 0.0, 0.0, 1.0)),
        ],
    );
    tree.element_set_text(text, "A");
    let graph = tree.render(0.0).clone();
    let pixels = render_scene_to_pixels(&graph);
    let sat = max_saturation(&pixels);
    assert!(
        sat < 20,
        "plain black text must stay near-grayscale, got max spread {sat}"
    );
}
