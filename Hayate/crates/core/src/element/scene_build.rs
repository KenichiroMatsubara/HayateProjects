use crate::color::Color;
use crate::element::effective_visual::{
    self, child_inherited_context, InheritedVisualContext,
};
use crate::element::id::ElementId;
use crate::element::kind::ElementKind;
use crate::element::inline_text::{is_ifc_root, is_inline_text_element};
use crate::element::scene_lowering::{
    clear_lowered_content, AnchorEntry, LoweringDirtySnapshot, SceneLowering,
};
use crate::element::visual_invalidation::VisualInvalidationReach;
use crate::element::taffy_projection::TraversalStep;
use crate::element::tree::ElementTree;
use crate::node::{Node, NodeId, NodeKind, SceneGraph};

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
pub(crate) fn update(
    tree: &ElementTree,
    scene_cache: &mut SceneGraph,
    lowering: &mut SceneLowering,
    dirty: LoweringDirtySnapshot,
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

    let patch_roots = minimal_patch_roots(dirty.elements.keys(), &tree.elements);
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

fn minimal_patch_roots<'a>(
    dirty: impl IntoIterator<Item = &'a ElementId>,
    elements: &std::collections::HashMap<ElementId, crate::element::tree::Element>,
) -> Vec<ElementId> {
    let dirty_set: std::collections::HashSet<ElementId> = dirty.into_iter().copied().collect();
    dirty_set
        .iter()
        .copied()
        .filter(|&id| !has_dirty_ancestor(id, &dirty_set, elements))
        .collect()
}

fn has_dirty_ancestor(
    id: ElementId,
    dirty: &std::collections::HashSet<ElementId>,
    elements: &std::collections::HashMap<ElementId, crate::element::tree::Element>,
) -> bool {
    let mut current = elements.get(&id).and_then(|el| el.parent);
    while let Some(parent) = current {
        if dirty.contains(&parent) {
            return true;
        }
        current = elements.get(&parent).and_then(|el| el.parent);
    }
    false
}

fn ensure_anchor(
    sg: &mut SceneGraph,
    lowering: &mut SceneLowering,
    id: ElementId,
    parent_anchor: Option<NodeId>,
) -> NodeId {
    if let Some(entry) = lowering.anchors.get(&id) {
        let anchor_id = entry.anchor_id;
        if let Some(parent) = parent_anchor {
            attach_under(sg, parent, anchor_id);
        }
        return anchor_id;
    }

    let anchor_id = if let Some(parent) = parent_anchor {
        sg.insert_child(
            parent,
            Node {
                kind: NodeKind::ElementAnchor { element_id: id },
                children: Vec::new(),
            },
        )
    } else {
        sg.insert(Node {
            kind: NodeKind::ElementAnchor { element_id: id },
            children: Vec::new(),
        })
    };
    lowering
        .anchors
        .insert(id, AnchorEntry { anchor_id });
    anchor_id
}

fn attach_under(sg: &mut SceneGraph, parent: NodeId, child: NodeId) {
    sg.retain_roots(|root| root != child);
    if let Some(p) = sg.get_mut(parent) {
        if !p.children.contains(&child) {
            p.children.push(child);
        }
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
) {
    lowering.walk_count += 1;

    let (taffy_node, el) = match tree.layout.projection.traversal_step(&tree.elements, id) {
        Some(TraversalStep::Visit(taffy_node, el)) => (taffy_node, el),
        Some(TraversalStep::Skip(_)) => {
            let anchor_id = ensure_anchor(sg, lowering, id, parent_anchor);
            let children = tree.ordered_children(id);
            clear_lowered_content(sg, anchor_id, &children, lowering);
            for child in children_for_reach(tree, id, reach) {
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
                    VisualInvalidationReach::Subtree,
                );
            }
            return;
        }
        None => return,
    };

    let anchor_id = ensure_anchor(sg, lowering, id, parent_anchor);
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
    );
}

fn children_for_reach(
    tree: &ElementTree,
    id: ElementId,
    reach: VisualInvalidationReach,
) -> Vec<ElementId> {
    match reach {
        VisualInvalidationReach::SelfOnly | VisualInvalidationReach::ZIndex => Vec::new(),
        VisualInvalidationReach::Subtree => tree.ordered_children(id),
        VisualInvalidationReach::TextLocal => {
            if is_ifc_root(&tree.elements, id) {
                tree.ordered_children(id)
                    .into_iter()
                    .filter(|&child| is_inline_text_element(&tree.elements, child))
                    .collect()
            } else {
                Vec::new()
            }
        }
    }
}

fn child_reach(
    tree: &ElementTree,
    parent_id: ElementId,
    child_id: ElementId,
    parent_reach: VisualInvalidationReach,
) -> VisualInvalidationReach {
    match parent_reach {
        VisualInvalidationReach::Subtree => VisualInvalidationReach::Subtree,
        VisualInvalidationReach::TextLocal
            if is_ifc_root(&tree.elements, parent_id)
                && is_inline_text_element(&tree.elements, child_id) =>
        {
            VisualInvalidationReach::TextLocal
        }
        VisualInvalidationReach::SelfOnly | VisualInvalidationReach::ZIndex => {
            VisualInvalidationReach::SelfOnly
        }
        VisualInvalidationReach::TextLocal => VisualInvalidationReach::SelfOnly,
    }
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
    } else {
        effective_parent
    };

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
        visual.opacity,
    );

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
        for child in children_for_reach(tree, id, reach) {
            walk_retained(
                tree,
                child,
                x,
                y,
                sg,
                lowering,
                Some(anchor_id),
                child_inherited.clone(),
                interaction,
                child_reach(tree, id, child, reach),
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

    for child in children_for_reach(tree, id, reach) {
        walk_retained(
            tree,
            child,
            x,
            y,
            sg,
            lowering,
            Some(anchor_id),
            child_inherited.clone(),
            interaction,
            child_reach(tree, id, child, reach),
        );
    }
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
    } else {
        effective_parent
    };

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
        visual.opacity,
    );

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
    opacity: f32,
) {
    let radius = border_radius.max(0.0);
    let border_w = border_width.max(0.0);
    let background = background_color.map(|c| c.with_opacity(opacity).to_array_f32());
    let border = border_color.map(|c| c.with_opacity(opacity).to_array_f32());

    if border_w > 0.0 {
        let Some(border_rgba) = border else {
            if let Some(bg) = background {
                emit_fill_rect(sg, parent_group, x, y, width, height, bg, radius);
            }
            return;
        };

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
