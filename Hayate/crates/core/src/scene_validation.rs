//! SceneGraph の共通契約検証。backend 固有 API に触れず、全 renderer が同じ構造エラーを
//! 観測する。debug と `scene-validation` feature でだけコンパイルされる（ADR-0148）。

use std::collections::{HashMap, HashSet};

use crate::{DrawCommand, DrawPaint, NodeId, NodeKind, PathVerb, SceneGraph};

/// renderer に依存しない SceneGraph 契約エラー。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SceneValidationError {
    MissingRoot { root: NodeId },
    MissingChild { parent: NodeId, child: NodeId },
    Cycle { node: NodeId },
    MultipleParents { node: NodeId },
    UnreachableNode { node: NodeId },
    InvalidCommand { node: NodeId },
}

/// One validation pass' observable amount of contract work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SceneValidationReport {
    visited_nodes: usize,
}

impl SceneValidationReport {
    pub fn visited_nodes(self) -> usize {
        self.visited_nodes
    }
}

/// Retained validation state. The first pass validates the complete graph; later passes accept
/// the retained subtree roots changed by lowering. An empty change set is a zero-work pass.
#[derive(Debug, Default)]
pub struct SceneGraphValidator {
    initialized: bool,
}

impl SceneGraphValidator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn validate(
        &mut self,
        graph: &SceneGraph,
        changed_roots: impl IntoIterator<Item = NodeId>,
    ) -> Result<SceneValidationReport, SceneValidationError> {
        if !self.initialized {
            validate_scene_graph(graph)?;
            self.initialized = true;
            return Ok(SceneValidationReport {
                visited_nodes: graph.len(),
            });
        }

        let candidates: HashSet<NodeId> = changed_roots.into_iter().collect();
        if candidates.is_empty() {
            return Ok(SceneValidationReport { visited_nodes: 0 });
        }
        let roots: Vec<NodeId> = candidates
            .iter()
            .copied()
            .filter(|&candidate| {
                let mut current = graph.parent_of(candidate);
                while let Some(parent) = current {
                    if candidates.contains(&parent) {
                        return false;
                    }
                    current = graph.parent_of(parent);
                }
                true
            })
            .collect();

        let mut parents = HashMap::new();
        let mut visited = HashSet::new();
        let mut visiting = HashSet::new();
        for root in roots {
            if graph.get(root).is_none() {
                return Err(SceneValidationError::MissingRoot { root });
            }
            visit(
                graph,
                root,
                graph.parent_of(root),
                &mut parents,
                &mut visited,
                &mut visiting,
            )?;
        }
        Ok(SceneValidationReport {
            visited_nodes: visited.len(),
        })
    }
}

/// retained SceneGraph の木構造を検証する。root/child 参照、循環、複数親、孤立ノードを
/// 同一語彙で返すため、skia-safe を含む backend は個別に解釈しない。
pub fn validate_scene_graph(graph: &SceneGraph) -> Result<(), SceneValidationError> {
    let mut parents = HashMap::new();
    let mut visited = HashSet::new();
    let mut visiting = HashSet::new();

    for &root in graph.roots() {
        if graph.get(root).is_none() {
            return Err(SceneValidationError::MissingRoot { root });
        }
        visit(graph, root, None, &mut parents, &mut visited, &mut visiting)?;
    }

    graph
        .iter()
        .map(|(id, _)| id)
        .find(|id| !visited.contains(id))
        .map_or(Ok(()), |node| {
            Err(SceneValidationError::UnreachableNode { node })
        })
}

fn visit(
    graph: &SceneGraph,
    node: NodeId,
    parent: Option<NodeId>,
    parents: &mut HashMap<NodeId, Option<NodeId>>,
    visited: &mut HashSet<NodeId>,
    visiting: &mut HashSet<NodeId>,
) -> Result<(), SceneValidationError> {
    if !visiting.insert(node) {
        return Err(SceneValidationError::Cycle { node });
    }
    if parents.insert(node, parent).is_some() {
        return Err(SceneValidationError::MultipleParents { node });
    }
    let current = graph.get(node).expect("caller verifies node exists");
    if !valid_node_kind(graph, &current.kind) {
        return Err(SceneValidationError::InvalidCommand { node });
    }
    for &child in &current.children {
        if graph.get(child).is_none() {
            return Err(SceneValidationError::MissingChild {
                parent: node,
                child,
            });
        }
        visit(graph, child, Some(node), parents, visited, visiting)?;
    }
    visiting.remove(&node);
    visited.insert(node);
    Ok(())
}

fn valid_node_kind(graph: &SceneGraph, kind: &NodeKind) -> bool {
    match kind {
        NodeKind::Rect {
            x,
            y,
            width,
            height,
            color,
            corner_radius,
        } => {
            finite([*x, *y, *width, *height, *corner_radius])
                && *width >= 0.0
                && *height >= 0.0
                && *corner_radius >= 0.0
                && valid_color(color)
        }
        NodeKind::RoundedRing {
            x,
            y,
            width,
            height,
            outer_radius,
            border_width,
            color,
        }
        | NodeKind::DashedBorder {
            x,
            y,
            width,
            height,
            outer_radius,
            border_width,
            color,
        } => {
            finite([*x, *y, *width, *height, *outer_radius, *border_width])
                && *width >= 0.0
                && *height >= 0.0
                && *outer_radius >= 0.0
                && *border_width >= 0.0
                && valid_color(color)
        }
        NodeKind::BlurredRoundedRect {
            x,
            y,
            width,
            height,
            corner_radius,
            std_dev,
            color,
            occluder,
        } => {
            finite([*x, *y, *width, *height, *corner_radius, *std_dev])
                && *width >= 0.0
                && *height >= 0.0
                && *corner_radius >= 0.0
                && *std_dev >= 0.0
                && valid_color(color)
                && occluder.is_none_or(|value| {
                    finite([
                        value.x,
                        value.y,
                        value.width,
                        value.height,
                        value.corner_radius,
                    ]) && value.width >= 0.0
                        && value.height >= 0.0
                        && value.corner_radius >= 0.0
                })
        }
        NodeKind::InsetBlurredRoundedRect {
            x,
            y,
            width,
            height,
            corner_radius,
            offset_x,
            offset_y,
            spread,
            std_dev,
            color,
        } => {
            finite([
                *x,
                *y,
                *width,
                *height,
                *corner_radius,
                *offset_x,
                *offset_y,
                *spread,
                *std_dev,
            ]) && *width >= 0.0
                && *height >= 0.0
                && *corner_radius >= 0.0
                && *std_dev >= 0.0
                && valid_color(color)
        }
        NodeKind::TextRun {
            x,
            y,
            color,
            text_run,
        } => {
            let Ok(data) = graph.resources().text_run(*text_run) else {
                return false;
            };
            finite([*x, *y, data.font_size])
                && data.font_size >= 0.0
                && valid_color(color)
                && data.glyphs.iter().all(|glyph| finite([glyph.x, glyph.y]))
                && data.decorations.iter().all(|line| {
                    finite([line.x0, line.x1, line.y, line.thickness]) && line.thickness >= 0.0
                })
        }
        NodeKind::Group { transform } => transform.iter().all(|value| value.is_finite()),
        NodeKind::Clip {
            x,
            y,
            width,
            height,
            corner_radii,
        } => {
            finite([*x, *y, *width, *height])
                && *width >= 0.0
                && *height >= 0.0
                && corner_radii
                    .iter()
                    .all(|value| value.is_finite() && *value >= 0.0)
        }
        NodeKind::Image {
            x,
            y,
            width,
            height,
            ..
        } => finite([*x, *y, *width, *height]) && *width >= 0.0 && *height >= 0.0,
        NodeKind::DrawList { x, y, commands } => {
            finite([*x, *y]) && commands.iter().all(valid_draw_command)
        }
        NodeKind::ElementAnchor { .. } => true,
    }
}

fn valid_draw_command(command: &DrawCommand) -> bool {
    match command {
        DrawCommand::FillPath { verbs, paint } | DrawCommand::StrokePath { verbs, paint } => {
            verbs.iter().all(valid_path_verb) && valid_paint(paint)
        }
        DrawCommand::Save | DrawCommand::Restore => true,
        DrawCommand::Translate { dx, dy } => finite([*dx, *dy]),
        DrawCommand::Rotate { radians } => radians.is_finite(),
        DrawCommand::Scale { sx, sy } => finite([*sx, *sy]),
        DrawCommand::Transform { a, b, c, d, e, f } => finite([*a, *b, *c, *d, *e, *f]),
        DrawCommand::ClipRect {
            x,
            y,
            width,
            height,
        } => finite([*x, *y, *width, *height]) && *width >= 0.0 && *height >= 0.0,
        DrawCommand::ClipPath { verbs } => verbs.iter().all(valid_path_verb),
    }
}

fn valid_paint(paint: &DrawPaint) -> bool {
    valid_color(&paint.color)
        && finite([
            paint.fill_rule,
            paint.stroke_width,
            paint.cap,
            paint.join,
            paint.miter_limit,
            paint.dash_offset,
        ])
        && paint.stroke_width >= 0.0
        && paint.miter_limit >= 0.0
        && paint
            .dash
            .iter()
            .all(|value| value.is_finite() && *value >= 0.0)
}

fn valid_path_verb(verb: &PathVerb) -> bool {
    match verb {
        PathVerb::MoveTo { x, y } | PathVerb::LineTo { x, y } => finite([*x, *y]),
        PathVerb::Close => true,
        PathVerb::QuadraticTo { cx, cy, x, y } => finite([*cx, *cy, *x, *y]),
        PathVerb::CubicTo {
            c1x,
            c1y,
            c2x,
            c2y,
            x,
            y,
        } => finite([*c1x, *c1y, *c2x, *c2y, *x, *y]),
        PathVerb::ArcTo {
            x1,
            y1,
            x2,
            y2,
            radius,
        } => finite([*x1, *y1, *x2, *y2, *radius]) && *radius >= 0.0,
        PathVerb::Rect {
            x,
            y,
            width,
            height,
        }
        | PathVerb::Oval {
            x,
            y,
            width,
            height,
        } => finite([*x, *y, *width, *height]) && *width >= 0.0 && *height >= 0.0,
        PathVerb::Rrect {
            x,
            y,
            width,
            height,
            rx,
            ry,
        } => {
            finite([*x, *y, *width, *height, *rx, *ry])
                && *width >= 0.0
                && *height >= 0.0
                && *rx >= 0.0
                && *ry >= 0.0
        }
        PathVerb::Circle { cx, cy, radius } => finite([*cx, *cy, *radius]) && *radius >= 0.0,
    }
}

fn valid_color(color: &[f32; 4]) -> bool {
    color
        .iter()
        .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
}

fn finite<const N: usize>(values: [f32; N]) -> bool {
    values.into_iter().all(f32::is_finite)
}
