//! SceneGraph の共通契約検証。backend 固有 API に触れず、全 renderer が同じ構造エラーを
//! 観測する。debug と `scene-validation` feature でだけコンパイルされる（ADR-0148）。

use std::collections::{HashMap, HashSet};

use crate::{NodeId, SceneGraph};

/// renderer に依存しない SceneGraph 契約エラー。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SceneValidationError {
    MissingRoot { root: NodeId },
    MissingChild { parent: NodeId, child: NodeId },
    Cycle { node: NodeId },
    MultipleParents { node: NodeId },
    UnreachableNode { node: NodeId },
}

/// retained SceneGraph の木構造を検証する。root/child 参照、循環、複数親、孤立ノードを
/// 同一語彙で返すため、CanvasKit と skia-safe を含む backend は個別に解釈しない。
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
        .map_or(Ok(()), |node| Err(SceneValidationError::UnreachableNode { node }))
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
    for &child in &current.children {
        if graph.get(child).is_none() {
            return Err(SceneValidationError::MissingChild { parent: node, child });
        }
        visit(graph, child, Some(node), parents, visited, visiting)?;
    }
    visiting.remove(&node);
    visited.insert(node);
    Ok(())
}
