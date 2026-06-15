use std::collections::{HashMap, HashSet};

use crate::element::id::ElementId;
use crate::element::visual_invalidation::{
    self, VisualInvalidationReach,
};
use crate::element::layout_pass::LayoutPass;
use crate::element::tree::{apply_visual, Element, Event};

/// Owns the dirty-tracking sets (`structure_dirty` / `shape_dirty` / `fonts_dirty`)
/// that drive `ElementTree::commit_frame()` (ADR-0075).
///
/// Dirty-marking *policy* (which mutations mark what dirty) stays in
/// `tree.rs`'s `element_set_*` methods; `ElementEngine` only owns the dirty
/// sets and resolves them.
pub(crate) struct ElementEngine {
    pub(crate) structure_dirty: HashSet<ElementId>,
    /// IFC roots needing Parley re-compose before layout (ADR-0063).
    pub(crate) shape_dirty: HashSet<ElementId>,
    /// Scene re-lowering reach for `shape_dirty` seeds (issue #185).
    pub(crate) shape_lowering_reach: HashMap<ElementId, VisualInvalidationReach>,
    /// Elements whose viewport-conditioned own-style changed on resize (ADR-0081).
    pub(crate) viewport_dirty: HashSet<ElementId>,
    /// Scene-only visual changes (issue #182). Drained after each `render()`.
    pub(crate) visual_dirty: HashMap<ElementId, VisualInvalidationReach>,
    /// Elements whose absolute box geometry `(x, y, w, h)` changed (or appeared)
    /// in the latest `resolve()` layout pass. Bridges the layout→lowering gap so a
    /// flex reflow that ripples up to ancestors / sideways to siblings re-lowers
    /// their (now stale) retained boxes. Filled in `resolve`, drained in `render`
    /// after `commit_frame()`.
    pub(crate) layout_geometry_dirty: HashSet<ElementId>,
    /// Set by `register_font`; cleared at the start of the next `resolve`.
    /// Causes all text elements to be re-shaped with the newly registered font.
    pub(crate) fonts_dirty: bool,
}

impl ElementEngine {
    pub fn new() -> Self {
        Self {
            structure_dirty: HashSet::new(),
            shape_dirty: HashSet::new(),
            shape_lowering_reach: HashMap::new(),
            viewport_dirty: HashSet::new(),
            visual_dirty: HashMap::new(),
            layout_geometry_dirty: HashSet::new(),
            fonts_dirty: false,
        }
    }

    pub fn mark_structure_dirty(&mut self, id: ElementId) {
        self.structure_dirty.insert(id);
    }

    pub fn mark_shape_dirty(&mut self, id: ElementId, reach: VisualInvalidationReach) {
        self.shape_dirty.insert(id);
        visual_invalidation::merge_reach(&mut self.shape_lowering_reach, id, reach);
    }

    pub fn mark_viewport_dirty(&mut self, id: ElementId) {
        self.viewport_dirty.insert(id);
    }

    pub fn mark_visual_dirty(&mut self, id: ElementId, reach: VisualInvalidationReach) {
        visual_invalidation::merge_reach(&mut self.visual_dirty, id, reach);
    }

    pub fn mark_fonts_dirty(&mut self) {
        self.fonts_dirty = true;
    }

    pub fn drain_visual_dirty(&mut self) -> HashMap<ElementId, VisualInvalidationReach> {
        std::mem::take(&mut self.visual_dirty)
    }

    pub fn drain_layout_geometry_dirty(&mut self) -> HashSet<ElementId> {
        std::mem::take(&mut self.layout_geometry_dirty)
    }

    pub fn drain_shape_lowering_reach(&mut self) -> HashMap<ElementId, VisualInvalidationReach> {
        std::mem::take(&mut self.shape_lowering_reach)
    }

    /// Resolve dirty state and settle layout: Taffy projection reconcile +
    /// Parley shaping + layout-cache refresh (`LayoutPass::run()` equivalent,
    /// ADR-0075 scope A).
    pub fn resolve(
        &mut self,
        layout: &mut LayoutPass,
        elements: &mut HashMap<ElementId, Element>,
        root: ElementId,
        viewport: (f32, f32),
        event_queue: &mut Vec<Event>,
    ) {
        self.promote_viewport_dirty(layout, elements, viewport);
        // The reduced layout interface (issue #308 / §5): one `settle` folds
        // reconcile → compute → cache → geometry diff. The returned diff (boxes
        // that moved/resized or appeared) is folded into `layout_geometry_dirty`
        // so `render` can re-lower stale retained boxes — a flex reflow that
        // ripples to ancestors / siblings lands every moved id here independently.
        let geometry_dirty = layout.settle(
            elements,
            root,
            viewport,
            event_queue,
            &mut self.structure_dirty,
            &mut self.shape_dirty,
            &mut self.fonts_dirty,
        );
        self.layout_geometry_dirty.extend(geometry_dirty);
    }

    fn promote_viewport_dirty(
        &mut self,
        layout: &mut LayoutPass,
        elements: &HashMap<ElementId, Element>,
        viewport: (f32, f32),
    ) {
        for id in self.viewport_dirty.drain() {
            let Some(el) = elements.get(&id) else {
                continue;
            };
            let mut text_dirty = false;
            let mut own = el.visual.clone();
            for (condition, prop) in &el.viewport_variants {
                if condition.matches(viewport.0, viewport.1) {
                    apply_visual(&mut own, prop, &mut text_dirty);
                }
            }
            if text_dirty {
                self.shape_dirty.insert(id);
                visual_invalidation::merge_reach(
                    &mut self.shape_lowering_reach,
                    id,
                    VisualInvalidationReach::Subtree,
                );
                layout.projection.mark_dirty(id);
            } else {
                visual_invalidation::merge_reach(
                    &mut self.visual_dirty,
                    id,
                    VisualInvalidationReach::Subtree,
                );
            }
        }
    }
}

impl Default for ElementEngine {
    fn default() -> Self {
        Self::new()
    }
}
