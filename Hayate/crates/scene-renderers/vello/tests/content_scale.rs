//! content scale は CSS レイアウト座標を物理ピクセルへ写像する（ADR-0007）。

use hayate_core::{Node, NodeKind, SceneGraph};
use hayate_scene_test_support::vello::{render_scene_to_pixels_scaled, try_vello_harness};

fn red_rect_scene() -> SceneGraph {
    let mut scene = SceneGraph::new();
    scene.insert(Node {
        kind: NodeKind::Rect {
            x: 0.0,
            y: 0.0,
            width: 50.0,
            height: 50.0,
            color: [1.0, 0.0, 0.0, 1.0],
            corner_radius: 0.0,
        },
        children: Vec::new(),
    });
    scene
}

fn pixel(data: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let idx = ((y * width + x) * 4) as usize;
    [data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]
}

#[test]
fn scale_1_paints_rect_at_css_pixel_extent() {
    let Some(mut harness) = try_vello_harness() else {
        eprintln!("skip scale_1: no wgpu adapter");
        return;
    };
    let pixels = render_scene_to_pixels_scaled(&mut harness, &red_rect_scene(), 100, 100, 1.0)
        .expect("vello render");
    let inside = pixel(&pixels, 100, 49, 25);
    let outside = pixel(&pixels, 100, 51, 25);
    assert!(inside[0] > 200, "inside rect should be red, got {inside:?}");
    assert!(
        outside[0] > 240 && outside[1] > 240,
        "outside rect should be clear, got {outside:?}"
    );
}

#[test]
fn scale_2_paints_rect_at_physical_pixel_extent() {
    let Some(mut harness) = try_vello_harness() else {
        eprintln!("skip scale_2: no wgpu adapter");
        return;
    };
    let pixels = render_scene_to_pixels_scaled(&mut harness, &red_rect_scene(), 200, 200, 2.0)
        .expect("vello render");
    let inside = pixel(&pixels, 200, 99, 50);
    let outside = pixel(&pixels, 200, 101, 50);
    assert!(
        inside[0] > 200,
        "inside scaled rect should be red, got {inside:?}"
    );
    assert!(
        outside[0] > 240 && outside[1] > 240,
        "outside scaled rect should be clear, got {outside:?}"
    );
}
