//! `RenderImage` → `SkImage` の直接描画（`ScenePainter::draw_image`）。ElementTree の
//! layout を経由せず `SceneGraph` を直接組み立て、image ノードのピクセルをそのまま検証する。

mod support;

use std::sync::Arc;

use hayate_core::{Blob, Node, NodeKind, RenderImage, RenderImageAlphaType, RenderImageFormat, SceneGraph};
use support::{pixel, render_scene_to_pixels_scaled};

/// 2x2 の赤/緑/青/白 RGBA8 straight-alpha 画像。
fn two_by_two_image() -> RenderImage {
    let pixels: Vec<u8> = vec![
        255, 0, 0, 255, //
        0, 255, 0, 255, //
        0, 0, 255, 255, //
        255, 255, 255, 255,
    ];
    RenderImage {
        width: 2,
        height: 2,
        format: RenderImageFormat::Rgba8,
        alpha_type: RenderImageAlphaType::Alpha,
        data: Blob::from(pixels),
    }
}

#[test]
fn draw_image_scales_and_places_pixels() {
    let mut graph = SceneGraph::new();
    graph.insert(Node {
        kind: NodeKind::Image {
            x: 10.0,
            y: 10.0,
            width: 40.0,
            height: 40.0,
            data: Arc::new(two_by_two_image()),
        },
        children: Vec::new(),
    });

    let pixels = render_scene_to_pixels_scaled(&graph, 60, 60, 1.0);

    // 各 2x2 セルは 20x20 論理px へ拡大される。セル中心をサンプルする。
    let top_left = pixel(&pixels, 60, 20, 20); // 赤セル中心
    let top_right = pixel(&pixels, 60, 40, 20); // 緑セル中心
    let bottom_left = pixel(&pixels, 60, 20, 40); // 青セル中心
    assert!(top_left[0] > 200 && top_left[1] < 60, "top-left quadrant should be red, got {top_left:?}");
    assert!(top_right[1] > 200 && top_right[0] < 60, "top-right quadrant should be green, got {top_right:?}");
    assert!(bottom_left[2] > 200 && bottom_left[0] < 60, "bottom-left quadrant should be blue, got {bottom_left:?}");

    // 画像の外側はキャンバスの clear color(白)のまま。
    let outside = pixel(&pixels, 60, 55, 55);
    assert!(
        outside[0] > 240 && outside[1] > 240 && outside[2] > 240,
        "outside the image should stay clear, got {outside:?}"
    );
}
