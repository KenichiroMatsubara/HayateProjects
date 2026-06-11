//! Content scale maps CSS layout coordinates to physical pixels (ADR-0007, #146).

use hayate_core::{Node, NodeKind, SceneGraph};
use hayate_scene_renderer_tiny_skia::TinySkiaSceneRenderer;
use tiny_skia::Pixmap;

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

const CLEAR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

#[test]
fn scale_1_paints_rect_at_css_pixel_extent() {
    let sg = red_rect_scene();
    let mut pixmap = Pixmap::new(100, 100).expect("pixmap");
    TinySkiaSceneRenderer::new().render_scene(&sg, &mut pixmap, CLEAR, 1.0);
    let inside = pixel(pixmap.data(), 100, 49, 25);
    let outside = pixel(pixmap.data(), 100, 51, 25);
    assert!(
        inside[0] > 200,
        "inside rect should be red, got {inside:?}"
    );
    assert!(
        outside[0] > 240 && outside[1] > 240,
        "outside rect should be clear, got {outside:?}"
    );
}

#[test]
fn scale_2_paints_rect_at_physical_pixel_extent() {
    let sg = red_rect_scene();
    let mut pixmap = Pixmap::new(200, 200).expect("pixmap");
    TinySkiaSceneRenderer::new().render_scene(&sg, &mut pixmap, CLEAR, 2.0);
    let inside = pixel(pixmap.data(), 200, 99, 50);
    let outside = pixel(pixmap.data(), 200, 101, 50);
    assert!(
        inside[0] > 200,
        "inside scaled rect should be red, got {inside:?}"
    );
    assert!(
        outside[0] > 240 && outside[1] > 240,
        "outside scaled rect should be clear, got {outside:?}"
    );
}
