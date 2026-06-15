use crate::color::Color;
use crate::element::effective_visual::{
    self, child_inherited_context, InheritedVisualContext,
};
use crate::element::style::{BorderStyleValue, OverflowValue, Shadow};
use crate::element::id::ElementId;
use crate::element::kind::ElementKind;
use crate::element::scene_lowering::{
    clear_lowered_content, AnchorEntry, LoweringDirtySnapshot, SceneLowering,
};
use crate::element::visual_invalidation::{self, VisualInvalidationReach};
use crate::element::taffy_projection::TraversalStep;
use crate::element::tree::ElementTree;
use crate::node::{Node, NodeId, NodeKind, SceneGraph};
use std::collections::HashSet;

/// Full ephemeral rebuild without retained anchors (parity reference / tests).
pub fn build_ephemeral(tree: &ElementTree) -> SceneGraph {
    let mut sg = SceneGraph::new();
    let interaction = tree.interaction_snapshot();
    if let Some(root) = tree.root() {
        walk_ephemeral(
            tree,
            root,
            0.0,
            0.0,
            &mut sg,
            None,
            InheritedVisualContext::root(),
            &interaction,
        );
    }
    sg
}

/// Incrementally update a scene graph using retained element anchors.
///
/// `now_ms` is the host clock driving in-flight transitions; the per-element
/// `resolve_effective` seam diffs the resolved visual against the stored
/// displayed value to start/advance interpolation (ADR-0093).
pub(crate) fn update(
    tree: &ElementTree,
    scene_cache: &mut SceneGraph,
    lowering: &mut SceneLowering,
    dirty: LoweringDirtySnapshot,
    now_ms: f64,
) {
    lowering.walk_count = 0;
    let interaction = tree.interaction_snapshot();

    if dirty.full_rebuild || !lowering.built {
        *scene_cache = SceneGraph::new();
        lowering.anchors.clear();
        if let Some(root) = tree.root() {
            walk_retained(
                tree,
                root,
                0.0,
                0.0,
                scene_cache,
                lowering,
                None,
                InheritedVisualContext::root(),
                &interaction,
                VisualInvalidationReach::Subtree,
                now_ms,
            );
        }
        lowering.built = true;
        return;
    }

    if dirty.elements.is_empty() {
        return;
    }

    for &parent_id in &dirty.z_index_reorder_parents {
        reorder_children_for_z_index(tree, scene_cache, lowering, parent_id);
    }

    let patch_roots = minimal_patch_roots(tree, &dirty.elements);
    for patch_root in patch_roots {
        let reach = dirty
            .elements
            .get(&patch_root)
            .copied()
            .unwrap_or(VisualInvalidationReach::Subtree);
        let parent_anchor = tree
            .elements
            .get(&patch_root)
            .and_then(|el| el.parent)
            .and_then(|parent| lowering.anchors.get(&parent).map(|entry| entry.anchor_id));
        let (ox, oy) = tree
            .elements
            .get(&patch_root)
            .and_then(|el| el.parent)
            .and_then(|parent| tree.layout.layout_cache.get(&parent))
            .map(|(x, y, _, _)| (*x, *y))
            .unwrap_or((0.0, 0.0));
        walk_retained(
            tree,
            patch_root,
            ox,
            oy,
            scene_cache,
            lowering,
            parent_anchor,
            inherited_for_patch_root(tree, patch_root),
            &interaction,
            reach,
            now_ms,
        );
    }
}

fn reorder_children_for_z_index(
    tree: &ElementTree,
    sg: &mut SceneGraph,
    lowering: &SceneLowering,
    parent_id: ElementId,
) {
    let Some(parent_entry) = lowering.anchors.get(&parent_id) else {
        return;
    };
    let parent_anchor = parent_entry.anchor_id;
    let ordered = tree.ordered_children(parent_id);
    let child_anchors: Vec<NodeId> = ordered
        .iter()
        .filter_map(|child| lowering.anchors.get(child).map(|e| e.anchor_id))
        .collect();
    if let Some(parent) = sg.get_mut(parent_anchor) {
        parent.children = child_anchors;
    }
}

fn inherited_for_patch_root(tree: &ElementTree, id: ElementId) -> InheritedVisualContext {
    let parent = tree.elements.get(&id).and_then(|el| el.parent);
    match parent {
        Some(parent_id) => {
            let parent_el = &tree.elements[&parent_id];
            let parent_ctx = inherited_for_patch_root(tree, parent_id);
            let inherited_base =
                effective_visual::apply_text_inheritance(&parent_ctx, &parent_el.visual);
            child_inherited_context(
                &parent_ctx,
                parent_el.kind,
                &inherited_base,
                &parent_el.visual,
            )
        }
        None => InheritedVisualContext::root(),
    }
}

fn minimal_patch_roots(
    tree: &ElementTree,
    dirty: &std::collections::HashMap<ElementId, VisualInvalidationReach>,
) -> Vec<ElementId> {
    dirty
        .keys()
        .copied()
        .filter(|&id| !covered_by_dirty_ancestor(tree, id, dirty))
        .collect()
}

/// Whether re-walking some dirty ancestor will itself re-emit `id`, so `id` need
/// not be its own patch root. True only when the ancestor's reach actually
/// propagates down the ancestor→id path: a `SelfOnly` / `ZIndex` ancestor
/// re-emits only itself, so a dirty descendant under it (e.g. an independent
/// in-flight transition beneath a transitioning parent, issue #228) must remain
/// its own patch root or its re-lowering would be skipped.
fn covered_by_dirty_ancestor(
    tree: &ElementTree,
    id: ElementId,
    dirty: &std::collections::HashMap<ElementId, VisualInvalidationReach>,
) -> bool {
    // Path id → root: chain[0] = id, chain[i+1] = parent(chain[i]).
    let mut chain = vec![id];
    let mut current = tree.elements.get(&id).and_then(|el| el.parent);
    while let Some(parent) = current {
        chain.push(parent);
        current = tree.elements.get(&parent).and_then(|el| el.parent);
    }
    // For each dirty ancestor, simulate the reach propagating down to `id` via
    // the single-source `step_reach`, exactly as the retained walk would.
    for (ancestor_idx, &ancestor) in chain.iter().enumerate().skip(1) {
        let Some(&ancestor_reach) = dirty.get(&ancestor) else {
            continue;
        };
        let mut reach = ancestor_reach;
        let mut parent = ancestor;
        let mut reached = true;
        for child_idx in (0..ancestor_idx).rev() {
            let child = chain[child_idx];
            match visual_invalidation::step_reach(
                reach,
                tree.element_context(parent),
                tree.element_context(child),
            ) {
                Some(next) => {
                    reach = next;
                    parent = child;
                }
                None => {
                    reached = false;
                    break;
                }
            }
        }
        if reached {
            return true;
        }
    }
    false
}

fn first_child_matching(
    sg: &SceneGraph,
    parent: NodeId,
    pred: impl Fn(&NodeKind) -> bool,
) -> Option<NodeId> {
    let parent_node = sg.get(parent)?;
    parent_node.children.iter().copied().find(|&child| {
        sg.get(child).is_some_and(|n| pred(&n.kind))
    })
}

/// Node under which child element anchors should attach — follows Clip/scroll Group
/// wrappers when the parent is a ScrollView (issue #199).
fn find_content_attachment_point(
    sg: &SceneGraph,
    anchor_id: NodeId,
    el: &crate::element::tree::Element,
) -> NodeId {
    let mut node = anchor_id;
    if el.transform.is_some() {
        node = first_child_matching(sg, node, |kind| matches!(kind, NodeKind::Group { .. }))
            .unwrap_or(node);
    }
    if el.kind == ElementKind::ScrollView {
        node = first_child_matching(sg, node, |kind| matches!(kind, NodeKind::Clip { .. }))
            .unwrap_or(node);
        let (sx, sy) = el.scroll_offset;
        if sx != 0.0 || sy != 0.0 {
            node = first_child_matching(sg, node, |kind| matches!(kind, NodeKind::Group { .. }))
                .unwrap_or(node);
        }
    }
    node
}

fn resolve_parent_attachment(
    tree: &ElementTree,
    sg: &SceneGraph,
    lowering: &SceneLowering,
    id: ElementId,
    parent_anchor: Option<NodeId>,
) -> Option<NodeId> {
    let parent_id = tree.elements.get(&id).and_then(|el| el.parent)?;
    let parent_entry = lowering.anchors.get(&parent_id)?;
    let parent_el = tree.elements.get(&parent_id)?;
    Some(find_content_attachment_point(
        sg,
        parent_entry.anchor_id,
        parent_el,
    ))
    .or(parent_anchor)
}

fn ensure_anchor(
    tree: &ElementTree,
    sg: &mut SceneGraph,
    lowering: &mut SceneLowering,
    id: ElementId,
    parent_anchor: Option<NodeId>,
) -> NodeId {
    let attach_parent = resolve_parent_attachment(tree, sg, lowering, id, parent_anchor);
    if let Some(entry) = lowering.anchors.get(&id) {
        let anchor_id = entry.anchor_id;
        if let Some(parent) = attach_parent {
            insert_anchor_ordered(tree, sg, lowering, id, parent, anchor_id);
        }
        return anchor_id;
    }

    let anchor_id = sg.insert(Node {
        kind: NodeKind::ElementAnchor { element_id: id },
        children: Vec::new(),
    });
    if let Some(parent) = attach_parent {
        insert_anchor_ordered(tree, sg, lowering, id, parent, anchor_id);
    }
    lowering.anchors.insert(id, AnchorEntry::new(anchor_id));
    anchor_id
}

/// Attach `child` (the anchor for element `id`) under `parent` at the scene-child
/// index that matches `id`'s position among its element siblings.
///
/// A partial patch re-walks only some of a parent's children (e.g. a hovered card,
/// or the grown/pushed siblings of an insert). Blindly appending a re-walked anchor
/// to the end of `parent.children` scrambles paint order, so the interacted element
/// paints over the wrong sibling — the "hover/click corrupts a *different* element"
/// symptom. Positioning relative to the preceding sibling *anchors actually present
/// under `parent`* keeps the retained child order in lockstep with element order and
/// is robust to Clip/Group content-attachment wrappers (all siblings share one
/// attachment point).
fn insert_anchor_ordered(
    tree: &ElementTree,
    sg: &mut SceneGraph,
    lowering: &SceneLowering,
    id: ElementId,
    parent: NodeId,
    child: NodeId,
) {
    sg.retain_roots(|root| root != child);
    if let Some(old_parent) = sg.parent_of(child) {
        if let Some(p) = sg.get_mut(old_parent) {
            p.children.retain(|&c| c != child);
        }
    }
    // Anchors of siblings that follow `id` in element order. Insert `child` just
    // before the first one present under `parent`; if none are present yet, append.
    // Inserting *before following siblings* (rather than *after preceding ones*)
    // keeps the parent's own content nodes — fill/border emitted before any child
    // anchor — ahead of every child, so the box still paints under its children.
    let following: HashSet<NodeId> = tree
        .elements
        .get(&id)
        .and_then(|el| el.parent)
        .map(|p| tree.ordered_children(p))
        .unwrap_or_default()
        .into_iter()
        .skip_while(|&sib| sib != id)
        .skip(1)
        .filter_map(|sib| lowering.anchors.get(&sib).map(|e| e.anchor_id))
        .collect();
    if let Some(p) = sg.get_mut(parent) {
        let index = p
            .children
            .iter()
            .position(|c| following.contains(c))
            .unwrap_or(p.children.len());
        p.children.insert(index, child);
    }
}

fn attach_under(sg: &mut SceneGraph, parent: NodeId, child: NodeId) {
    sg.retain_roots(|root| root != child);
    if let Some(old_parent) = sg.parent_of(child) {
        if let Some(p) = sg.get_mut(old_parent) {
            p.children.retain(|&id| id != child);
        }
    }
    if let Some(p) = sg.get_mut(parent) {
        if !p.children.contains(&child) {
            p.children.push(child);
        }
    }
}

/// Re-stack a re-walked element's child anchors after its own content, in element
/// order. `emit_element` emits the box's own content (fill/border/text) by
/// appending, but `clear_lowered_content` preserves child anchors at the front of
/// the list — so without this pass the box's own fill paints *over* its children
/// (and stale sibling order survives). Re-attaching every child in element order
/// after content emission restores `[content..., child0, child1, ...]`.
///
/// Also handles the Clip/scroll-Group wrapper case it was written for: when
/// `effective_parent` is a wrapper inside the anchor, children slide under the
/// wrapper so clipping still applies.
fn reparent_child_anchors_under(
    sg: &mut SceneGraph,
    _anchor_id: NodeId,
    effective_parent: Option<NodeId>,
    children: &[ElementId],
    lowering: &SceneLowering,
) {
    let Some(parent) = effective_parent else {
        return;
    };
    for &child_id in children {
        let Some(child_anchor) = lowering.anchors.get(&child_id).map(|e| e.anchor_id) else {
            continue;
        };
        attach_under(sg, parent, child_anchor);
    }
}

fn walk_retained(
    tree: &ElementTree,
    id: ElementId,
    ox: f32,
    oy: f32,
    sg: &mut SceneGraph,
    lowering: &mut SceneLowering,
    parent_anchor: Option<NodeId>,
    inherited: InheritedVisualContext,
    interaction: &crate::element::pseudo_state::InteractionSnapshot,
    reach: VisualInvalidationReach,
    now_ms: f64,
) {
    lowering.walk_count += 1;

    let (taffy_node, el) = match tree.layout.projection.traversal_step(&tree.elements, id) {
        Some(TraversalStep::Visit(taffy_node, el)) => (taffy_node, el),
        Some(TraversalStep::Skip(_)) => {
            let anchor_id = ensure_anchor(tree, sg, lowering, id, parent_anchor);
            let children = tree.ordered_children(id);
            clear_lowered_content(sg, anchor_id, &children, lowering);
            for (child, child_reach) in children_for_reach(tree, id, reach) {
                walk_retained(
                    tree,
                    child,
                    ox,
                    oy,
                    sg,
                    lowering,
                    Some(anchor_id),
                    inherited.clone(),
                    interaction,
                    child_reach,
                    now_ms,
                );
            }
            return;
        }
        None => return,
    };

    let anchor_id = ensure_anchor(tree, sg, lowering, id, parent_anchor);
    let children = tree.ordered_children(id);
    clear_lowered_content(sg, anchor_id, &children, lowering);

    emit_element(
        tree,
        id,
        el,
        taffy_node,
        ox,
        oy,
        sg,
        lowering,
        anchor_id,
        inherited,
        interaction,
        reach,
        now_ms,
    );
}

/// The children a retained walk descends into under `reach`, each paired with
/// the reach it carries — derived entirely from the single-source `step_reach`.
fn children_for_reach(
    tree: &ElementTree,
    id: ElementId,
    reach: VisualInvalidationReach,
) -> Vec<(ElementId, VisualInvalidationReach)> {
    let parent_ctx = tree.element_context(id);
    tree.ordered_children(id)
        .into_iter()
        .filter_map(|child| {
            visual_invalidation::step_reach(reach, parent_ctx, tree.element_context(child))
                .map(|child_reach| (child, child_reach))
        })
        .collect()
}

fn emit_element(
    tree: &ElementTree,
    id: ElementId,
    el: &crate::element::tree::Element,
    taffy_node: taffy::NodeId,
    ox: f32,
    oy: f32,
    sg: &mut SceneGraph,
    lowering: &mut SceneLowering,
    anchor_id: NodeId,
    inherited: InheritedVisualContext,
    interaction: &crate::element::pseudo_state::InteractionSnapshot,
    reach: VisualInvalidationReach,
    now_ms: f64,
) {
    let inherited_base = effective_visual::apply_text_inheritance(&inherited, &el.visual);
    let child_inherited = child_inherited_context(
        &inherited,
        el.kind,
        &inherited_base,
        &el.visual,
    );
    let own = effective_visual::own_with_viewport_variants(
        &el.visual,
        &el.viewport_variants,
        tree.viewport(),
    );
    let resolved = effective_visual::resolve_effective(
        &inherited,
        &own,
        &el.pseudo_styles,
        interaction,
        id,
    );
    // Diff the after-change resolved visual against the previous frame's
    // displayed value at the resolve seam, interpolating changed continuous
    // properties (ADR-0093). The retained anchor holds the before-change value.
    let visual = lowering
        .anchors
        .get_mut(&id)
        .map(|entry| entry.resolve_displayed(&resolved, now_ms))
        .unwrap_or(resolved);
    let layout = match tree.layout.projection.taffy.layout(taffy_node) {
        Ok(l) => l,
        Err(_) => return,
    };
    let x = ox + layout.location.x;
    let y = oy + layout.location.y;
    let w = layout.size.width;
    let h = layout.size.height;

    let confirmed_color = visual.text_color.unwrap_or(Color::BLACK);
    let confirmed_font_size = visual.font_size.unwrap_or(16.0);

    let mut effective_parent = Some(anchor_id);
    if let Some(transform) = el.transform {
        let group_id = emit(
            sg,
            effective_parent,
            Node {
                kind: NodeKind::Group { transform },
                children: Vec::new(),
            },
        );
        effective_parent = Some(group_id);
    }

    let effective_parent = if el.kind == ElementKind::ScrollView {
        let clip_id = emit(
            sg,
            effective_parent,
            Node {
                kind: NodeKind::Clip {
                    x,
                    y,
                    width: w,
                    height: h,
                    corner_radii: [0.0; 4],
                },
                children: Vec::new(),
            },
        );
        let (sx, sy) = el.scroll_offset;
        if sx != 0.0 || sy != 0.0 {
            let scroll_group = emit(
                sg,
                Some(clip_id),
                Node {
                    kind: NodeKind::Group {
                        transform: [1.0, 0.0, 0.0, 1.0, -sx as f64, -sy as f64],
                    },
                    children: Vec::new(),
                },
            );
            Some(scroll_group)
        } else {
            Some(clip_id)
        }
    } else if visual.overflow == OverflowValue::Hidden {
        let clip_id = emit(
            sg,
            effective_parent,
            Node {
                kind: NodeKind::Clip {
                    x,
                    y,
                    width: w,
                    height: h,
                    corner_radii: [visual.border_radius; 4],
                },
                children: Vec::new(),
            },
        );
        Some(clip_id)
    } else {
        effective_parent
    };

    if !visual.box_shadow.is_empty() {
        emit_box_shadows(
            sg,
            effective_parent,
            x,
            y,
            w,
            h,
            visual.border_radius,
            &visual.box_shadow,
            visual.opacity,
            false,
        );
    }

    emit_visual_box(
        sg,
        effective_parent,
        x,
        y,
        w,
        h,
        visual.border_radius,
        visual.border_width,
        visual.background_color,
        visual.border_color,
        visual.border_style,
        visual.opacity,
    );

    if !visual.box_shadow.is_empty() {
        emit_box_shadows(
            sg,
            effective_parent,
            x,
            y,
            w,
            h,
            visual.border_radius,
            &visual.box_shadow,
            visual.opacity,
            true,
        );
    }

    if el.kind == ElementKind::Image {
        if let Some(img) = el.src_image.clone() {
            emit(
                sg,
                effective_parent,
                Node {
                    kind: NodeKind::Image {
                        x,
                        y,
                        width: w,
                        height: h,
                        data: img,
                    },
                    children: Vec::new(),
                },
            );
        }
        for (child, child_reach) in children_for_reach(tree, id, reach) {
            walk_retained(
                tree,
                child,
                x,
                y,
                sg,
                lowering,
                effective_parent,
                child_inherited.clone(),
                interaction,
                child_reach,
                now_ms,
            );
        }
        reparent_child_anchors_under(
            sg,
            anchor_id,
            effective_parent,
            &tree.ordered_children(id),
            lowering,
        );
        return;
    }

    if el.kind == ElementKind::TextInput {
        let content_x = x + layout.border.left + layout.padding.left;
        let content_y = y + layout.border.top + layout.padding.top;
        let color = confirmed_color
            .with_opacity(visual.opacity)
            .to_array_f32();
        let runs = if let Some(cl) = el.content_layout.as_ref() {
            Some(cl.runs.as_slice())
        } else {
            el.text_layout.as_ref().map(|tl| tl.runs.as_slice())
        };
        if let Some(runs) = runs {
            for run in runs {
                emit(
                    sg,
                    effective_parent,
                    Node {
                        kind: NodeKind::TextRun {
                            x: content_x,
                            y: content_y,
                            color,
                            data: run.clone(),
                        },
                        children: Vec::new(),
                    },
                );
            }
        }
        if el.cursor_visible {
            if let Some(cl) = el.content_layout.as_ref() {
                let cursor_index = el
                    .edit
                    .as_ref()
                    .map(|edit| edit.cursor_byte_index)
                    .unwrap_or(0);
                let cursor = parley::Cursor::from_byte_index(
                    &cl.layout,
                    cursor_index,
                    parley::Affinity::Upstream,
                );
                let bbox = cursor.geometry(&cl.layout, 1.5_f32);
                emit(
                    sg,
                    effective_parent,
                    Node {
                        kind: NodeKind::Rect {
                            x: content_x + bbox.x0 as f32,
                            y: content_y + bbox.y0 as f32,
                            width: ((bbox.x1 - bbox.x0) as f32).max(1.5),
                            height: (bbox.y1 - bbox.y0) as f32,
                            color,
                            corner_radius: 0.0,
                        },
                        children: Vec::new(),
                    },
                );
            } else {
                emit(
                    sg,
                    effective_parent,
                    Node {
                        kind: NodeKind::Rect {
                            x: content_x,
                            y: content_y,
                            width: 1.5,
                            height: confirmed_font_size * 1.2,
                            color: confirmed_color
                                .with_opacity(visual.opacity)
                                .to_array_f32(),
                            corner_radius: 0.0,
                        },
                        children: Vec::new(),
                    },
                );
            }
        }
    } else if let Some(tl) = el.text_layout.as_ref() {
        let color = confirmed_color
            .with_opacity(visual.opacity)
            .to_array_f32();
        emit_selection_highlight(tree, id, &tl.layout, x, y, sg, effective_parent);
        for run in &tl.runs {
            emit(
                sg,
                effective_parent,
                Node {
                    kind: NodeKind::TextRun {
                        x,
                        y,
                        color,
                        data: run.clone(),
                    },
                    children: Vec::new(),
                },
            );
        }
    }

    for (child, child_reach) in children_for_reach(tree, id, reach) {
        walk_retained(
            tree,
            child,
            x,
            y,
            sg,
            lowering,
            effective_parent,
            child_inherited.clone(),
            interaction,
            child_reach,
            now_ms,
        );
    }
    reparent_child_anchors_under(
        sg,
        anchor_id,
        effective_parent,
        &tree.ordered_children(id),
        lowering,
    );
}

fn walk_ephemeral(
    tree: &ElementTree,
    id: ElementId,
    ox: f32,
    oy: f32,
    sg: &mut SceneGraph,
    parent_group: Option<NodeId>,
    inherited: InheritedVisualContext,
    interaction: &crate::element::pseudo_state::InteractionSnapshot,
) {
    let (taffy_node, el) = match tree.layout.projection.traversal_step(&tree.elements, id) {
        Some(TraversalStep::Visit(taffy_node, el)) => (taffy_node, el),
        Some(TraversalStep::Skip(_)) => {
            for child in tree.ordered_children(id) {
                walk_ephemeral(
                    tree,
                    child,
                    ox,
                    oy,
                    sg,
                    parent_group,
                    inherited.clone(),
                    interaction,
                );
            }
            return;
        }
        None => return,
    };
    let inherited_base = effective_visual::apply_text_inheritance(&inherited, &el.visual);
    let child_inherited = child_inherited_context(
        &inherited,
        el.kind,
        &inherited_base,
        &el.visual,
    );
    let own = effective_visual::own_with_viewport_variants(
        &el.visual,
        &el.viewport_variants,
        tree.viewport(),
    );
    // Full ephemeral rebuild has no retained `last_displayed`, so it never
    // interpolates — it paints the resolved target directly (ADR-0093).
    let visual = effective_visual::resolve_effective(
        &inherited,
        &own,
        &el.pseudo_styles,
        interaction,
        id,
    );
    let layout = match tree.layout.projection.taffy.layout(taffy_node) {
        Ok(l) => l,
        Err(_) => return,
    };
    let x = ox + layout.location.x;
    let y = oy + layout.location.y;
    let w = layout.size.width;
    let h = layout.size.height;

    let confirmed_color = visual.text_color.unwrap_or(Color::BLACK);
    let confirmed_font_size = visual.font_size.unwrap_or(16.0);

    let effective_parent = if let Some(transform) = el.transform {
        let group_id = emit(
            sg,
            parent_group,
            Node {
                kind: NodeKind::Group { transform },
                children: Vec::new(),
            },
        );
        Some(group_id)
    } else {
        parent_group
    };

    let effective_parent = if el.kind == ElementKind::ScrollView {
        let clip_id = emit(
            sg,
            effective_parent,
            Node {
                kind: NodeKind::Clip {
                    x,
                    y,
                    width: w,
                    height: h,
                    corner_radii: [0.0; 4],
                },
                children: Vec::new(),
            },
        );
        let (sx, sy) = el.scroll_offset;
        if sx != 0.0 || sy != 0.0 {
            let scroll_group = emit(
                sg,
                Some(clip_id),
                Node {
                    kind: NodeKind::Group {
                        transform: [1.0, 0.0, 0.0, 1.0, -sx as f64, -sy as f64],
                    },
                    children: Vec::new(),
                },
            );
            Some(scroll_group)
        } else {
            Some(clip_id)
        }
    } else if visual.overflow == OverflowValue::Hidden {
        let clip_id = emit(
            sg,
            effective_parent,
            Node {
                kind: NodeKind::Clip {
                    x,
                    y,
                    width: w,
                    height: h,
                    corner_radii: [visual.border_radius; 4],
                },
                children: Vec::new(),
            },
        );
        Some(clip_id)
    } else {
        effective_parent
    };

    if !visual.box_shadow.is_empty() {
        emit_box_shadows(
            sg,
            effective_parent,
            x,
            y,
            w,
            h,
            visual.border_radius,
            &visual.box_shadow,
            visual.opacity,
            false,
        );
    }

    emit_visual_box(
        sg,
        effective_parent,
        x,
        y,
        w,
        h,
        visual.border_radius,
        visual.border_width,
        visual.background_color,
        visual.border_color,
        visual.border_style,
        visual.opacity,
    );

    if !visual.box_shadow.is_empty() {
        emit_box_shadows(
            sg,
            effective_parent,
            x,
            y,
            w,
            h,
            visual.border_radius,
            &visual.box_shadow,
            visual.opacity,
            true,
        );
    }

    if el.kind == ElementKind::Image {
        if let Some(img) = el.src_image.clone() {
            emit(
                sg,
                effective_parent,
                Node {
                    kind: NodeKind::Image {
                        x,
                        y,
                        width: w,
                        height: h,
                        data: img,
                    },
                    children: Vec::new(),
                },
            );
        }
        for child in tree.ordered_children(id) {
            walk_ephemeral(
                tree,
                child,
                x,
                y,
                sg,
                effective_parent,
                child_inherited.clone(),
                interaction,
            );
        }
        return;
    }

    if el.kind == ElementKind::TextInput {
        let content_x = x + layout.border.left + layout.padding.left;
        let content_y = y + layout.border.top + layout.padding.top;
        let color = confirmed_color
            .with_opacity(visual.opacity)
            .to_array_f32();
        let runs = if let Some(cl) = el.content_layout.as_ref() {
            Some(cl.runs.as_slice())
        } else {
            el.text_layout.as_ref().map(|tl| tl.runs.as_slice())
        };
        if let Some(runs) = runs {
            for run in runs {
                emit(
                    sg,
                    effective_parent,
                    Node {
                        kind: NodeKind::TextRun {
                            x: content_x,
                            y: content_y,
                            color,
                            data: run.clone(),
                        },
                        children: Vec::new(),
                    },
                );
            }
        }
        if el.cursor_visible {
            if let Some(cl) = el.content_layout.as_ref() {
                let cursor_index = el
                    .edit
                    .as_ref()
                    .map(|edit| edit.cursor_byte_index)
                    .unwrap_or(0);
                let cursor = parley::Cursor::from_byte_index(
                    &cl.layout,
                    cursor_index,
                    parley::Affinity::Upstream,
                );
                let bbox = cursor.geometry(&cl.layout, 1.5_f32);
                emit(
                    sg,
                    effective_parent,
                    Node {
                        kind: NodeKind::Rect {
                            x: content_x + bbox.x0 as f32,
                            y: content_y + bbox.y0 as f32,
                            width: ((bbox.x1 - bbox.x0) as f32).max(1.5),
                            height: (bbox.y1 - bbox.y0) as f32,
                            color,
                            corner_radius: 0.0,
                        },
                        children: Vec::new(),
                    },
                );
            } else {
                emit(
                    sg,
                    effective_parent,
                    Node {
                        kind: NodeKind::Rect {
                            x: content_x,
                            y: content_y,
                            width: 1.5,
                            height: confirmed_font_size * 1.2,
                            color: confirmed_color
                                .with_opacity(visual.opacity)
                                .to_array_f32(),
                            corner_radius: 0.0,
                        },
                        children: Vec::new(),
                    },
                );
            }
        }
    } else if let Some(tl) = el.text_layout.as_ref() {
        let color = confirmed_color
            .with_opacity(visual.opacity)
            .to_array_f32();
        emit_selection_highlight(tree, id, &tl.layout, x, y, sg, effective_parent);
        for run in &tl.runs {
            emit(
                sg,
                effective_parent,
                Node {
                    kind: NodeKind::TextRun {
                        x,
                        y,
                        color,
                        data: run.clone(),
                    },
                    children: Vec::new(),
                },
            );
        }
    }

    for child in tree.ordered_children(id) {
        walk_ephemeral(
            tree,
            child,
            x,
            y,
            sg,
            effective_parent,
            child_inherited.clone(),
            interaction,
        );
    }
}

fn emit(sg: &mut SceneGraph, parent_group: Option<NodeId>, node: Node) -> NodeId {
    match parent_group {
        None => sg.insert(node),
        Some(p) => sg.insert_child(p, node),
    }
}

/// Material-flavored selection tint (ADR-0097: a single core-drawn chrome whose
/// style is theme-switchable; the value lives here as the initial theme).
const SELECTION_HIGHLIGHT_COLOR: [f32; 4] = [0.20, 0.45, 0.95, 0.35];

/// Lower the active selection's highlight for IFC root `id`, as one filled rect
/// per covered line, positioned in the element's content space (offset by the
/// text run origin `ox`, `oy`). No-op unless the document selection lies in `id`.
fn emit_selection_highlight(
    tree: &ElementTree,
    id: ElementId,
    layout: &parley::Layout<crate::element::text::TextBrush>,
    ox: f32,
    oy: f32,
    sg: &mut SceneGraph,
    parent: Option<NodeId>,
) {
    let Some((start, end)) = tree.selection_range_in_block(id) else {
        return;
    };
    for (rx, ry, rw, rh) in selection_highlight_rects(layout, start, end) {
        emit(
            sg,
            parent,
            Node {
                kind: NodeKind::Rect {
                    x: ox + rx,
                    y: oy + ry,
                    width: rw,
                    height: rh,
                    color: SELECTION_HIGHLIGHT_COLOR,
                    corner_radius: 0.0,
                },
                children: Vec::new(),
            },
        );
    }
}

/// Per-line highlight rectangles (in layout-local coordinates) covering the byte
/// range `start..end` of a Parley layout. Each line contributes the span from
/// the caret at its clamped range start to the caret at its clamped range end.
fn selection_highlight_rects(
    layout: &parley::Layout<crate::element::text::TextBrush>,
    start: usize,
    end: usize,
) -> Vec<(f32, f32, f32, f32)> {
    use parley::{Affinity, Cursor};
    let mut rects = Vec::new();
    if start >= end {
        return rects;
    }
    for line in layout.lines() {
        let line_range = line.text_range();
        let s = start.max(line_range.start);
        let e = end.min(line_range.end);
        if s >= e {
            continue;
        }
        let m = line.metrics();
        let y0 = m.block_min_coord;
        let height = m.block_max_coord - m.block_min_coord;
        let x_start = Cursor::from_byte_index(layout, s, Affinity::Downstream)
            .geometry(layout, 0.0)
            .x0 as f32;
        let x_end = Cursor::from_byte_index(layout, e, Affinity::Upstream)
            .geometry(layout, 0.0)
            .x0 as f32;
        let left = x_start.min(x_end);
        let width = (x_end - x_start).abs();
        if width > 0.0 && height > 0.0 {
            rects.push((left, y0, width, height));
        }
    }
    rects
}

fn emit_visual_box(
    sg: &mut SceneGraph,
    parent_group: Option<NodeId>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    border_radius: f32,
    border_width: f32,
    background_color: Option<Color>,
    border_color: Option<Color>,
    border_style: BorderStyleValue,
    opacity: f32,
) {
    let radius = border_radius.max(0.0);
    let border_w = border_width.max(0.0);
    let background = background_color.map(|c| c.with_opacity(opacity).to_array_f32());
    let border = border_color.map(|c| c.with_opacity(opacity).to_array_f32());

    // A border is drawn only when it has both a positive width and an explicit
    // style (CSS-like: `border-style` defaults to `none`, issue #204).
    let draw_border = border_w > 0.0 && border_style != BorderStyleValue::None;

    if draw_border {
        let Some(border_rgba) = border else {
            if let Some(bg) = background {
                emit_fill_rect(sg, parent_group, x, y, width, height, bg, radius);
            }
            return;
        };

        if border_style == BorderStyleValue::Dashed {
            // Background fills the full box; dashes stroke the perimeter on top.
            if let Some(bg) = background {
                emit_fill_rect(sg, parent_group, x, y, width, height, bg, radius);
            }
            emit(
                sg,
                parent_group,
                Node {
                    kind: NodeKind::DashedBorder {
                        x,
                        y,
                        width,
                        height,
                        outer_radius: radius,
                        border_width: border_w,
                        color: border_rgba,
                    },
                    children: Vec::new(),
                },
            );
            return;
        }

        if let Some(bg) = background {
            emit_fill_rect(
                sg,
                parent_group,
                x,
                y,
                width,
                height,
                border_rgba,
                radius,
            );
            let inner_w = (width - 2.0 * border_w).max(0.0);
            let inner_h = (height - 2.0 * border_w).max(0.0);
            if inner_w > 0.0 && inner_h > 0.0 {
                let inner_radius = (radius - border_w).max(0.0);
                emit_fill_rect(
                    sg,
                    parent_group,
                    x + border_w,
                    y + border_w,
                    inner_w,
                    inner_h,
                    bg,
                    inner_radius,
                );
            }
            return;
        }

        if radius > 0.0 {
            emit(
                sg,
                parent_group,
                Node {
                    kind: NodeKind::RoundedRing {
                        x,
                        y,
                        width,
                        height,
                        outer_radius: radius,
                        border_width: border_w,
                        color: border_rgba,
                    },
                    children: Vec::new(),
                },
            );
            return;
        }

        for (bx, by, bw2, bh2) in [
            (x, y, width, border_w),
            (x, y + height - border_w, width, border_w),
            (x, y + border_w, border_w, (height - 2.0 * border_w).max(0.0)),
            (
                x + width - border_w,
                y + border_w,
                border_w,
                (height - 2.0 * border_w).max(0.0),
            ),
        ] {
            emit_fill_rect(sg, parent_group, bx, by, bw2, bh2, border_rgba, 0.0);
        }
        return;
    }

    if let Some(bg) = background {
        emit_fill_rect(sg, parent_group, x, y, width, height, bg, radius);
    }
}

fn emit_fill_rect(
    sg: &mut SceneGraph,
    parent_group: Option<NodeId>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: [f32; 4],
    corner_radius: f32,
) {
    emit(
        sg,
        parent_group,
        Node {
            kind: NodeKind::Rect {
                x,
                y,
                width,
                height,
                color,
                corner_radius,
            },
            children: Vec::new(),
        },
    );
}

/// Number of translucent layers used to approximate a shadow's gaussian blur
/// (ADR-0095: "blur は許容範囲のガウス近似でよい"). Box-shadow is lowered to plain
/// rounded-rect fills so the Vello and tiny-skia painters render it identically
/// (semantic DOM/Canvas parity); blur ≈ overlapping translucent rounded rects.
const SHADOW_BLUR_LAYERS: usize = 6;

/// Emit the `inset == want_inset` subset of an element's box-shadow layers.
///
/// CSS paints the first-listed shadow on top, so we emit in reverse order (the
/// last-listed shadow first / bottom-most). Outset shadows are emitted behind
/// the box; inset shadows on top of the background, clipped to the border box.
#[allow(clippy::too_many_arguments)]
fn emit_box_shadows(
    sg: &mut SceneGraph,
    parent_group: Option<NodeId>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    border_radius: f32,
    shadows: &[Shadow],
    opacity: f32,
    want_inset: bool,
) {
    let radius = border_radius.max(0.0);
    for shadow in shadows.iter().rev() {
        if shadow.inset != want_inset {
            continue;
        }
        let color = shadow.color.with_opacity(opacity);
        if color.a <= 0.0 {
            continue;
        }
        if want_inset {
            emit_inset_shadow(sg, parent_group, x, y, width, height, radius, shadow, color);
        } else {
            emit_drop_shadow(sg, parent_group, x, y, width, height, radius, shadow, color);
        }
    }
}

/// Outset (drop) shadow: a rounded rect grown by `spread`, shifted by the
/// offset, and blurred by overlapping translucent rounded rects.
#[allow(clippy::too_many_arguments)]
fn emit_drop_shadow(
    sg: &mut SceneGraph,
    parent_group: Option<NodeId>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radius: f32,
    shadow: &Shadow,
    color: Color,
) {
    let sx = x + shadow.offset_x - shadow.spread;
    let sy = y + shadow.offset_y - shadow.spread;
    let sw = (width + 2.0 * shadow.spread).max(0.0);
    let sh = (height + 2.0 * shadow.spread).max(0.0);
    let sr = (radius + shadow.spread).max(0.0);
    if sw <= 0.0 || sh <= 0.0 {
        return;
    }

    let blur = shadow.blur.max(0.0);
    if blur <= 0.5 {
        emit_fill_rect(sg, parent_group, sx, sy, sw, sh, color.to_array_f32(), sr);
        return;
    }

    // Distribute the colour alpha across overlapping layers so the dense centre
    // sums to ≈ the shadow's alpha while the outer edge fades to a soft halo.
    let n = SHADOW_BLUR_LAYERS;
    let layer = Color {
        a: color.a / (n as f64 + 1.0),
        ..color
    };
    let layer_rgba = layer.to_array_f32();
    for i in (0..=n).rev() {
        let grow = blur * (i as f32) / (n as f32);
        emit_fill_rect(
            sg,
            parent_group,
            sx - grow,
            sy - grow,
            sw + 2.0 * grow,
            sh + 2.0 * grow,
            layer_rgba,
            (sr + grow).max(0.0),
        );
    }
}

/// Inset shadow: a darkened inner-edge band, layered from the border-box edge
/// inward (spread + blur thick) and clipped to the border box.
#[allow(clippy::too_many_arguments)]
fn emit_inset_shadow(
    sg: &mut SceneGraph,
    parent_group: Option<NodeId>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radius: f32,
    shadow: &Shadow,
    color: Color,
) {
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    let clip_id = emit(
        sg,
        parent_group,
        Node {
            kind: NodeKind::Clip {
                x,
                y,
                width,
                height,
                corner_radii: [radius; 4],
            },
            children: Vec::new(),
        },
    );

    let band = (shadow.spread + shadow.blur).max(0.5);
    let max_band = width.min(height) * 0.5;
    let n = SHADOW_BLUR_LAYERS;
    let layer = Color {
        a: color.a / n as f64,
        ..color
    };
    let layer_rgba = layer.to_array_f32();
    // Additive translucent edge bands, clipped to the (rounded) border box.
    // Overlapping layers darken the inner perimeter and fade toward the centre,
    // approximating an inset shadow without clearing the background (unlike a
    // ring fill). The offset only nudges the band rectangle.
    let bx = x + shadow.offset_x;
    let by = y + shadow.offset_y;
    for i in 1..=n {
        let bw = (band * (i as f32) / (n as f32)).min(max_band);
        if bw <= 0.0 {
            continue;
        }
        // top, bottom, left, right bands
        for (rx, ry, rw, rh) in [
            (bx, by, width, bw),
            (bx, by + height - bw, width, bw),
            (bx, by + bw, bw, (height - 2.0 * bw).max(0.0)),
            (bx + width - bw, by + bw, bw, (height - 2.0 * bw).max(0.0)),
        ] {
            if rw <= 0.0 || rh <= 0.0 {
                continue;
            }
            emit_fill_rect(sg, Some(clip_id), rx, ry, rw, rh, layer_rgba, 0.0);
        }
    }
}
