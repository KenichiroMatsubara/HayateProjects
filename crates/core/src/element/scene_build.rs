use crate::element::id::ElementId;
use crate::element::tree::ElementTree;
use crate::node::{Node, NodeKind, SceneGraph};

pub fn build(tree: &ElementTree) -> SceneGraph {
    let mut sg = SceneGraph::new();
    if let Some(root) = tree.root() {
        walk(tree, root, 0.0, 0.0, &mut sg);
    }
    sg
}

fn walk(tree: &ElementTree, id: ElementId, ox: f32, oy: f32, sg: &mut SceneGraph) {
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

    // 1) Background fill.
    if let Some(bg) = el.visual.background_color {
        sg.insert(Node {
            kind: NodeKind::Rect {
                x,
                y,
                width: w,
                height: h,
                color: bg.with_opacity(el.visual.opacity).to_array_f32(),
                corner_radius: el.visual.border_radius,
            },
            children: Vec::new(),
        });
    }

    // 2) Border — four side rects until a dedicated BorderRect lands.
    if el.visual.border_width > 0.0 {
        if let Some(bc) = el.visual.border_color {
            let bw = el.visual.border_width;
            let color = bc.with_opacity(el.visual.opacity).to_array_f32();
            // top
            sg.insert(Node {
                kind: NodeKind::Rect { x, y, width: w, height: bw, color, corner_radius: 0.0 },
                children: Vec::new(),
            });
            // bottom
            sg.insert(Node {
                kind: NodeKind::Rect {
                    x,
                    y: y + h - bw,
                    width: w,
                    height: bw,
                    color,
                    corner_radius: 0.0,
                },
                children: Vec::new(),
            });
            // left
            sg.insert(Node {
                kind: NodeKind::Rect {
                    x,
                    y: y + bw,
                    width: bw,
                    height: (h - 2.0 * bw).max(0.0),
                    color,
                    corner_radius: 0.0,
                },
                children: Vec::new(),
            });
            // right
            sg.insert(Node {
                kind: NodeKind::Rect {
                    x: x + w - bw,
                    y: y + bw,
                    width: bw,
                    height: (h - 2.0 * bw).max(0.0),
                    color,
                    corner_radius: 0.0,
                },
                children: Vec::new(),
            });
        }
    }

    // 3) Text runs.
    if let Some(tl) = el.text_layout.as_ref() {
        let color = el
            .visual
            .text_color
            .with_opacity(el.visual.opacity)
            .to_array_f32();
        for run in &tl.runs {
            sg.insert(Node {
                kind: NodeKind::TextRun { x, y, color, data: run.clone() },
                children: Vec::new(),
            });
        }
    }

    // 4) Recurse into children.
    for &child in &el.children {
        walk(tree, child, x, y, sg);
    }
}
