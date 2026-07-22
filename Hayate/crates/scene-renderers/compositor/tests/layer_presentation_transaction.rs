use std::collections::HashMap;

use hayate_core::element::id::ElementId;
use hayate_core::element::style::{Dimension, StyleProp};
use hayate_core::{Color, CommittedFrame, ElementKind, ElementTree};
use hayate_layer_compositor::{
    scroll_layer_geometry_from_inputs, LayerPresentation, LayerPresentationAdapter,
    LayerPresentationError, LayerPresentationFrame, PlacementPlan, RasterJob, RasterJobKind,
    ScrollLayerGeometry,
};

#[derive(Default)]
struct RecordingAdapter {
    rasterized: Vec<RasterJobKind>,
    repaints: Vec<(RasterJobKind, bool)>,
    placed: Vec<RasterJobKind>,
    composites: usize,
    discarded: Vec<ElementId>,
    fail_composite: bool,
}

impl LayerPresentationAdapter for RecordingAdapter {
    type Error = &'static str;

    fn rasterize(&mut self, job: &RasterJob) -> Result<u64, Self::Error> {
        self.rasterized.push(job.kind);
        self.repaints.push((job.kind, job.repaint));
        Ok(64)
    }

    fn composite(&mut self, plan: &PlacementPlan) -> Result<(), Self::Error> {
        self.placed
            .extend(plan.planes.iter().map(|plane| plane.kind));
        self.composites += 1;
        self.fail_composite
            .then_some(())
            .map_or(Ok(()), |_| Err("composite failed"))
    }

    fn discard(&mut self, layers: &[ElementId]) {
        self.discarded.extend_from_slice(layers);
    }
}

fn px(value: f32) -> Dimension {
    Dimension::px(value)
}

fn root_tree() -> ElementTree {
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(100.0, 100.0);
    tree
}

fn frame<'a>(
    committed: &'a CommittedFrame,
    scroll: &'a HashMap<ElementId, ScrollLayerGeometry>,
) -> LayerPresentationFrame<'a> {
    LayerPresentationFrame {
        snapshot: committed.snapshot(),
        topology: committed.layer_topology(),
        scroll_geometry: scroll,
    }
}

#[test]
fn failed_composite_keeps_revision_and_cache_ledger_uncommitted() {
    let mut tree = root_tree();
    let committed = tree.commit_rendered_frame(0.0);
    let scroll = HashMap::new();
    let mut presentation = LayerPresentation::new();
    let mut adapter = RecordingAdapter {
        fail_composite: true,
        ..Default::default()
    };

    let error = presentation
        .present(frame(&committed, &scroll), &mut adapter)
        .expect_err("failed composite must abort the presentation transaction");

    assert!(matches!(error, LayerPresentationError::Adapter(_)));
    assert_eq!(presentation.cached_bytes(), 0);
    assert_eq!(presentation.applied_revision(), None);
    assert!(!presentation.pending_dirty().is_empty());
    assert_eq!(adapter.rasterized, vec![RasterJobKind::Content]);
}

#[test]
fn clean_frame_reuses_retained_placements_without_a_scene_walk() {
    let mut tree = root_tree();
    let first = tree.commit_rendered_frame(0.0);
    let scroll = HashMap::new();
    let mut presentation = LayerPresentation::new();
    let mut adapter = RecordingAdapter::default();
    presentation
        .present(frame(&first, &scroll), &mut adapter)
        .unwrap();
    assert_eq!(presentation.placement_rebuilds(), 1);
    assert!(first.layer_topology().placement_nodes_visited() > 0);

    let clean = tree.commit_rendered_frame(16.0);
    assert_eq!(
        clean.layer_topology().placement_nodes_visited(),
        0,
        "Core must retain canonical placements without walking the snapshot on a clean frame"
    );
    presentation
        .present(frame(&clean, &scroll), &mut adapter)
        .unwrap();

    assert_eq!(presentation.placement_rebuilds(), 1);
    assert_eq!(
        presentation.applied_revision(),
        Some(clean.layer_topology().revision())
    );
}

#[test]
fn content_only_change_reuses_core_placements_without_a_scene_walk() {
    let mut tree = root_tree();
    let root = tree.root().expect("root");
    let _ = tree.commit_rendered_frame(0.0);

    tree.element_set_style(
        root,
        &[StyleProp::BackgroundColor(Color::new(0.2, 0.4, 0.6, 1.0))],
    );
    let changed = tree.commit_rendered_frame(16.0);

    assert!(changed.layer_topology().content_changed().contains(&root));
    assert!(changed.layer_topology().geometry_changed().is_empty());
    assert_eq!(changed.layer_topology().placement_nodes_visited(), 0);
}

#[test]
fn failed_topology_change_is_rebuilt_on_a_clean_retry() {
    let mut tree = root_tree();
    let first = tree.commit_rendered_frame(0.0);
    let mut presentation = LayerPresentation::new();
    let mut adapter = RecordingAdapter::default();
    presentation
        .present(frame(&first, &HashMap::new()), &mut adapter)
        .unwrap();

    let root = tree.root().expect("root");
    let scroll = tree.element_create(1, ElementKind::ScrollView);
    let content = tree.element_create(2, ElementKind::View);
    tree.element_append_child(root, scroll);
    tree.element_append_child(scroll, content);
    tree.element_set_style(
        scroll,
        &[StyleProp::Width(px(100.0)), StyleProp::Height(px(100.0))],
    );
    tree.element_set_style(
        content,
        &[StyleProp::Width(px(100.0)), StyleProp::Height(px(300.0))],
    );
    let changed = tree.commit_rendered_frame(16.0);
    let changed_scroll = scroll_layer_geometry_from_inputs(changed.scroll_inputs());
    adapter.fail_composite = true;
    presentation
        .present(frame(&changed, &changed_scroll), &mut adapter)
        .expect_err("changed frame must fail");
    assert_eq!(presentation.placement_rebuilds(), 1);

    adapter.fail_composite = false;
    let clean = tree.commit_rendered_frame(32.0);
    let clean_scroll = scroll_layer_geometry_from_inputs(clean.scroll_inputs());
    presentation
        .present(frame(&clean, &clean_scroll), &mut adapter)
        .unwrap();

    assert_eq!(
        presentation.placement_rebuilds(),
        2,
        "a failed structural/geometry change must survive into the clean retry"
    );
}

#[test]
fn failed_scroll_chrome_repaint_is_retried_on_a_clean_frame() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let scroll = tree.element_create(1, ElementKind::ScrollView);
    let content = tree.element_create(2, ElementKind::View);
    tree.element_append_child(root, scroll);
    tree.element_append_child(scroll, content);
    tree.set_root(root);
    tree.set_viewport(100.0, 100.0);
    tree.element_set_style(
        scroll,
        &[StyleProp::Width(px(100.0)), StyleProp::Height(px(100.0))],
    );
    tree.element_set_style(
        content,
        &[StyleProp::Width(px(100.0)), StyleProp::Height(px(300.0))],
    );

    let mut presentation = LayerPresentation::new();
    let mut adapter = RecordingAdapter::default();
    let first = tree.commit_rendered_frame(0.0);
    let first_scroll = scroll_layer_geometry_from_inputs(first.scroll_inputs());
    presentation
        .present(frame(&first, &first_scroll), &mut adapter)
        .unwrap();

    tree.element_set_scroll_offset(scroll, 0.0, 40.0);
    let changed = tree.commit_rendered_frame(16.0);
    assert!(changed.layer_topology().chrome_changed().contains(&scroll));
    let changed_scroll = scroll_layer_geometry_from_inputs(changed.scroll_inputs());
    adapter.fail_composite = true;
    presentation
        .present(frame(&changed, &changed_scroll), &mut adapter)
        .expect_err("changed frame must fail");

    adapter.fail_composite = false;
    adapter.repaints.clear();
    let clean = tree.commit_rendered_frame(32.0);
    assert!(clean.layer_topology().chrome_changed().is_empty());
    let clean_scroll = scroll_layer_geometry_from_inputs(clean.scroll_inputs());
    presentation
        .present(frame(&clean, &clean_scroll), &mut adapter)
        .unwrap();

    assert!(
        adapter
            .repaints
            .contains(&(RasterJobKind::ScrollChrome, true)),
        "a failed chrome repaint must remain dirty on the clean retry"
    );
}

#[test]
fn scroll_chrome_is_placed_by_the_shared_transaction_plan() {
    let mut tree = ElementTree::new();
    let root = tree.element_create(0, ElementKind::View);
    let scroll = tree.element_create(1, ElementKind::ScrollView);
    let content = tree.element_create(2, ElementKind::View);
    tree.element_append_child(root, scroll);
    tree.element_append_child(scroll, content);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        scroll,
        &[StyleProp::Width(px(200.0)), StyleProp::Height(px(200.0))],
    );
    tree.element_set_style(
        content,
        &[StyleProp::Width(px(200.0)), StyleProp::Height(px(600.0))],
    );
    let committed = tree.commit_rendered_frame(0.0);
    let scroll_geometry = scroll_layer_geometry_from_inputs(committed.scroll_inputs());
    let mut presentation = LayerPresentation::new();
    let mut adapter = RecordingAdapter::default();

    presentation
        .present(frame(&committed, &scroll_geometry), &mut adapter)
        .unwrap();

    assert_eq!(
        adapter.placed,
        vec![
            RasterJobKind::Content,
            RasterJobKind::Content,
            RasterJobKind::ScrollChrome
        ]
    );
}
