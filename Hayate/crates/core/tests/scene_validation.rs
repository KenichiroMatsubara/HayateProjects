use hayate_core::{
    validate_scene_graph, Color, ElementKind, ElementTree, Node, NodeKind, SceneGraph,
    SceneGraphValidator, SceneValidationError, StyleProp,
};
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

#[test]
fn retained_validation_visits_only_changed_subtrees_and_skips_clean_frames() {
    let mut graph = SceneGraph::new();
    let left = graph.insert(rect());
    let right = graph.insert(rect());
    let left_child = graph.insert_child(left, rect());
    graph.insert_child(right, rect());
    let mut validator = SceneGraphValidator::new();

    let initial = validator
        .validate(&graph, std::iter::empty())
        .expect("initial graph is valid");
    assert_eq!(
        initial.visited_nodes(),
        4,
        "cold validation walks the full graph"
    );

    graph.get_mut(left_child).expect("child exists").kind = NodeKind::Rect {
        x: 1.0,
        y: 2.0,
        width: 3.0,
        height: 4.0,
        color: [0.5; 4],
        corner_radius: 0.0,
    };
    let changed = validator
        .validate(&graph, [left])
        .expect("changed subtree is valid");
    assert_eq!(
        changed.visited_nodes(),
        2,
        "only the left subtree is revisited"
    );

    let clean = validator
        .validate(&graph, std::iter::empty())
        .expect("an unchanged frame stays valid without work");
    assert_eq!(
        clean.visited_nodes(),
        0,
        "clean frames do not run validation"
    );
}

#[test]
fn invalid_draw_instruction_has_a_renderer_independent_contract_error() {
    let mut graph = SceneGraph::new();
    let invalid = graph.insert(Node {
        kind: NodeKind::Rect {
            x: f32::NAN,
            y: 0.0,
            width: 1.0,
            height: 1.0,
            color: [1.0; 4],
            corner_radius: 0.0,
        },
        children: Vec::new(),
    });

    assert_eq!(
        validate_scene_graph(&graph),
        Err(SceneValidationError::InvalidCommand { node: invalid }),
    );
}

#[test]
fn element_tree_connects_retained_dirty_tracking_to_incremental_validation() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let left = tree.element_create(1, ElementKind::View);
    let right = tree.element_create(2, ElementKind::View);
    tree.element_append_child(root, left);
    tree.element_append_child(root, right);
    tree.set_root(root);

    tree.render(0.0);
    let initial = tree.test_scene_validation_visited_nodes();
    assert!(initial >= 3, "the cold retained scene is fully validated");

    tree.element_set_style(left, &[StyleProp::BackgroundColor(Color::WHITE)]);
    tree.render(16.0);
    let changed = tree.test_scene_validation_visited_nodes();
    assert!(changed > 0, "the changed subtree is validated");
    assert!(changed < initial, "an unchanged sibling is not revisited");

    tree.render(32.0);
    assert_eq!(tree.test_scene_validation_visited_nodes(), 0);
}
