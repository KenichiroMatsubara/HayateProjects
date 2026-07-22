use hayate_core::{ElementId, Node, NodeKind, SceneGraph};

fn group() -> Node {
    Node {
        kind: NodeKind::Group {
            transform: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        },
        children: Vec::new(),
    }
}

#[test]
fn committed_snapshot_is_isolated_from_later_scene_mutation() {
    let mut scene = SceneGraph::new();
    let parent = scene.insert(group());
    let snapshot = scene.snapshot();

    scene
        .get_mut(parent)
        .expect("retained parent")
        .children
        .push(parent);

    assert!(snapshot
        .get(parent)
        .expect("snapshot parent")
        .children
        .is_empty());
    assert_eq!(scene.get(parent).expect("live parent").children, [parent]);
}

#[test]
fn snapshot_commit_work_is_proportional_to_the_changed_nodes() {
    let mut scene = SceneGraph::new();
    let nodes: Vec<_> = (0..1_024).map(|_| scene.insert(group())).collect();
    let _baseline = scene.snapshot();

    scene
        .get_mut(nodes[511])
        .expect("changed node")
        .children
        .push(nodes[0]);
    let changed = scene.snapshot();

    assert_eq!(changed.commit_stats().changed_nodes(), 1);
    assert_eq!(changed.commit_stats().storage_entries_written(), 1);
}

#[test]
fn snapshot_resolves_parent_and_element_anchor_from_internal_indexes() {
    let mut scene = SceneGraph::new();
    let parent = scene.insert(group());
    let element = ElementId::from_u64(42);
    let anchor = scene.insert_child(
        parent,
        Node {
            kind: NodeKind::ElementAnchor {
                element_id: element,
            },
            children: Vec::new(),
        },
    );

    let snapshot = scene.snapshot();

    assert_eq!(snapshot.parent_of(anchor), Some(parent));
    assert_eq!(snapshot.anchor_of(element), Some(anchor));
}

#[test]
fn change_journal_classifies_geometry_structure_and_deletion_with_stable_ids() {
    let mut scene = SceneGraph::new();
    let parent = scene.insert(group());
    let removed = scene.insert_child(parent, group());
    let _baseline = scene.snapshot();

    if let NodeKind::Group { transform } = &mut scene.get_mut(parent).unwrap().kind {
        transform[4] = 12.0;
    }
    scene.remove(removed).expect("remove retained child");
    let replacement = scene.insert(group());
    let snapshot = scene.snapshot();

    assert!(snapshot.changes().geometry_nodes().contains(&parent));
    assert!(snapshot.changes().structural_nodes().contains(&parent));
    assert!(snapshot.changes().deleted_nodes().contains(&removed));
    assert_ne!(
        replacement, removed,
        "retired IDs must not alias live nodes"
    );
}

#[test]
fn snapshot_supports_concurrent_reads_while_the_scene_advances() {
    let mut scene = SceneGraph::new();
    let node = scene.insert(group());
    let snapshot = scene.snapshot();
    let reader = snapshot.clone();

    let read = std::thread::spawn(move || reader.get(node).unwrap().children.clone());
    scene.get_mut(node).unwrap().children.push(node);

    assert!(read.join().expect("snapshot reader").is_empty());
    assert_eq!(scene.get(node).unwrap().children, [node]);
}
