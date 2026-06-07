use hayate_core::{
    DrawOp, Node, NodeKind, RecordingPainter, SceneGraph, render_scene_graph,
};

#[test]
fn walk_visits_roots_in_paint_order() {
    let mut scene = SceneGraph::new();
    scene.insert(Node {
        kind: NodeKind::Rect {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
            color: [1.0, 0.0, 0.0, 1.0],
            corner_radius: 0.0,
        },
        children: Vec::new(),
    });
    scene.insert(Node {
        kind: NodeKind::Rect {
            x: 2.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
            color: [0.0, 1.0, 0.0, 1.0],
            corner_radius: 0.0,
        },
        children: Vec::new(),
    });

    let mut painter = RecordingPainter::new();
    render_scene_graph(&scene, &mut painter);

    assert_eq!(painter.ops().len(), 2);
    assert!(matches!(
        painter.ops()[0],
        DrawOp::FillRect {
            color: [1.0, 0.0, 0.0, 1.0],
            ..
        }
    ));
    assert!(matches!(
        painter.ops()[1],
        DrawOp::FillRect {
            color: [0.0, 1.0, 0.0, 1.0],
            ..
        }
    ));
}
