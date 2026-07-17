use std::collections::HashSet;

use crate::{ElementId, SceneGraph};

/// Platform-free, renderer-ready view produced by one frame commit.
pub struct CommittedFrame<'a> {
    scene: &'a SceneGraph,
    layers: &'a [ElementId],
    content_dirty_layers: &'a HashSet<ElementId>,
    chrome_dirty_layers: &'a HashSet<ElementId>,
    transform_dirty_layers: &'a HashSet<ElementId>,
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
    /// Profile-resolved scroll Group affine for this committed frame. The compositor applies
    /// this to a scroll texture whose pixels are cached without the Group baked in, preserving
    /// both iOS rubber-band translation and Android edge stretch on composite-only frames.
    pub scroll_affine: [f64; 6],
    pub content_dirty: bool,
}

impl<'a> CommittedFrame<'a> {
    pub(crate) fn new(
        scene: &'a SceneGraph,
        layers: &'a [ElementId],
        content_dirty_layers: &'a HashSet<ElementId>,
        chrome_dirty_layers: &'a HashSet<ElementId>,
        transform_dirty_layers: &'a HashSet<ElementId>,
        scroll_inputs: Vec<ScrollCompositorInput>,
        pending_visual_work: bool,
    ) -> Self {
        Self {
            scene,
            layers,
            content_dirty_layers,
            chrome_dirty_layers,
            transform_dirty_layers,
            scroll_inputs,
            pending_visual_work,
        }
    }

    pub fn scene(&self) -> &SceneGraph {
        self.scene
    }
    pub fn layers(&self) -> &[ElementId] {
        self.layers
    }
    pub fn content_dirty_layers(&self) -> &HashSet<ElementId> {
        self.content_dirty_layers
    }
    pub fn chrome_dirty_layers(&self) -> &HashSet<ElementId> {
        self.chrome_dirty_layers
    }
    pub fn transform_dirty_layers(&self) -> &HashSet<ElementId> {
        self.transform_dirty_layers
    }
    pub fn scroll_inputs(&self) -> &[ScrollCompositorInput] {
        &self.scroll_inputs
    }
    pub fn has_pending_visual_work(&self) -> bool {
        self.pending_visual_work
    }
}
