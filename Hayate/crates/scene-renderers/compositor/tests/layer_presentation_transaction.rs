use std::collections::{HashMap, HashSet};

use hayate_core::element::id::ElementId;
use hayate_core::{LayerRasterBounds, SceneGraph};
use hayate_layer_compositor::{
    LayerPresentation, LayerPresentationAdapter, LayerPresentationError, LayerPresentationFrame,
    RasterJob, RasterJobKind, ScrollLayerGeometry,
};

#[derive(Default)]
struct RecordingAdapter {
    rasterized: Vec<RasterJobKind>,
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
        _plan: &hayate_layer_compositor::PlacementPlan,
    ) -> Result<(), Self::Error> {
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
