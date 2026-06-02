use hayate_core::{Node, NodeKind, NullBackend, RecordingBackend, SceneGraph};

#[test]
fn recording_backend_keeps_rendered_scene() {
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

    let mut backend = RecordingBackend::new();
    backend.render(&scene, [0.0, 0.0, 0.0, 1.0]);

    assert_eq!(backend.frames().len(), 1);
    assert_eq!(backend.frames()[0].scene.len(), 1);
}

#[test]
fn null_backend_accepts_render_and_clear() {
    let scene = SceneGraph::new();
    let mut backend = NullBackend::new();
    backend.render(&scene, [0.0, 0.0, 0.0, 1.0]);
    backend.clear([1.0, 1.0, 1.0, 1.0]);
}
