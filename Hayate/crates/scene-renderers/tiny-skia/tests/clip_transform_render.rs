use hayate_core::{Node, NodeKind, SceneGraph};
use hayate_scene_renderer_tiny_skia::TinySkiaSceneRenderer;
use tiny_skia::Pixmap;

fn pixel(pixmap: &Pixmap, x: u32, y: u32) -> [u8; 4] {
    let idx = (y * pixmap.width() + x) as usize * 4;
    let data = pixmap.data();
    [data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]
}

/// A clip rect emitted while a translation is active (as happens for a
/// scrolled scroll-view containing a nested scroll-view) must be clipped in
/// the *transformed* coordinate space, matching where its content actually
/// paints — not in raw local coordinates.
#[test]
fn clip_rect_follows_active_transform() {
    let mut scene = SceneGraph::new();

    let group = scene.insert(Node {
        kind: NodeKind::Group {
            transform: [1.0, 0.0, 0.0, 1.0, -10.0, -10.0],
        },
        children: Vec::new(),
    });

    let clip = scene.insert_child(
        group,
        Node {
            kind: NodeKind::Clip {
                x: 20.0,
                y: 20.0,
                width: 30.0,
                height: 30.0,
            },
            children: Vec::new(),
        },
    );

    scene.insert_child(
        clip,
        Node {
            kind: NodeKind::Rect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
                color: [1.0, 0.0, 0.0, 1.0],
                corner_radius: 0.0,
            },
            children: Vec::new(),
        },
    );

    let mut pixmap = Pixmap::new(64, 64).unwrap();
    TinySkiaSceneRenderer::new().render_scene(&scene, &mut pixmap, [1.0, 1.0, 1.0, 1.0], 1.0);

    // Local clip rect [20,50]x[20,50] translated by (-10,-10) lands at
    // [10,40]x[10,40] in pixmap space — that's where the red fill should
    // actually be visible.
    assert_eq!(
        pixel(&pixmap, 15, 15),
        [255, 0, 0, 255],
        "content should be visible inside the transformed clip rect"
    );
    assert_eq!(
        pixel(&pixmap, 45, 45),
        [255, 255, 255, 255],
        "content outside the transformed clip rect must stay clipped"
    );
}
