use std::collections::{HashMap, HashSet};

use hayate_core::element::id::ElementId;
use hayate_core::element::style::{Dimension, StyleProp};
use hayate_core::{ElementKind, ElementTree, LayerRasterBounds, SceneGraph};
use hayate_layer_compositor::{
    scroll_layer_geometry_from_inputs, LayerPresentation, LayerPresentationAdapter,
    LayerPresentationError, LayerPresentationFrame, RasterJob, RasterJobKind, ScrollLayerGeometry,
};

#[derive(Default)]
struct RecordingAdapter {
    rasterized: Vec<RasterJobKind>,
    placed: Vec<RasterJobKind>,
    composites: usize,
    discarded: Vec<ElementId>,
    fail_composite: bool,
}

impl LayerPresentationAdapter for RecordingAdapter {
    type Error = &'static str;

    fn rasterize(&mut self, job: &RasterJob) -> Result<u64, Self::Error> {
        self.rasterized.push(job.kind);
        Ok(64)
    }

    fn composite(
        &mut self,
        plan: &hayate_layer_compositor::PlacementPlan,
    ) -> Result<(), Self::Error> {
        self.placed
            .extend(plan.planes.iter().map(|plane| plane.kind));
        self.composites += 1;
        if self.fail_composite {
            Err("composite failed")
        } else {
            Ok(())
        }
    }

    fn discard(&mut self, layers: &[ElementId]) {
        self.discarded.extend_from_slice(layers);
    }
}

fn px(value: f32) -> Dimension {
    Dimension::px(value)
}

fn id(value: u64) -> ElementId {
    ElementId::from_u64(value)
}

fn frame<'a>(
    scene: &'a SceneGraph,
    layers: &'a [ElementId],
    bounds: &'a [LayerRasterBounds],
    dirty: &'a HashSet<ElementId>,
    chrome_dirty: &'a HashSet<ElementId>,
    scroll: &'a HashMap<ElementId, ScrollLayerGeometry>,
) -> LayerPresentationFrame<'a> {
    LayerPresentationFrame {
        scene,
        layers,
        layer_raster_bounds: bounds,
        layer_dirty: dirty,
        chrome_dirty,
        scroll_geometry: scroll,
    }
}

#[test]
fn missing_non_root_bounds_fails_before_backend_work() {
    let scene = SceneGraph::new();
    let layers = [id(1), id(2)];
    let empty = HashSet::new();
    let scroll = HashMap::new();
    let mut presentation = LayerPresentation::new();
    let mut adapter = RecordingAdapter::default();

    let error = presentation
        .present(
            frame(&scene, &layers, &[], &empty, &empty, &scroll),
            &mut adapter,
        )
        .expect_err("a non-root layer without Core bounds is an invalid committed frame");

    assert!(matches!(
        error,
        LayerPresentationError::InvalidFrame { layer, .. } if layer == id(2)
    ));
    assert!(adapter.rasterized.is_empty());
    assert_eq!(adapter.composites, 0);
}

#[test]
fn failed_composite_does_not_commit_the_cache_ledger() {
    let scene = SceneGraph::new();
    let layers = [id(1)];
    let empty = HashSet::new();
    let scroll = HashMap::new();
    let mut presentation = LayerPresentation::new();
    let mut adapter = RecordingAdapter {
        fail_composite: true,
        ..Default::default()
    };

    let error = presentation
        .present(
            frame(&scene, &layers, &[], &empty, &empty, &scroll),
            &mut adapter,
        )
        .expect_err("failed composite must abort the presentation transaction");

    assert!(matches!(error, LayerPresentationError::Adapter(_)));
    assert_eq!(
        presentation.cached_bytes(),
        0,
        "no staged cache entry is committed"
    );
    assert_eq!(adapter.rasterized, vec![RasterJobKind::Content]);
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
    let frame = tree.commit_rendered_frame(0.0);
    let empty = HashSet::new();
    let chrome_dirty = HashSet::from([scroll]);
    let scroll_geometry = scroll_layer_geometry_from_inputs(frame.scroll_inputs());
    let mut presentation = LayerPresentation::new();
    let mut adapter = RecordingAdapter::default();

    presentation
        .present(
            LayerPresentationFrame {
                scene: frame.scene(),
                layers: frame.layers(),
                layer_raster_bounds: frame.layer_raster_bounds(),
                layer_dirty: &empty,
                chrome_dirty: &chrome_dirty,
                scroll_geometry: &scroll_geometry,
            },
            &mut adapter,
        )
        .expect("the committed scroll frame should be presentable");

    assert_eq!(
        adapter.placed,
        vec![
            RasterJobKind::Content,
            RasterJobKind::Content,
            RasterJobKind::ScrollChrome,
        ],
        "the adapter must receive content and fixed chrome in one shared placement plan",
    );
}
