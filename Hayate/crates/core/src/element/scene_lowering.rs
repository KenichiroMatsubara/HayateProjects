use std::collections::HashSet;

use crate::element::id::ElementId;
use crate::element::pseudo_state::InteractionSnapshot;
use crate::element::tree::ElementTree;
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
    pub last_interaction: InteractionSnapshot,
}

impl SceneLowering {
    pub fn reset(&mut self) {
        self.anchors.clear();
        self.built = false;
        self.walk_count = 0;
        self.last_interaction = InteractionSnapshot::default();
    }
}

/// Dirty elements scheduled for scene re-lowering this frame.
#[derive(Debug, Default)]
pub(crate) struct LoweringDirtySnapshot {
    pub elements: HashSet<ElementId>,
    pub fonts: bool,
    pub full_rebuild: bool,
}

pub(crate) fn collect_lowering_dirty(
    tree: &ElementTree,
    structure_dirty: &HashSet<ElementId>,
    shape_dirty: &HashSet<ElementId>,
    viewport_dirty: &HashSet<ElementId>,
    visual_dirty: &HashSet<ElementId>,
    fonts_dirty: bool,
    interaction: &InteractionSnapshot,
    last_interaction: &InteractionSnapshot,
    cursor_dirty: Option<ElementId>,
) -> LoweringDirtySnapshot {
    let mut snapshot = LoweringDirtySnapshot::default();
    if fonts_dirty {
        snapshot.full_rebuild = true;
        return snapshot;
    }

    for &id in visual_dirty {
        snapshot.elements.insert(id);
    }
    for &id in viewport_dirty {
        snapshot.elements.insert(id);
        expand_descendants(tree, id, &mut snapshot.elements);
    }
    for &id in structure_dirty {
        snapshot.elements.insert(id);
        expand_descendants(tree, id, &mut snapshot.elements);
    }
    for &id in shape_dirty {
        snapshot.elements.insert(id);
        expand_descendants(tree, id, &mut snapshot.elements);
    }
    if let Some(id) = cursor_dirty {
        snapshot.elements.insert(id);
    }
    if interaction != last_interaction {
        collect_pseudo_dirty(tree, interaction, last_interaction, &mut snapshot.elements);
    }
    snapshot
}

fn expand_descendants(tree: &ElementTree, root: ElementId, out: &mut HashSet<ElementId>) {
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        if !out.insert(id) {
            continue;
        }
        if let Some(el) = tree.elements.get(&id) {
            stack.extend(el.children.iter().copied());
        }
    }
}

fn collect_pseudo_dirty(
    tree: &ElementTree,
    interaction: &InteractionSnapshot,
    last: &InteractionSnapshot,
    out: &mut HashSet<ElementId>,
) {
    let mut candidates = HashSet::new();
    candidates.extend(interaction.hovered.iter().copied());
    candidates.extend(last.hovered.iter().copied());
    if let Some(id) = interaction.active {
        candidates.insert(id);
    }
    if let Some(id) = last.active {
        candidates.insert(id);
    }
    if let Some(id) = interaction.focused {
        candidates.insert(id);
    }
    if let Some(id) = last.focused {
        candidates.insert(id);
    }
    for id in candidates {
        if tree.elements.get(&id).is_some_and(|el| {
            !el.pseudo_styles.hover.is_empty()
                || !el.pseudo_styles.active.is_empty()
                || !el.pseudo_styles.focus.is_empty()
        })
        {
            out.insert(id);
        }
    }
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
