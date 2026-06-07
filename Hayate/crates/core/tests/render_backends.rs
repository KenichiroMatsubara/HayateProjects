use hayate_core::{
    DrawOp, Node, NodeKind, NullPainter, RecordingPainter, SceneGraph, SceneRecorder,
    render_scene_graph,
};

#[test]
fn recording_painter_records_rect() {
    let mut scene = SceneGraph::new();
    scene.insert(Node {
        kind: NodeKind::Rect {
            x: 1.0,
            y: 2.0,
            width: 3.0,
            height: 4.0,
            color: [0.1, 0.2, 0.3, 1.0],
            corner_radius: 0.0,
        },
        children: Vec::new(),
    });

    let mut painter = RecordingPainter::new();
    render_scene_graph(&scene, &mut painter);

    assert_eq!(painter.ops().len(), 1);
    assert!(matches!(
        painter.ops()[0],
        DrawOp::FillRect {
            x: 1.0,
            y: 2.0,
            width: 3.0,
            height: 4.0,
            color: [0.1, 0.2, 0.3, 1.0],
            corner_radius: 0.0,
        }
    ));
}

#[test]
fn recording_painter_records_group_and_clip_nesting() {
    let mut scene = SceneGraph::new();
    let group_id = scene.insert(Node {
        kind: NodeKind::Group {
            transform: [1.0, 0.0, 0.0, 1.0, 10.0, 20.0],
        },
        children: Vec::new(),
    });
    let clip_id = scene.insert_child(
        group_id,
        Node {
            kind: NodeKind::Clip {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 50.0,
            },
            children: Vec::new(),
        },
    );
    let _ = scene.insert_child(
        clip_id,
        Node {
            kind: NodeKind::Rect {
                x: 5.0,
                y: 6.0,
                width: 7.0,
                height: 8.0,
                color: [1.0, 0.0, 0.0, 1.0],
                corner_radius: 0.0,
            },
            children: Vec::new(),
        },
    );

    let mut painter = RecordingPainter::new();
    render_scene_graph(&scene, &mut painter);

    assert_eq!(painter.ops().len(), 5);
    assert!(matches!(
        painter.ops()[0],
        DrawOp::PushTransform {
            transform: [1.0, 0.0, 0.0, 1.0, 10.0, 20.0],
        }
    ));
    assert!(matches!(
        painter.ops()[1],
        DrawOp::PushClipRect {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 50.0,
        }
    ));
    assert!(matches!(
        painter.ops()[2],
        DrawOp::FillRect {
            x: 5.0,
            y: 6.0,
            width: 7.0,
            height: 8.0,
            color: [1.0, 0.0, 0.0, 1.0],
            corner_radius: 0.0,
        }
    ));
    assert!(matches!(painter.ops()[3], DrawOp::PopClip));
    assert!(matches!(painter.ops()[4], DrawOp::PopTransform));
}

#[test]
fn scene_recorder_stores_walk_ops() {
    let mut scene = SceneGraph::new();
    scene.insert(Node {
        kind: NodeKind::Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
            color: [0.0, 0.0, 0.0, 1.0],
            corner_radius: 0.0,
        },
        children: Vec::new(),
    });

    let mut recorder = SceneRecorder::new();
    recorder.record(&scene, [0.0, 0.0, 0.0, 1.0]);

    assert_eq!(recorder.frames().len(), 1);
    assert_eq!(recorder.frames()[0].ops.len(), 1);
}

#[test]
fn null_painter_accepts_walk() {
    let scene = SceneGraph::new();
    let mut painter = NullPainter;
    render_scene_graph(&scene, &mut painter);
}

#[test]
fn recording_painter_records_rounded_ring() {
    let mut scene = SceneGraph::new();
    scene.insert(Node {
        kind: NodeKind::RoundedRing {
            x: 4.0,
            y: 4.0,
            width: 40.0,
            height: 30.0,
            outer_radius: 8.0,
            border_width: 2.0,
            color: [0.0, 0.0, 1.0, 1.0],
        },
        children: Vec::new(),
    });

    let mut painter = RecordingPainter::new();
    render_scene_graph(&scene, &mut painter);

    assert_eq!(painter.ops().len(), 1);
    assert!(matches!(
        painter.ops()[0],
        DrawOp::FillRoundedRing {
            x: 4.0,
            y: 4.0,
            width: 40.0,
            height: 30.0,
            outer_radius: 8.0,
            border_width: 2.0,
            color: [0.0, 0.0, 1.0, 1.0],
        }
    ));
}

#[test]
fn recording_painter_records_rounded_rect_corner_radius() {
    let mut scene = SceneGraph::new();
    scene.insert(Node {
        kind: NodeKind::Rect {
            x: 0.0,
            y: 0.0,
            width: 20.0,
            height: 20.0,
            color: [1.0, 0.0, 0.0, 1.0],
            corner_radius: 6.0,
        },
        children: Vec::new(),
    });

    let mut painter = RecordingPainter::new();
    render_scene_graph(&scene, &mut painter);

    assert!(matches!(
        painter.ops()[0],
        DrawOp::FillRect {
            corner_radius: 6.0,
            ..
        }
    ));
}
