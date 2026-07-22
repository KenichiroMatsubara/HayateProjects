use hayate_core::element::style::{Dimension, StyleProp};
use hayate_core::{
    render_scene_graph, Color, CommittedFrame, ElementKind, ElementTree, LayerScene,
    LayerSceneKind, RecordingPainter, SceneRead,
};

fn nested_layer_tree() -> (ElementTree, hayate_core::ElementId, hayate_core::ElementId) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let layer = tree.element_create(1, ElementKind::View);
    let nested = tree.element_create(2, ElementKind::View);
    tree.element_append_child(root, layer);
    tree.element_append_child(layer, nested);
    tree.set_root(root);
    tree.set_viewport(100.0, 100.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(100.0)),
            StyleProp::BackgroundColor(Color::new(0.1, 0.2, 0.3, 1.0)),
        ],
    );
    tree.element_set_style(
        layer,
        &[
            StyleProp::Width(Dimension::px(40.0)),
            StyleProp::Height(Dimension::px(40.0)),
            StyleProp::BackgroundColor(Color::new(0.8, 0.2, 0.1, 1.0)),
        ],
    );
    tree.element_set_style(
        nested,
        &[
            StyleProp::Width(Dimension::px(10.0)),
            StyleProp::Height(Dimension::px(10.0)),
            StyleProp::BackgroundColor(Color::new(0.2, 0.8, 0.1, 1.0)),
        ],
    );
    tree.element_set_transform(layer, Some([1.0, 0.0, 0.0, 1.0, 4.0, 0.0]));
    tree.element_set_transform(nested, Some([1.0, 0.0, 0.0, 1.0, 2.0, 0.0]));
    (tree, layer, nested)
}

#[test]
fn committed_frame_is_owned_and_carries_layer_topology() {
    fn assert_owned(_: CommittedFrame) {}

    let (mut tree, layer, nested) = nested_layer_tree();
    let frame = tree.commit_rendered_frame(0.0);
    let topology = frame.layer_topology();

    assert_eq!(topology.paint_order(), frame.layer_topology().paint_order());
    assert_eq!(
        topology.parent_of(layer),
        topology.paint_order().first().copied()
    );
    assert_eq!(topology.parent_of(nested), Some(layer));
    assert!(topology.structural_changed().contains(&layer));
    assert!(topology.geometry_changed().contains(&nested));
    assert_owned(frame);
}

#[test]
fn layer_scene_shares_snapshot_nodes_and_excludes_nested_layers() {
    let (mut tree, layer, nested) = nested_layer_tree();
    let frame = tree.commit_rendered_frame(0.0);
    let scene = LayerScene::new(
        frame.snapshot().clone(),
        frame.layer_topology().clone(),
        layer,
        LayerSceneKind::Content,
    )
    .expect("committed layer has a projection");

    let shared = scene
        .roots()
        .iter()
        .find_map(|root| scene.get(*root).map(|node| (*root, node)))
        .expect("layer has content");
    assert!(std::ptr::eq(
        shared.1,
        frame.snapshot().get(shared.0).unwrap()
    ));
    assert!(scene
        .get(frame.snapshot().anchor_of(nested).unwrap())
        .is_none());

    let mut painter = RecordingPainter::new();
    render_scene_graph(&scene, &mut painter);
    assert!(!painter.ops().is_empty());
}
