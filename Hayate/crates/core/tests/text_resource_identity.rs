use std::sync::Arc;

use hayate_core::{
    Blob, ElementKind, ElementTree, FontInstanceId, Node, NodeKind, RenderFont, RenderGlyph,
    ResourceLookupError, SceneGraph, TextFontAttributes, TextResourcePolicy, TextRunData,
    TextRunId, TextSynthesis,
};

fn text_run(font: RenderFont, text: &str) -> TextRunData {
    TextRunData {
        font,
        font_size: 16.0,
        font_attributes: TextFontAttributes::default(),
        glyphs: vec![RenderGlyph {
            id: 42,
            x: 1.0,
            y: 2.0,
        }],
        decorations: Vec::new(),
        text: Arc::from(text),
        synthesis: TextSynthesis::default(),
        normalized_coords: Vec::new(),
    }
}

#[test]
fn committed_frames_expose_text_payloads_only_through_the_id_lookup_snapshot() {
    let mut tree = ElementTree::new();
    let text = tree.element_create(1, ElementKind::Text);
    tree.set_root(text);
    tree.element_set_text(text, "committed");

    let frame = tree.commit_rendered_frame(0.0);
    let text_run = frame
        .scene()
        .iter()
        .find_map(|(_, node)| match node.kind {
            NodeKind::TextRun { text_run, .. } => Some(text_run),
            _ => None,
        })
        .expect("committed text node");

    assert_eq!(
        frame.resources().text_run(text_run).unwrap().text.as_ref(),
        "committed"
    );
}

#[test]
fn the_typed_sweep_policy_reclaims_after_its_named_threshold() {
    let mut scene = SceneGraph::with_text_resource_policy(
        TextResourcePolicy::new(1).expect("non-zero sweep threshold"),
    );
    let font = RenderFont::new(Blob::from(vec![5, 5, 5, 5]), 0);
    let old = scene.intern_text_run(text_run(font.clone(), "policy"));
    let node = scene.insert(Node {
        kind: NodeKind::TextRun {
            x: 0.0,
            y: 0.0,
            color: [0.0; 4],
            text_run: old,
        },
        children: Vec::new(),
    });

    scene.remove(node);
    scene.maintain_resources();

    assert!(scene.resources().text_run(old).is_err());
    assert_ne!(scene.intern_text_run(text_run(font, "policy")), old);
}

#[test]
fn reinterning_during_one_scene_update_keeps_the_existing_identity() {
    let mut scene = SceneGraph::with_text_resource_policy(
        TextResourcePolicy::new(1).expect("non-zero sweep threshold"),
    );
    let font = RenderFont::new(Blob::from(vec![6, 6, 6, 6]), 0);
    let first = scene.intern_text_run(text_run(font.clone(), "same update"));
    let old_node = scene.insert(Node {
        kind: NodeKind::TextRun {
            x: 0.0,
            y: 0.0,
            color: [0.0; 4],
            text_run: first,
        },
        children: Vec::new(),
    });

    scene.remove(old_node);
    let reinterned = scene.intern_text_run(text_run(font, "same update"));
    scene.insert(Node {
        kind: NodeKind::TextRun {
            x: 0.0,
            y: 0.0,
            color: [0.0; 4],
            text_run: reinterned,
        },
        children: Vec::new(),
    });
    scene.maintain_resources();

    assert_eq!(reinterned, first);
    assert_eq!(
        scene.resources().text_run(first).unwrap().text.as_ref(),
        "same update"
    );
}

#[test]
fn a_concurrent_scene_snapshot_keeps_its_text_resources_alive_until_drop() {
    let mut scene = SceneGraph::new();
    let font = RenderFont::new(Blob::from(vec![4, 3, 2, 1]), 0);
    let text_run = scene.intern_text_run(text_run(font, "snapshot"));
    let node = scene.insert(Node {
        kind: NodeKind::TextRun {
            x: 0.0,
            y: 0.0,
            color: [0.0; 4],
            text_run,
        },
        children: Vec::new(),
    });
    let raster_snapshot = scene.clone();

    scene.remove(node);
    let while_snapshot_alive = scene.sweep_resources();
    assert_eq!(while_snapshot_alive.text_runs, 0);

    let resolved = std::thread::spawn(move || {
        raster_snapshot
            .resources()
            .text_run(text_run)
            .expect("snapshot pins the text run")
            .text
            .to_string()
    })
    .join()
    .expect("raster lookup thread");
    assert_eq!(resolved, "snapshot");

    assert_eq!(scene.sweep_resources().text_runs, 1);
}

#[test]
fn swept_text_run_ids_stay_stale_after_the_slot_is_reused() {
    let mut scene = SceneGraph::new();
    let font = RenderFont::new(Blob::from(vec![9, 8, 7, 6]), 0);
    let old = scene.intern_text_run(text_run(font.clone(), "retired"));
    let old_font = scene.resources().text_run(old).unwrap().font_instance;
    let node = scene.insert(Node {
        kind: NodeKind::TextRun {
            x: 0.0,
            y: 0.0,
            color: [0.0; 4],
            text_run: old,
        },
        children: Vec::new(),
    });

    scene.remove(node);
    let swept = scene.sweep_resources();

    assert_eq!(swept.text_runs, 1);
    assert_eq!(swept.font_instances, 1);
    assert!(matches!(
        scene.resources().text_run(old),
        Err(ResourceLookupError::StaleTextRun(id)) if id == old
    ));
    assert!(matches!(
        scene.resources().font_instance(old_font),
        Err(ResourceLookupError::StaleFontInstance(id)) if id == old_font
    ));
    let replacement = scene.intern_text_run(text_run(font, "retired"));
    assert_ne!(replacement, old);
}

#[test]
fn scene_nodes_hold_only_the_fixed_size_text_run_id() {
    let mut scene = SceneGraph::new();
    let font = RenderFont::new(Blob::from(vec![1, 2, 3, 4]), 0);
    let text_run = scene.intern_text_run(text_run(font, "node payload"));
    let node = scene.insert(Node {
        kind: NodeKind::TextRun {
            x: 10.0,
            y: 20.0,
            color: [0.0, 0.0, 0.0, 1.0],
            text_run,
        },
        children: Vec::new(),
    });

    let NodeKind::TextRun {
        text_run: stored, ..
    } = scene.get(node).expect("text node").kind
    else {
        panic!("expected text run node");
    };
    assert_eq!(stored, text_run);
    assert_eq!(std::mem::size_of::<TextRunId>(), 16);
    assert_eq!(std::mem::size_of::<FontInstanceId>(), 16);
}

#[test]
fn identical_font_instances_and_text_runs_share_stable_ids() {
    let mut scene = SceneGraph::new();
    let font = RenderFont::new(Blob::from(vec![1, 2, 3, 4]), 0);

    let first = scene.intern_text_run(text_run(font.clone(), "same"));
    let second = scene.intern_text_run(text_run(font, "same"));

    assert_eq!(first, second);
    let run = scene
        .resources()
        .text_run(first)
        .expect("interned text run");
    assert_eq!(run.text.as_ref(), "same");
    let font = scene
        .resources()
        .font_instance(run.font_instance)
        .expect("interned font instance");
    assert_eq!(font.font.index, 0);
}
