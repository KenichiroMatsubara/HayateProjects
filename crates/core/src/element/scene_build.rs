use crate::element::id::ElementId;
use crate::element::kind::ElementKind;
use crate::element::tree::ElementTree;
use crate::node::{Node, NodeId, NodeKind, SceneGraph};

pub fn build(tree: &ElementTree) -> SceneGraph {
    let mut sg = SceneGraph::new();
    if let Some(root) = tree.root() {
        walk(tree, root, 0.0, 0.0, &mut sg, None);
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
) {
    let el = match tree.elements.get(id) {
        Some(e) => e,
        None => return,
    };
    let layout = match tree.taffy.layout(el.taffy_node) {
        Ok(l) => l,
        Err(_) => return,
    };
    let x = ox + layout.location.x;
    let y = oy + layout.location.y;
    let w = layout.size.width;
    let h = layout.size.height;

    // If the element has a transform, wrap everything (including children) in a Group.
    let effective_parent = if let Some(transform) = el.transform {
        let group_id = emit(
            sg,
            parent_group,
            Node { kind: NodeKind::Group { transform }, children: Vec::new() },
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
            Node { kind: NodeKind::Clip { x, y, width: w, height: h }, children: Vec::new() },
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

    // 1) Background fill.
    if let Some(bg) = el.visual.background_color {
        emit(
            sg,
            effective_parent,
            Node {
                kind: NodeKind::Rect {
                    x,
                    y,
                    width: w,
                    height: h,
                    color: bg.with_opacity(el.visual.opacity).to_array_f32(),
                    corner_radius: el.visual.border_radius,
                },
                children: Vec::new(),
            },
        );
    }

    // 2) Border — four side rects until a dedicated BorderRect lands.
    if el.visual.border_width > 0.0 {
        if let Some(bc) = el.visual.border_color {
            let bw = el.visual.border_width;
            let color = bc.with_opacity(el.visual.opacity).to_array_f32();
            for (bx, by, bw2, bh2) in [
                (x, y, w, bw),
                (x, y + h - bw, w, bw),
                (x, y + bw, bw, (h - 2.0 * bw).max(0.0)),
                (x + w - bw, y + bw, bw, (h - 2.0 * bw).max(0.0)),
            ] {
                emit(
                    sg,
                    effective_parent,
                    Node {
                        kind: NodeKind::Rect {
                            x: bx,
                            y: by,
                            width: bw2,
                            height: bh2,
                            color,
                            corner_radius: 0.0,
                        },
                        children: Vec::new(),
                    },
                );
            }
        }
    }

    // 3a) Image content.
    if el.kind == ElementKind::Image {
        if let Some(img) = el.src_image.clone() {
            emit(
                sg,
                effective_parent,
                Node {
                    kind: NodeKind::Image { x, y, width: w, height: h, data: img },
                    children: Vec::new(),
                },
            );
        }
        // No text runs for Image elements.
        let mut children: Vec<(ElementId, i32)> = el
            .children
            .iter()
            .map(|&cid| (cid, tree.elements.get(cid).map_or(0, |c| c.visual.z_index)))
            .collect();
        children.sort_by_key(|&(_, z)| z);
        for (child, _) in children {
            walk(tree, child, x, y, sg, effective_parent);
        }
        return;
    }

    // 3b) Text runs (TextInput uses content_layout; all others use text_layout).
    if el.kind == ElementKind::TextInput {
        let color = el.visual.text_color.with_opacity(el.visual.opacity).to_array_f32();
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
                    Node { kind: NodeKind::TextRun { x, y, color, data: run.clone() }, children: Vec::new() },
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
                            height: el.visual.font_size * 1.2,
                            color: el.visual.text_color.with_opacity(el.visual.opacity).to_array_f32(),
                            corner_radius: 0.0,
                        },
                        children: Vec::new(),
                    },
                );
            }
        }
    } else if let Some(tl) = el.text_layout.as_ref() {
        let color = el.visual.text_color.with_opacity(el.visual.opacity).to_array_f32();
        for run in &tl.runs {
            emit(
                sg,
                effective_parent,
                Node { kind: NodeKind::TextRun { x, y, color, data: run.clone() }, children: Vec::new() },
            );
        }
    }

    // 4) Recurse into children, sorted by z_index (stable — preserves document order for ties).
    let mut children: Vec<(ElementId, i32)> = el
        .children
        .iter()
        .map(|&cid| (cid, tree.elements.get(cid).map_or(0, |c| c.visual.z_index)))
        .collect();
    children.sort_by_key(|&(_, z)| z);
    for (child, _) in children {
        walk(tree, child, x, y, sg, effective_parent);
    }
}

/// Insert a node either as a root (parent_group = None) or as a child.
fn emit(sg: &mut SceneGraph, parent_group: Option<NodeId>, node: Node) -> NodeId {
    match parent_group {
        None => sg.insert(node),
        Some(p) => sg.insert_child(p, node),
    }
}
