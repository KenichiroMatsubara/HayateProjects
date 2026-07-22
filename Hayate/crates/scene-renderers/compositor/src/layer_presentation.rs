//! Stateful, backend-independent retained layer-presentation transaction.

use std::collections::{HashMap, HashSet};
use std::fmt::Display;

use hayate_core::element::id::ElementId;
use hayate_core::{
    compose, LayerPlacement, LayerRasterBounds, LayerScene, LayerSceneKind, LayerTopology,
    SceneSnapshot,
};

use crate::{GpuBudget, PresentPlanner, RasterBand, ScrollLayerExtent, ScrollLayerGeometry};

/// One committed Core frame projected into the facts presentation consumes.
pub struct LayerPresentationFrame<'a> {
    pub snapshot: &'a SceneSnapshot,
    pub topology: &'a LayerTopology,
    pub scroll_geometry: &'a HashMap<ElementId, ScrollLayerGeometry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RasterJobKind {
    Content,
    ScrollChrome,
}

#[derive(Debug)]
pub struct RasterJob<'a> {
    pub layer: ElementId,
    pub kind: RasterJobKind,
    pub scene: &'a LayerScene,
    pub bounds: Option<LayerRasterBounds>,
    pub band: Option<RasterBand>,
    pub scroll_band: Option<ScrollLayerExtent>,
    pub repaint: bool,
}

struct PreparedJob {
    layer: ElementId,
    kind: RasterJobKind,
    scene: LayerScene,
    bounds: Option<LayerRasterBounds>,
    band: Option<RasterBand>,
    scroll_band: Option<ScrollLayerExtent>,
    repaint: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub layer: ElementId,
    pub kind: RasterJobKind,
    pub transform: [f64; 6],
    pub clip: Option<[f32; 4]>,
}

#[derive(Debug, Default, Clone)]
pub struct PlacementPlan {
    pub planes: Vec<Placement>,
}

pub trait LayerPresentationAdapter {
    type Error: Display;

    fn rasterize(&mut self, job: &RasterJob<'_>) -> Result<u64, Self::Error>;
    fn composite(&mut self, plan: &PlacementPlan) -> Result<(), Self::Error>;
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

/// Owns retained placements, the logical cache ledger, applied revision and pending dirty facts.
/// Retry/fallback policy intentionally remains with the Render Host.
#[derive(Debug, Default)]
pub struct LayerPresentation {
    planner: PresentPlanner,
    previous_layers: HashSet<ElementId>,
    placements: Vec<LayerPlacement>,
    applied_revision: Option<u64>,
    pending_dirty: HashSet<ElementId>,
    pending_chrome_dirty: HashSet<ElementId>,
    pending_placement_rebuild: bool,
    placement_rebuilds: u64,
}

impl LayerPresentation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cached_bytes(&self) -> u64 {
        self.planner.cached_bytes()
    }

    pub fn applied_revision(&self) -> Option<u64> {
        self.applied_revision
    }

    pub fn pending_dirty(&self) -> &HashSet<ElementId> {
        &self.pending_dirty
    }

    pub fn placement_rebuilds(&self) -> u64 {
        self.placement_rebuilds
    }

    pub fn invalidate(&mut self) {
        self.planner.invalidate();
        self.previous_layers.clear();
        self.placements.clear();
        self.applied_revision = None;
        self.pending_dirty.clear();
        self.pending_chrome_dirty.clear();
        self.pending_placement_rebuild = false;
    }

    /// Prepare, execute and commit one frame. Shared committed state changes only after every
    /// backend operation succeeds; failures retain dirty facts for the host's next decision.
    pub fn present<A: LayerPresentationAdapter>(
        &mut self,
        frame: LayerPresentationFrame<'_>,
        adapter: &mut A,
    ) -> Result<(), LayerPresentationError> {
        let layers = frame.topology.paint_order();
        let Some(&root) = layers.first() else {
            return Ok(());
        };
        let current_layers: HashSet<ElementId> = layers.iter().copied().collect();
        let bounds: HashMap<ElementId, LayerRasterBounds> = frame
            .topology
            .raster_bounds()
            .iter()
            .map(|bounds| (bounds.layer, *bounds))
            .collect();
        self.pending_dirty
            .extend(frame.topology.content_changed().iter().copied());
        self.pending_chrome_dirty
            .extend(frame.topology.chrome_changed().iter().copied());
        self.pending_placement_rebuild |= self.applied_revision.is_none()
            || !frame.topology.structural_changed().is_empty()
            || !frame.topology.geometry_changed().is_empty()
            || !frame.topology.transform_changed().is_empty();

        let stale: Vec<ElementId> = self
            .previous_layers
            .difference(&current_layers)
            .copied()
            .collect();
        let non_scroll: Vec<ElementId> = layers
            .iter()
            .copied()
            .filter(|layer| *layer == root || !frame.scroll_geometry.contains_key(layer))
            .collect();
        let content_plan = self.planner.plan_layers(&non_scroll, &self.pending_dirty);
        let mut jobs = Vec::new();
        for layer in content_plan.raster {
            jobs.push(PreparedJob {
                layer,
                kind: RasterJobKind::Content,
                scene: make_scene(
                    frame.snapshot,
                    frame.topology,
                    layer,
                    LayerSceneKind::Content,
                )?,
                bounds: bounds.get(&layer).copied(),
                band: None,
                scroll_band: None,
                repaint: true,
            });
        }
        for (&layer, geometry) in frame.scroll_geometry {
            let needs_content_raster = geometry.content_dirty
                || self.pending_dirty.contains(&layer)
                || self.planner.scroll_layer_needs_raster(
                    layer,
                    geometry.visible_top,
                    geometry.viewport_height,
                );
            if needs_content_raster {
                jobs.push(PreparedJob {
                    layer,
                    kind: RasterJobKind::Content,
                    scene: make_scene(
                        frame.snapshot,
                        frame.topology,
                        layer,
                        LayerSceneKind::ScrollContent {
                            scroll_affine: geometry.scroll_affine,
                        },
                    )?,
                    bounds: bounds.get(&layer).copied(),
                    band: Some(geometry.raster_band()),
                    scroll_band: Some(geometry.band),
                    repaint: true,
                });
            }
            if layer != root {
                let mut chrome_bounds =
                    bounds
                        .get(&layer)
                        .copied()
                        .ok_or(LayerPresentationError::InvalidFrame {
                            layer,
                            reason: "missing Core raster bounds",
                        })?;
                chrome_bounds.origin_y = geometry.absolute_top;
                chrome_bounds.height = geometry.viewport_height;
                jobs.push(PreparedJob {
                    layer,
                    kind: RasterJobKind::ScrollChrome,
                    scene: make_scene(
                        frame.snapshot,
                        frame.topology,
                        layer,
                        LayerSceneKind::ScrollChrome,
                    )?,
                    bounds: Some(chrome_bounds),
                    band: None,
                    scroll_band: None,
                    repaint: self.pending_chrome_dirty.contains(&layer),
                });
            }
        }

        let prepared_placements = self
            .pending_placement_rebuild
            .then(|| frame.topology.placements().to_vec());
        let base_placements = prepared_placements.as_deref().unwrap_or(&self.placements);
        let placement_plan = placement_plan(base_placements, frame.scroll_geometry);

        let mut staged = Vec::with_capacity(jobs.len());
        for prepared in &jobs {
            let job = RasterJob {
                layer: prepared.layer,
                kind: prepared.kind,
                scene: &prepared.scene,
                bounds: prepared.bounds,
                band: prepared.band,
                scroll_band: prepared.scroll_band,
                repaint: prepared.repaint,
            };
            let bytes = adapter
                .rasterize(&job)
                .map_err(|error| LayerPresentationError::Adapter(error.to_string()))?;
            staged.push((job.layer, job.kind, job.scroll_band, bytes));
        }
        adapter
            .composite(&placement_plan)
            .map_err(|error| LayerPresentationError::Adapter(error.to_string()))?;

        let mut committed = HashMap::new();
        for (layer, kind, band, bytes) in staged {
            let entry = committed.entry(layer).or_insert((None, bytes));
            if kind == RasterJobKind::Content {
                entry.0 = band;
            }
            entry.1 = bytes;
        }
        for (layer, (new_scroll_band, bytes)) in committed {
            match new_scroll_band {
                Some(band) => self.planner.note_scroll_rasterized(layer, band, bytes),
                None if self.planner.cached_scroll_band(layer).is_some() => {
                    self.planner.update_cached_bytes(layer, bytes)
                }
                None => self.planner.note_layer_rasterized(layer, bytes),
            }
        }
        for plane in &placement_plan.planes {
            self.planner.note_composited(plane.layer);
        }
        for layer in &stale {
            self.planner.evict(*layer);
        }
        adapter.discard(&stale);
        self.previous_layers = current_layers;
        if let Some(placements) = prepared_placements {
            self.placements = placements;
            self.placement_rebuilds += 1;
        }
        self.applied_revision = Some(frame.topology.revision());
        self.pending_dirty.clear();
        self.pending_chrome_dirty.clear();
        self.pending_placement_rebuild = false;
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

fn make_scene(
    snapshot: &SceneSnapshot,
    topology: &LayerTopology,
    layer: ElementId,
    kind: LayerSceneKind,
) -> Result<LayerScene, LayerPresentationError> {
    LayerScene::new(snapshot.clone(), topology.clone(), layer, kind).ok_or(
        LayerPresentationError::InvalidFrame {
            layer,
            reason: "missing layer scene",
        },
    )
}

fn placement_plan(
    placements: &[LayerPlacement],
    scroll_geometry: &HashMap<ElementId, ScrollLayerGeometry>,
) -> PlacementPlan {
    let mut plan = PlacementPlan::default();
    for placement in placements {
        let transform = scroll_geometry
            .get(&placement.layer)
            .map_or(placement.transform, |geometry| {
                compose(placement.transform, geometry.scroll_affine)
            });
        plan.planes.push(Placement {
            layer: placement.layer,
            kind: RasterJobKind::Content,
            transform,
            clip: placement.clip,
        });
        if scroll_geometry.contains_key(&placement.layer) {
            plan.planes.push(Placement {
                layer: placement.layer,
                kind: RasterJobKind::ScrollChrome,
                transform: placement.transform,
                clip: placement.clip,
            });
        }
    }
    plan
}
