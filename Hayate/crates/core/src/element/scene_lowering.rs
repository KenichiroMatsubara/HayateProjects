use std::collections::{HashMap, HashSet};

use crate::element::id::ElementId;
use crate::element::tree::ElementTree;
use crate::element::visual_invalidation::{
    self, VisualInvalidationReach,
};
use crate::node::{NodeId, SceneGraph};

#[derive(Debug, Clone)]
pub(crate) struct AnchorEntry {
    pub anchor_id: NodeId,
}

/// Retained element→scene lowering state (issue #182).
#[derive(Debug, Default)]
pub(crate) struct SceneLowering {
    pub anchors: std::collections::HashMap<ElementId, AnchorEntry>,
    pub built: bool,
    pub walk_count: usize,
}

impl SceneLowering {
    pub fn reset(&mut self) {
        self.anchors.clear();
        self.built = false;
        self.walk_count = 0;
    }
}

/// Dirty elements scheduled for scene re-lowering this frame.
#[derive(Debug, Default)]
pub(crate) struct LoweringDirtySnapshot {
    pub elements: HashMap<ElementId, VisualInvalidationReach>,
    pub z_index_reorder_parents: HashSet<ElementId>,
    pub fonts: bool,
    pub full_rebuild: bool,
}

pub(crate) fn collect_lowering_dirty(
    tree: &ElementTree,
    structure_dirty: &HashSet<ElementId>,
    shape_dirty: &HashSet<ElementId>,
    shape_lowering_reach: &HashMap<ElementId, VisualInvalidationReach>,
    viewport_dirty: &HashSet<ElementId>,
    visual_dirty: &HashMap<ElementId, VisualInvalidationReach>,
    fonts_dirty: bool,
) -> LoweringDirtySnapshot {
    let mut snapshot = LoweringDirtySnapshot::default();
    if fonts_dirty {
        snapshot.full_rebuild = true;
        return snapshot;
    }

    for (&id, &reach) in visual_dirty {
        visual_invalidation::apply_visual_invalidation(
            tree,
            id,
            reach,
            &mut snapshot.elements,
            &mut snapshot.z_index_reorder_parents,
        );
    }
    for &id in viewport_dirty {
        visual_invalidation::expand_subtree(tree, id, &mut snapshot.elements);
    }
    for &id in structure_dirty {
        visual_invalidation::expand_subtree(tree, id, &mut snapshot.elements);
    }
    for &id in shape_dirty {
        let reach = shape_lowering_reach
            .get(&id)
            .copied()
            .unwrap_or(VisualInvalidationReach::Subtree);
        visual_invalidation::apply_visual_invalidation(
            tree,
            id,
            reach,
            &mut snapshot.elements,
            &mut snapshot.z_index_reorder_parents,
        );
    }
    snapshot
}

pub(crate) fn clear_lowered_content(
    sg: &mut SceneGraph,
    anchor_id: NodeId,
    element_children: &[ElementId],
    lowering: &SceneLowering,
) {
    let preserve: HashSet<NodeId> = element_children
        .iter()
        .filter_map(|child| lowering.anchors.get(child).map(|e| e.anchor_id))
        .collect();
    let to_remove: Vec<NodeId> = sg
        .get(anchor_id)
        .map(|anchor| {
            anchor
                .children
                .iter()
                .copied()
                .filter(|id| !preserve.contains(id))
                .collect()
        })
        .unwrap_or_default();
    for id in to_remove {
        sg.remove_subtree(id);
    }
    if let Some(anchor) = sg.get_mut(anchor_id) {
        anchor.children.retain(|id| preserve.contains(id));
    }
}
