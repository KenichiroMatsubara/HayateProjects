use crate::color::Color;
use crate::element::effective_visual::{
    self, child_inherited_context, InheritedVisualContext,
};
use crate::element::id::ElementId;
use crate::element::kind::ElementKind;
use crate::element::tree::ElementTree;
use crate::node::{Node, NodeId, NodeKind, SceneGraph};

pub fn build(tree: &ElementTree) -> SceneGraph {
    let mut sg = SceneGraph::new();
    let interaction = tree.interaction_snapshot();
    if let Some(root) = tree.root() {
        walk(
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

/// Emit SceneGraph nodes for `id` and its subtree.
///
/// `parent_group` — when Some, newly created nodes are inserted as children of that
/// Group/Clip node instead of as top-level roots. This lets transforms and clip regions
/// wrap a whole subtree without changing layout-computed coordinates.
fn walk(
    tree: &ElementTree,
    id: ElementId,
    ox: f32,
    oy: f32,
    sg: &mut SceneGraph,
    parent_group: Option<NodeId>,
    inherited: InheritedVisualContext,
    interaction: &crate::element::pseudo_state::InteractionSnapshot,
) {
    let el = match tree.elements.get(&id) {
        Some(e) => e,
        None => return,
    };
    // Inline text elements have no Taffy box (ADR-0063/0064); recurse without emitting.
    let taffy_node = match el.taffy_node {
        Some(n) => n,
        None => {
            for child in tree.ordered_children(id) {
                walk(
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
    };
    let inherited_base = effective_visual::apply_text_inheritance(&inherited, &el.visual);
    let child_inherited = child_inherited_context(
        &inherited,
        el.kind,
        &inherited_base,
        &el.visual,
    );
    let visual = effective_visual::resolve_effective(
        &inherited,
        &el.visual,
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

    // If the element has a transform, wrap everything (including children) in a Group.
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

    // ScrollView: clip to its bounds, then apply a translate for the scroll offset.
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
            // Negative translate shifts content up/left so positive offset scrolls down/right.
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

    // 1–2) Background and border fills (effective visual includes pseudo states).
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

    // 3a) Image content.
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
        // No text runs for Image elements.
        for child in tree.ordered_children(id) {
            walk(
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

    // 3b) Text runs (TextInput uses content_layout; all others use text_layout).
    if el.kind == ElementKind::TextInput {
        let color = confirmed_color
            .with_opacity(visual.opacity)
            .to_array_f32();
        // content_layout covers committed text + active preedit; fall back to
        // placeholder (text_layout) only when neither is present.
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
        // Cursor rect — only in Canvas mode (HTML mode uses the native <input> cursor).
        if el.cursor_visible {
            if let Some(cl) = el.content_layout.as_ref() {
                let cursor = parley::Cursor::from_byte_index(
                    &cl.layout,
                    el.cursor_byte_index,
                    parley::Affinity::Upstream,
                );
                let bbox = cursor.geometry(&cl.layout, 1.5_f32);
                emit(
                    sg,
                    effective_parent,
                    Node {
                        kind: NodeKind::Rect {
                            x: x + bbox.x0 as f32,
                            y: y + bbox.y0 as f32,
                            width: ((bbox.x1 - bbox.x0) as f32).max(1.5),
                            height: (bbox.y1 - bbox.y0) as f32,
                            color,
                            corner_radius: 0.0,
                        },
                        children: Vec::new(),
                    },
                );
            } else {
                // Empty text_content: draw cursor at element origin.
                emit(
                    sg,
                    effective_parent,
                    Node {
                        kind: NodeKind::Rect {
                            x,
                            y,
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

    // 4) Recurse into children in paint order (Z-Order の単一正本: ADR-0060).
    for child in tree.ordered_children(id) {
        walk(
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

/// Insert a node either as a root (parent_group = None) or as a child.
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
