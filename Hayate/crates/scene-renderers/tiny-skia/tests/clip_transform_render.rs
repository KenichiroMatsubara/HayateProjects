use hayate_core::{Node, NodeKind, SceneGraph};
use hayate_scene_renderer_tiny_skia::TinySkiaSceneRenderer;
use tiny_skia::Pixmap;

fn pixel(pixmap: &Pixmap, x: u32, y: u32) -> [u8; 4] {
    let idx = (y * pixmap.width() + x) as usize * 4;
    let data = pixmap.data();
    [data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]
}

/// 平行移動が有効な状態で発行されたクリップ矩形（ネストした scroll-view を含む
/// スクロール済み scroll-view で発生）は、生のローカル座標ではなく変換後の座標空間で
/// クリップしなければならない。実際に内容が描画される位置と一致させるため。
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
                corner_radii: [0.0; 4],
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

    // ローカルのクリップ矩形 [20,50]x[20,50] を (-10,-10) 平行移動すると
    // pixmap 空間で [10,40]x[10,40] に来る。そこに赤い塗りが見えるはず。
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
