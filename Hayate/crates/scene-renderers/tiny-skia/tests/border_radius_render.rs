use hayate_core::{Node, NodeKind, SceneGraph};
use hayate_scene_renderer_tiny_skia::TinySkiaSceneRenderer;
use tiny_skia::Pixmap;

fn pixel(pixmap: &Pixmap, x: u32, y: u32) -> [u8; 4] {
    let idx = (y * pixmap.width() + x) as usize * 4;
    let data = pixmap.data();
    [data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]
}

#[test]
fn rounded_rect_leaves_corner_pixels_clear() {
    let mut scene = SceneGraph::new();
    scene.insert(Node {
        kind: NodeKind::Rect {
            x: 8.0,
            y: 8.0,
            width: 48.0,
            height: 48.0,
            color: [1.0, 0.0, 0.0, 1.0],
            corner_radius: 12.0,
        },
        children: Vec::new(),
    });

    let mut pixmap = Pixmap::new(64, 64).unwrap();
    TinySkiaSceneRenderer::new().render_scene(&scene, &mut pixmap, [1.0, 1.0, 1.0, 1.0], 1.0);

    assert_eq!(
        pixel(&pixmap, 9, 9),
        [255, 255, 255, 255],
        "rounded corner should expose the clear color"
    );
    assert!(
        pixel(&pixmap, 32, 32)[0] > 200 && pixel(&pixmap, 32, 32)[3] > 200,
        "center should be filled"
    );
}

#[test]
fn rounded_ring_paints_band_without_erasing_center() {
    // A RoundedRing paints only its band; it must NOT clear its interior. The
    // native focus ring (#335) is a RoundedRing drawn on top of a filled box, so
    // hollowing the centre with a Clear would punch the content transparent
    // (issue #337). Here the ring sits over the opaque clear colour, which must
    // survive intact at the centre.
    let mut scene = SceneGraph::new();
    scene.insert(Node {
        kind: NodeKind::RoundedRing {
            x: 8.0,
            y: 8.0,
            width: 48.0,
            height: 48.0,
            outer_radius: 12.0,
            border_width: 4.0,
            color: [0.0, 0.0, 1.0, 1.0],
        },
        children: Vec::new(),
    });

    let mut pixmap = Pixmap::new(64, 64).unwrap();
    TinySkiaSceneRenderer::new().render_scene(&scene, &mut pixmap, [1.0, 1.0, 1.0, 1.0], 1.0);

    assert_eq!(
        pixel(&pixmap, 32, 32),
        [255, 255, 255, 255],
        "ring centre retains the background — the band must not erase what it overlays"
    );
    assert!(
        pixel(&pixmap, 20, 8)[2] > 200,
        "top border should be painted blue"
    );
}

#[test]
fn dashed_border_leaves_gaps_along_the_edge() {
    let mut scene = SceneGraph::new();
    scene.insert(Node {
        kind: NodeKind::DashedBorder {
            x: 8.0,
            y: 8.0,
            width: 48.0,
            height: 48.0,
            outer_radius: 0.0,
            border_width: 4.0,
            color: [0.0, 0.0, 1.0, 1.0],
        },
        children: Vec::new(),
    });

    let mut pixmap = Pixmap::new(64, 64).unwrap();
    TinySkiaSceneRenderer::new().render_scene(&scene, &mut pixmap, [1.0, 1.0, 1.0, 1.0], 1.0);

    // The center is empty (the border is only a stroked perimeter), so it keeps
    // the white clear color rather than the blue border color.
    assert!(
        pixel(&pixmap, 32, 32)[0] > 200,
        "dashed border must not fill the box interior"
    );

    // Scan the top border band (y ≈ 10): a dashed edge has BOTH blue dashes and
    // white gaps, unlike a solid border which would be painted continuously.
    let mut dashes = 0;
    let mut gaps = 0;
    for x in 9..55 {
        let px = pixel(&pixmap, x, 10);
        if px[2] > 200 && px[0] < 60 {
            dashes += 1;
        } else if px[0] > 200 && px[1] > 200 && px[2] > 200 {
            gaps += 1;
        }
    }
    assert!(dashes > 0, "dashed border must paint blue dashes on the top edge");
    assert!(
        gaps > 0,
        "dashed border must leave white gaps between dashes on the top edge"
    );
}

#[test]
fn border_and_background_renders_rounded_frame() {
    let mut scene = SceneGraph::new();
    scene.insert(Node {
        kind: NodeKind::Rect {
            x: 8.0,
            y: 8.0,
            width: 48.0,
            height: 48.0,
            color: [0.0, 0.0, 1.0, 1.0],
            corner_radius: 12.0,
        },
        children: Vec::new(),
    });
    scene.insert(Node {
        kind: NodeKind::Rect {
            x: 12.0,
            y: 12.0,
            width: 40.0,
            height: 40.0,
            color: [1.0, 0.0, 0.0, 1.0],
            corner_radius: 8.0,
        },
        children: Vec::new(),
    });

    let mut pixmap = Pixmap::new(64, 64).unwrap();
    TinySkiaSceneRenderer::new().render_scene(&scene, &mut pixmap, [1.0, 1.0, 1.0, 1.0], 1.0);

    assert_eq!(
        pixel(&pixmap, 9, 9),
        [255, 255, 255, 255],
        "outer rounded corner should remain clear"
    );
    assert!(
        pixel(&pixmap, 32, 32)[0] > 200,
        "inner background should be visible at center"
    );
    assert!(
        pixel(&pixmap, 20, 8)[2] > 200,
        "border frame should remain visible on the top edge"
    );
}
