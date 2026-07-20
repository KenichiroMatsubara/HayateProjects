//! Stateful, backend-independent layer-presentation transaction.
//!
//! The transaction owns planning, committed cache state, validation and the ordering of
//! prepare → execute → commit. Backends only raster [`RasterJob`]s and composite the resulting
//! [`PlacementPlan`]. This keeps Pixmaps/textures and final surface blits on the adapter side.

use std::collections::{HashMap, HashSet};
use std::fmt::Display;

use hayate_core::element::id::ElementId;
use hayate_core::{LayerRasterBounds, SceneGraph};

use crate::layer_scene::{
    collect_layer_placements, compose, extract_layer_scene, extract_root_scene,
    extract_scroll_chrome_scene, extract_scroll_layer_scene,
};
use crate::{GpuBudget, PresentPlanner, RasterBand, ScrollLayerExtent, ScrollLayerGeometry};

/// A committed frame projected into the information layer presentation owns.
pub struct LayerPresentationFrame<'a> {
    pub scene: &'a SceneGraph,
    pub layers: &'a [ElementId],
    pub layer_raster_bounds: &'a [LayerRasterBounds],
    pub layer_dirty: &'a HashSet<ElementId>,
    pub chrome_dirty: &'a HashSet<ElementId>,
    pub scroll_geometry: &'a HashMap<ElementId, ScrollLayerGeometry>,
}

/// Which independently cached plane a job updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RasterJobKind {
    Content,
    ScrollChrome,
}

/// One validated raster operation. `scene` has already been extracted by the shared layer
/// presentation module; backend adapters must not repeat scene traversal or validation.
#[derive(Debug)]
pub struct RasterJob<'a> {
    pub layer: ElementId,
    pub kind: RasterJobKind,
    pub scene: &'a SceneGraph,
    pub bounds: Option<LayerRasterBounds>,
    pub band: Option<RasterBand>,
    pub scroll_band: Option<ScrollLayerExtent>,
}

struct PreparedJob {
    layer: ElementId,
    kind: RasterJobKind,
    scene: SceneGraph,
    bounds: Option<LayerRasterBounds>,
    band: Option<RasterBand>,
    scroll_band: Option<ScrollLayerExtent>,
}

/// A single backend-agnostic composite plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub layer: ElementId,
    pub kind: RasterJobKind,
    pub transform: [f64; 6],
    pub clip: Option<[f32; 4]>,
}

/// The full placement plan for the final composite and blit.
#[derive(Debug, Default)]
pub struct PlacementPlan {
    pub planes: Vec<Placement>,
}

/// The backend-local work left after shared prepare has completed.
pub trait LayerPresentationAdapter {
    type Error: Display;

    /// Raster a prepared job and return the cache plane's byte size.
    fn rasterize(&mut self, job: &RasterJob<'_>) -> Result<u64, Self::Error>;
    /// Composite and blit every plane. A failure leaves the shared ledger uncommitted.
    fn composite(&mut self, plan: &PlacementPlan) -> Result<(), Self::Error>;
    /// Discard exactly the layers which shared committed state evicted or found stale.
    fn discard(&mut self, layers: &[ElementId]);
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayerPresentationError {
    InvalidFrame {
        layer: ElementId,
        reason: &'static str,
    },
    Adapter(String),
}

/// Stateful owner of the layer-presentation transaction and cache ledger.
#[derive(Debug, Default)]
pub struct LayerPresentation {
    planner: PresentPlanner,
    previous_layers: HashSet<ElementId>,
}

impl LayerPresentation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cached_bytes(&self) -> u64 {
        self.planner.cached_bytes()
    }

    pub fn invalidate(&mut self) {
        self.planner.invalidate();
        self.previous_layers.clear();
    }

    /// Presents one committed frame. No planner or LRU mutation is performed until all raster,
    /// composite and adapter-owned blit work has succeeded.
    pub fn present<A: LayerPresentationAdapter>(
        &mut self,
        frame: LayerPresentationFrame<'_>,
        adapter: &mut A,
    ) -> Result<(), LayerPresentationError> {
        let Some(&root) = frame.layers.first() else {
            return Ok(());
        };
        let boundaries: HashSet<ElementId> = frame.layers.iter().copied().collect();
        let bounds: HashMap<ElementId, LayerRasterBounds> = frame
            .layer_raster_bounds
            .iter()
            .map(|bound| (bound.layer, *bound))
            .collect();

        // Prepare validates every non-root layer before any backend work starts.
        for &layer in frame.layers.iter().skip(1) {
            if !bounds.contains_key(&layer) {
                return Err(LayerPresentationError::InvalidFrame {
                    layer,
                    reason: "missing Core raster bounds",
                });
            }
            let exists = match frame.scroll_geometry.get(&layer) {
                Some(geometry) => extract_scroll_layer_scene(
                    frame.scene,
                    layer,
                    &boundaries,
                    geometry.scroll_affine,
                )
                .is_some(),
                None => extract_layer_scene(frame.scene, layer, &boundaries).is_some(),
            };
            if !exists {
                return Err(LayerPresentationError::InvalidFrame {
                    layer,
                    reason: "missing layer sub-scene",
                });
            }
        }

        let stale: Vec<ElementId> = self
            .previous_layers
            .difference(&boundaries)
            .copied()
            .collect();
        let non_scroll: Vec<ElementId> = frame
            .layers
            .iter()
            .copied()
            .filter(|layer| *layer == root || !frame.scroll_geometry.contains_key(layer))
            .collect();
        let content_plan = self.planner.plan_layers(&non_scroll, frame.layer_dirty);
        let mut jobs = Vec::new();
        for layer in content_plan.raster {
            let scene = if layer == root {
                extract_root_scene(frame.scene, root, &boundaries)
            } else {
                // Validated above.
                extract_layer_scene(frame.scene, layer, &boundaries).expect("validated layer scene")
            };
            jobs.push(PreparedJob {
                layer,
                kind: RasterJobKind::Content,
                scene,
                bounds: bounds.get(&layer).copied(),
                band: None,
                scroll_band: None,
            });
        }
        for (&layer, geometry) in frame.scroll_geometry {
            let needs_raster = geometry.content_dirty
                || self.planner.scroll_layer_needs_raster(
                    layer,
                    geometry.visible_top,
                    geometry.viewport_height,
                );
            if needs_raster {
                let scene = extract_scroll_layer_scene(
                    frame.scene,
                    layer,
                    &boundaries,
                    geometry.scroll_affine,
                )
                .expect("validated scroll layer scene");
                jobs.push(PreparedJob {
                    layer,
                    kind: RasterJobKind::Content,
                    scene,
                    bounds: bounds.get(&layer).copied(),
                    band: Some(geometry.raster_band()),
                    scroll_band: Some(geometry.band),
                });
            }
            if frame.chrome_dirty.contains(&layer) {
                if let Some(scene) = extract_scroll_chrome_scene(frame.scene, layer, &boundaries) {
                    let mut chrome_bounds = *bounds.get(&layer).expect("validated bounds");
                    chrome_bounds.origin_y = geometry.absolute_top;
                    chrome_bounds.height = geometry.viewport_height;
                    jobs.push(PreparedJob {
                        layer,
                        kind: RasterJobKind::ScrollChrome,
                        scene,
                        bounds: Some(chrome_bounds),
                        band: None,
                        scroll_band: None,
                    });
                }
            }
        }

        let mut staged = Vec::with_capacity(jobs.len());
        for job in &jobs {
            let job = RasterJob {
                layer: job.layer,
                kind: job.kind,
                scene: &job.scene,
                bounds: job.bounds,
                band: job.band,
                scroll_band: job.scroll_band,
            };
            let bytes = adapter
                .rasterize(&job)
                .map_err(|error| LayerPresentationError::Adapter(error.to_string()))?;
            staged.push((job.layer, job.kind, job.scroll_band, bytes));
        }

        let mut placement_plan = PlacementPlan::default();
        for placement in collect_layer_placements(frame.scene, root, &boundaries) {
            let transform = frame
                .scroll_geometry
                .get(&placement.layer)
                .map_or(placement.transform, |geometry| {
                    compose(placement.transform, geometry.scroll_affine)
                });
            placement_plan.planes.push(Placement {
                layer: placement.layer,
                kind: RasterJobKind::Content,
                transform,
                clip: placement.clip,
            });
        }
        adapter
            .composite(&placement_plan)
            .map_err(|error| LayerPresentationError::Adapter(error.to_string()))?;

        // Commit only after the adapter has successfully rastered, composited and blitted.
        for (layer, kind, band, bytes) in staged {
            match (kind, band) {
                (RasterJobKind::Content, Some(band)) => {
                    self.planner.note_scroll_rasterized(layer, band, bytes)
                }
                (RasterJobKind::Content, None) => self.planner.note_layer_rasterized(layer, bytes),
                (RasterJobKind::ScrollChrome, _) => {}
            }
        }
        for plane in &placement_plan.planes {
            self.planner.note_composited(plane.layer);
        }
        for stale_layer in &stale {
            self.planner.evict(*stale_layer);
        }
        adapter.discard(&stale);
        self.previous_layers = boundaries;
        Ok(())
    }

    pub fn enforce_budget<A: LayerPresentationAdapter>(
        &mut self,
        budget: GpuBudget,
        adapter: &mut A,
    ) {
        let evicted = self.planner.enforce_budget(budget);
        adapter.discard(&evicted);
    }
}
