use std::collections::{HashMap, HashSet};

use taffy::{NodeId, TaffyTree};

use crate::element::id::ElementId;
use crate::element::inline_text::{is_ifc_root, is_inline_text_element};
use crate::element::taffy_bridge::MeasureCtx;
use crate::element::tree::Element;
use crate::element::visual_invalidation::{
    self, ElementMapTopology, VisualInvalidationReach,
};

/// Result of [`TaffyProjection::traversal_step`].
pub(crate) enum TraversalStep<'a> {
    /// `id` has no Taffy node; recurse into this element's children without
    /// yielding `id` itself.
    Skip(&'a Element),
    /// `id` has a Taffy node; yield it along with the element.
    Visit(NodeId, &'a Element),
}

/// Derived Taffy layout tree for the block-box subset of an `ElementTree`.
///
/// Inline text elements (text whose parent is also text) are excluded per ADR-0063/0064.
/// IFC roots (`text` under non-text parent) are measured leaves.
pub(crate) struct TaffyProjection {
    pub(crate) taffy: TaffyTree<MeasureCtx>,
    element_to_node: HashMap<ElementId, NodeId>,
    built: bool,
}

impl TaffyProjection {
    pub fn new() -> Self {
        Self {
            taffy: TaffyTree::new(),
            element_to_node: HashMap::new(),
            built: false,
        }
    }

    pub fn mark_dirty(&mut self, id: ElementId) {
        if let Some(node) = self.element_to_node.get(&id) {
            let _ = self.taffy.mark_dirty(*node);
        }
    }

    pub fn set_style(&mut self, id: ElementId, style: taffy::Style) {
        if let Some(node) = self.element_to_node.get(&id) {
            let _ = self.taffy.set_style(*node, style);
        }
    }

    pub fn has_node(&self, id: ElementId) -> bool {
        self.element_to_node.contains_key(&id)
    }

    pub fn node_id(&self, id: ElementId) -> Option<NodeId> {
        self.element_to_node.get(&id).copied()
    }

    /// Shared skeleton for the three Canonical Tree traversals (`scene_build`,
    /// `walk_resolved`, `walk_accessibility`): look up `id`'s Taffy node.
    ///
    /// If `id` has no element, returns `None` — callers should stop. If `id`
    /// has no Taffy node (e.g. an inline text element inside an IFC), returns
    /// `Skip` so callers recurse into `id`'s children without yielding `id`
    /// itself, mirroring `layout_pass::cache_layout`. Otherwise returns
    /// `Visit` with the Taffy node and the element to yield.
    pub fn traversal_step<'a>(
        &self,
        elements: &'a HashMap<ElementId, Element>,
        id: ElementId,
    ) -> Option<TraversalStep<'a>> {
        let el = elements.get(&id)?;
        match self.node_id(id) {
            Some(node) => Some(TraversalStep::Visit(node, el)),
            None => Some(TraversalStep::Skip(el)),
        }
    }

    /// Reconcile the Taffy projection when structure has changed.
    ///
    /// `structure_dirty` is owned by `ElementEngine` (ADR-0075); this drains it.
    pub fn reconcile(
        &mut self,
        elements: &HashMap<ElementId, Element>,
        root: ElementId,
        structure_dirty: &mut HashSet<ElementId>,
    ) {
        if self.built && structure_dirty.is_empty() {
            return;
        }

        if !self.built {
            if elements.contains_key(&root) {
                build_subtree(self, elements, root);
            }
            self.built = true;
            structure_dirty.clear();
            return;
        }

        // Structure changes always carry `Subtree` reach, so tagging every
        // drained id `Subtree` and routing through the shared reach seam yields
        // exactly the old presence-only patch-root search (`step_reach` returns
        // `Some(Subtree)` for every ancestor→descendant step). The reach kernel
        // now lives in one place; this is its `structure_dirty` specialization.
        let dirty: HashMap<ElementId, VisualInvalidationReach> = structure_dirty
            .drain()
            .map(|id| (id, VisualInvalidationReach::Subtree))
            .collect();
        let topology = ElementMapTopology { elements };
        let patch_roots = visual_invalidation::minimal_patch_roots(&topology, &dirty);
        for patch_root in patch_roots {
            patch_subtree(self, elements, patch_root);
        }
        prune_orphan_projections(self, elements);
    }
}

impl Default for TaffyProjection {
    fn default() -> Self {
        Self::new()
    }
}

fn patch_subtree(
    projection: &mut TaffyProjection,
    elements: &HashMap<ElementId, Element>,
    id: ElementId,
) {
    if !elements.contains_key(&id) {
        purge_element_projection(projection, id);
        return;
    }

    if is_inline_text_element(elements, id) {
        if let Some(children) = elements.get(&id).map(|el| el.children.clone()) {
            for child in children {
                patch_subtree(projection, elements, child);
            }
        }
        return;
    }

    let existing_node = projection.element_to_node.get(&id).copied();

    let node = if let Some(node) = existing_node {
        clear_taffy_children(&mut projection.taffy, node);
        sync_node_from_element(projection, elements, id, node);
        node
    } else {
        let node = create_projected_node(projection, elements, id);
        projection.element_to_node.insert(id, node);
        if let Some(parent_id) = elements.get(&id).and_then(|e| e.parent) {
            if let Some(parent_node) = projection.node_id(parent_id) {
                let _ = projection.taffy.add_child(parent_node, node);
            }
        }
        node
    };

    for child in elements
        .get(&id)
        .map(|el| el.children.clone())
        .unwrap_or_default()
    {
        if let Some(child_node) = build_subtree(projection, elements, child) {
            let _ = projection.taffy.add_child(node, child_node);
        }
    }
}

/// Drop stale `element_to_node` mapping for a removed element.
///
/// Taffy nodes for removed subtrees are detached during the ancestor's
/// `clear_taffy_children` in `patch_subtree` (ADR-0064 lazy reconcile).
fn purge_element_projection(projection: &mut TaffyProjection, id: ElementId) {
    projection.element_to_node.remove(&id);
}

/// Remove `element_to_node` entries whose elements were deleted before reconcile.
fn prune_orphan_projections(
    projection: &mut TaffyProjection,
    elements: &HashMap<ElementId, Element>,
) {
    projection
        .element_to_node
        .retain(|id, _| elements.contains_key(id));
}

fn clear_taffy_children(taffy: &mut TaffyTree<MeasureCtx>, node: NodeId) {
    loop {
        let children = match taffy.children(node) {
            Ok(c) => c,
            Err(_) => break,
        };
        if children.is_empty() {
            break;
        }
        for child in children {
            remove_taffy_subtree(taffy, child);
        }
    }
}

fn remove_taffy_subtree(taffy: &mut TaffyTree<MeasureCtx>, node: NodeId) {
    if let Ok(children) = taffy.children(node) {
        for child in children {
            remove_taffy_subtree(taffy, child);
        }
    }
    let _ = taffy.remove(node);
}

fn sync_node_from_element(
    projection: &mut TaffyProjection,
    elements: &HashMap<ElementId, Element>,
    id: ElementId,
    node: NodeId,
) {
    let el = match elements.get(&id) {
        Some(e) => e,
        None => return,
    };
    let measure_ctx = if is_ifc_root(elements, id) {
        MeasureCtx::Text(id)
    } else {
        MeasureCtx::None
    };
    let _ = projection.taffy.set_style(node, el.layout_style.clone());
    let _ = projection
        .taffy
        .set_node_context(node, Some(measure_ctx));
}

fn create_projected_node(
    projection: &mut TaffyProjection,
    elements: &HashMap<ElementId, Element>,
    id: ElementId,
) -> NodeId {
    let el = elements.get(&id).expect("create_projected_node: missing element");
    let measure_ctx = if is_ifc_root(elements, id) {
        MeasureCtx::Text(id)
    } else {
        MeasureCtx::None
    };
    projection
        .taffy
        .new_leaf_with_context(el.layout_style.clone(), measure_ctx)
        .expect("taffy new_leaf_with_context")
}

fn build_subtree(
    projection: &mut TaffyProjection,
    elements: &HashMap<ElementId, Element>,
    id: ElementId,
) -> Option<NodeId> {
    if is_inline_text_element(elements, id) {
        let children = elements
            .get(&id)
            .map(|el| el.children.clone())
            .unwrap_or_default();
        for child in children {
            build_subtree(projection, elements, child);
        }
        return None;
    }

    let node = create_projected_node(projection, elements, id);
    projection.element_to_node.insert(id, node);

    let children = elements
        .get(&id)
        .map(|el| el.children.clone())
        .unwrap_or_default();
    for child in children {
        if let Some(child_node) = build_subtree(projection, elements, child) {
            let _ = projection.taffy.add_child(node, child_node);
        }
    }

    Some(node)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::kind::ElementKind;
    use crate::element::tree::Visual;

    fn make_view(id: u64, parent: Option<ElementId>) -> (ElementId, Element) {
        let eid = ElementId::from_u64(id);
        let el = Element {
            kind: ElementKind::View,
            parent,
            children: Vec::new(),
            layout_style: taffy::Style::default(),
            visual: Visual::default(),
            text: None,
            src: None,
            text_layout: None,
            transform: None,
            scroll_offset: (0.0, 0.0),
            src_image: None,
            edit: None,
            cursor_visible: false,
            content_layout: None,
            aria_label: None,
            role: None,
            pseudo_styles: Default::default(),
            disabled: false,
            selectable: false,
            viewport_variants: Vec::new(),
        };
        (eid, el)
    }

    fn make_text(id: u64, parent: Option<ElementId>, text: &str) -> (ElementId, Element) {
        let eid = ElementId::from_u64(id);
        let el = Element {
            kind: ElementKind::Text,
            parent,
            children: Vec::new(),
            layout_style: taffy::Style::default(),
            visual: Visual::default(),
            text: Some(text.to_string()),
            src: None,
            text_layout: None,
            transform: None,
            scroll_offset: (0.0, 0.0),
            src_image: None,
            edit: None,
            cursor_visible: false,
            content_layout: None,
            aria_label: None,
            role: None,
            pseudo_styles: Default::default(),
            disabled: false,
            selectable: false,
            viewport_variants: Vec::new(),
        };
        (eid, el)
    }

    #[test]
    fn reconcile_excludes_inline_text_from_projection_map() {
        let mut projection = TaffyProjection::new();
        let mut elements = HashMap::new();

        let (root_id, root) = make_text(1, None, "outer");
        let (inline_id, inline) = make_text(2, Some(root_id), "inner");
        elements.insert(root_id, root);
        elements.insert(inline_id, inline);
        elements
            .get_mut(&root_id)
            .unwrap()
            .children
            .push(inline_id);

        projection.reconcile(&elements, root_id, &mut HashSet::new());

        assert!(!projection.has_node(inline_id));
        assert!(projection.has_node(root_id));
    }

    #[test]
    fn reconcile_after_subtree_removal_clears_stale_projection_entries() {
        let mut projection = TaffyProjection::new();
        let mut elements = HashMap::new();

        let (root_id, mut root) = make_view(1, None);
        let (branch_id, branch) = make_view(2, Some(root_id));
        let (leaf_id, leaf) = make_view(3, Some(branch_id));
        root.children = vec![branch_id];
        elements.insert(root_id, root);
        elements.insert(branch_id, branch);
        elements.insert(leaf_id, leaf);
        elements
            .get_mut(&branch_id)
            .unwrap()
            .children
            .push(leaf_id);

        projection.reconcile(&elements, root_id, &mut HashSet::new());
        assert!(projection.has_node(branch_id));
        assert!(projection.has_node(leaf_id));

        // Simulate `element_remove(branch)` after detach: parent stays, subtree gone.
        elements.get_mut(&root_id).unwrap().children.clear();
        elements.remove(&branch_id);
        elements.remove(&leaf_id);

        let mut structure_dirty = HashSet::new();
        structure_dirty.insert(root_id);
        structure_dirty.insert(branch_id);

        projection.reconcile(&elements, root_id, &mut structure_dirty);

        assert!(
            !projection.has_node(branch_id),
            "removed branch projection must be cleared"
        );
        assert!(
            !projection.has_node(leaf_id),
            "removed descendant projection must be cleared"
        );

        // Second reconcile must not panic on stale SlotMap keys.
        projection.reconcile(&elements, root_id, &mut HashSet::new());
    }

    #[test]
    fn reconcile_append_to_deep_branch_preserves_unrelated_node_ids() {
        let mut projection = TaffyProjection::new();
        let mut elements = HashMap::new();

        let (root_id, mut root) = make_view(1, None);
        let (branch_a_id, branch_a) = make_view(2, Some(root_id));
        let (branch_b_id, branch_b) = make_view(3, Some(root_id));
        root.children = vec![branch_a_id, branch_b_id];
        elements.insert(root_id, root);
        elements.insert(branch_a_id, branch_a);
        elements.insert(branch_b_id, branch_b);

        projection.reconcile(&elements, root_id, &mut HashSet::new());
        let branch_b_node_before = projection
            .node_id(branch_b_id)
            .expect("branch_b must be projected");

        let (new_child_id, new_child) = make_view(4, Some(branch_a_id));
        elements
            .get_mut(&branch_a_id)
            .unwrap()
            .children
            .push(new_child_id);
        elements.insert(new_child_id, new_child);
        let mut structure_dirty = HashSet::new();
        structure_dirty.insert(branch_a_id);
        structure_dirty.insert(new_child_id);

        projection.reconcile(&elements, root_id, &mut structure_dirty);

        let branch_b_node_after = projection
            .node_id(branch_b_id)
            .expect("branch_b must remain projected");
        assert_eq!(
            branch_b_node_before, branch_b_node_after,
            "append under sibling branch must not rebuild unrelated Taffy nodes"
        );
        assert!(
            projection.has_node(new_child_id),
            "new child must be projected"
        );
    }
}
