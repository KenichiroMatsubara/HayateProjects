//! 絶対色プローブ: SceneGraph の色 [f32;4]（sRGB 値）が raster 経路で
//! バイト値そのまま（sRGB エンコードそのまま・再エンコードなし）で出ることを固定する。
//! ガンマ二重適用（薄くなる: 0.5 → ≈0.735）や linear 誤解釈（濃くなる）を検出する。

mod support;

use hayate_core::{Node, NodeKind, SceneGraph};
use support::{pixel, render_scene_to_pixels};

fn solid_rect_scene(color: [f32; 4]) -> SceneGraph {
    let mut sg = SceneGraph::new();
    sg.insert(Node {
        kind: NodeKind::Rect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
            color,
            corner_radius: 0.0,
        },
        children: Vec::new(),
    });
    sg
}

fn assert_center_rgb(color: [f32; 4], expect: [u8; 3]) {
    let pixels = render_scene_to_pixels(&solid_rect_scene(color));
    let px = pixel(&pixels, 100, 50, 50);
    for i in 0..3 {
        assert!(
            (px[i] as i16 - expect[i] as i16).abs() <= 1,
            "channel {i}: scene color {color:?} should raster to {expect:?} (±1), got {px:?} — \
             値が大きければガンマ二重適用（薄い）、小さければ linear 誤解釈（濃い）"
        );
    }
}

#[test]
fn mid_gray_is_not_washed_out() {
    // 0.5 sRGB → 128。二重エンコードなら 188 前後、linear 誤解釈なら 55 前後。
    assert_center_rgb([0.5, 0.5, 0.5, 1.0], [128, 128, 128]);
}

#[test]
fn mid_tones_pass_through_exactly() {
    assert_center_rgb([0.2, 0.4, 0.8, 1.0], [51, 102, 204]);
    assert_center_rgb([1.0, 0.0, 0.0, 1.0], [255, 0, 0]);
}
