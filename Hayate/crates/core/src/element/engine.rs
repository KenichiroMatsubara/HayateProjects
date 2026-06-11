use std::collections::{HashMap, HashSet};

use crate::element::id::ElementId;
use crate::element::layout_pass::{cache_layout, LayoutPass};
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
    /// Elements whose viewport-conditioned own-style changed on resize (ADR-0081).
    pub(crate) viewport_dirty: HashSet<ElementId>,
    /// Set by `register_font`; cleared at the start of the next `resolve`.
    /// Causes all text elements to be re-shaped with the newly registered font.
    pub(crate) fonts_dirty: bool,
}

impl ElementEngine {
    pub fn new() -> Self {
        Self {
            structure_dirty: HashSet::new(),
            shape_dirty: HashSet::new(),
            viewport_dirty: HashSet::new(),
            fonts_dirty: false,
        }
    }

    pub fn mark_structure_dirty(&mut self, id: ElementId) {
        self.structure_dirty.insert(id);
    }

    pub fn mark_shape_dirty(&mut self, id: ElementId) {
        self.shape_dirty.insert(id);
    }

    pub fn mark_viewport_dirty(&mut self, id: ElementId) {
        self.viewport_dirty.insert(id);
    }

    pub fn mark_fonts_dirty(&mut self) {
        self.fonts_dirty = true;
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
        layout
            .projection
            .reconcile(&*elements, root, &mut self.structure_dirty);
        layout.compute(
            elements,
            root,
            viewport,
            event_queue,
            &mut self.shape_dirty,
            &mut self.fonts_dirty,
        );
        layout.layout_cache.clear();
        cache_layout(
            elements,
            &layout.projection,
            root,
            0.0,
            0.0,
            &mut layout.layout_cache,
        );
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
                layout.projection.mark_dirty(id);
            }
        }
    }
}

impl Default for ElementEngine {
    fn default() -> Self {
        Self::new()
    }
}
