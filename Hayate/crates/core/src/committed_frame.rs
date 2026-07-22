use crate::{ElementId, LayerTopology, SceneResources, SceneSnapshot};

/// Platform-free, renderer-ready owned value produced by one frame commit.
#[derive(Debug, Clone)]
pub struct CommittedFrame {
    snapshot: SceneSnapshot,
    topology: LayerTopology,
    scroll_inputs: Vec<ScrollCompositorInput>,
    pending_visual_work: bool,
}

/// Core-owned scroll facts from which a projection derives backend geometry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollCompositorInput {
    pub layer: ElementId,
    pub absolute_top: f32,
    pub viewport_height: f32,
    pub scroll_offset: f32,
    pub max_scroll_offset: f32,
    pub scroll_affine: [f64; 6],
    pub content_dirty: bool,
}

impl CommittedFrame {
    pub(crate) fn new(
        snapshot: SceneSnapshot,
        topology: LayerTopology,
        scroll_inputs: Vec<ScrollCompositorInput>,
        pending_visual_work: bool,
    ) -> Self {
        Self {
            snapshot,
            topology,
            scroll_inputs,
            pending_visual_work,
        }
    }

    pub fn snapshot(&self) -> &SceneSnapshot {
        &self.snapshot
    }

    pub fn layer_topology(&self) -> &LayerTopology {
        &self.topology
    }

    pub fn resources(&self) -> &SceneResources {
        self.snapshot.resources()
    }

    pub fn scroll_inputs(&self) -> &[ScrollCompositorInput] {
        &self.scroll_inputs
    }

    pub fn has_pending_visual_work(&self) -> bool {
        self.pending_visual_work
    }
}
