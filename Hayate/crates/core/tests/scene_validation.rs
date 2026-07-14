use hayate_core::{validate_scene_graph, Node, NodeKind, SceneGraph, SceneValidationError};
use slotmap::Key;

fn rect() -> Node {
    Node {
        kind: NodeKind::Rect {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
            color: [1.0; 4],
            corner_radius: 0.0,
        },
        children: Vec::new(),
    }
}

#[test]
fn invalid_scene_graph_has_a_renderer_independent_contract_error() {
    let mut graph = SceneGraph::new();
    let root = graph.insert(rect());
    graph
        .get_mut(root)
        .expect("root exists")
        .children
        .push(hayate_core::NodeId::null());

    assert!(matches!(
        validate_scene_graph(&graph),
        Err(SceneValidationError::MissingChild { parent, child }) if parent == root && child.is_null(),
    ));
}
