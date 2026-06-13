use std::sync::Arc;

use fontique::Synthesis;
use slotmap::{DefaultKey, SlotMap};

use crate::render::{RenderFont, RenderGlyph, RenderImage};

pub type NodeId = DefaultKey;

#[derive(Debug, Clone)]
pub struct TextDecorationLine {
    pub x0: f32,
    pub x1: f32,
    /// Center y in run-local coordinates (same space as [`RenderGlyph::y`]).
    pub y: f32,
    pub thickness: f32,
}

#[derive(Debug, Clone)]
pub struct TextRunData {
    pub font: RenderFont,
    pub font_size: f32,
    pub glyphs: Vec<RenderGlyph>,
    pub decorations: Vec<TextDecorationLine>,
    pub text: Arc<str>,
    /// Font synthesis from fontique (faux bold / italic skew / variation axes).
    pub synthesis: Synthesis,
    /// Normalized variation coordinates from Parley shaping.
    pub normalized_coords: Vec<i16>,
}

#[derive(Debug, Clone)]
pub enum NodeKind {
    Rect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: [f32; 4],
        corner_radius: f32,
    },
    /// Filled ring between an outer rounded rect and an inset inner rounded rect.
    RoundedRing {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        outer_radius: f32,
        border_width: f32,
        color: [f32; 4],
    },
    /// Dashed border stroked along the box perimeter (`border-style: dashed`).
    /// The stroke is inset by `border_width / 2` so it stays inside the box.
    DashedBorder {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        outer_radius: f32,
        border_width: f32,
        color: [f32; 4],
    },
    TextRun {
        x: f32,
        y: f32,
        color: [f32; 4],
        data: Arc<TextRunData>,
    },
    /// Applies an affine transform (kurbo coefficients [a,b,c,d,e,f]) to its children.
    Group { transform: [f64; 6] },
    /// Clips its children to the given axis-aligned rectangle.
    Clip {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },
    /// Draws a raster image scaled to fit the given rect.
    Image {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        data: Arc<RenderImage>,
    },
    /// Structural-only node giving an element retained scene identity (issue #182).
    /// Carries no transform; painters walk its children transparently.
    ElementAnchor {
        element_id: crate::element::id::ElementId,
    },
}

#[derive(Debug, Clone)]
pub struct Node {
    pub kind: NodeKind,
    pub children: Vec<NodeId>,
}

#[derive(Debug, Clone)]
pub struct SceneGraph {
    nodes: SlotMap<NodeId, Node>,
    /// Top-level nodes in paint order (no parent). Children of Group/Clip are not listed here.
    roots: Vec<NodeId>,
}

impl SceneGraph {
    pub fn new() -> Self {
        Self {
            nodes: SlotMap::new(),
            roots: Vec::new(),
        }
    }

    /// Insert a top-level (root) node.
    pub fn insert(&mut self, node: Node) -> NodeId {
        let id = self.nodes.insert(node);
        self.roots.push(id);
        id
    }

    /// Insert a node as a child of an existing node.
    pub fn insert_child(&mut self, parent: NodeId, node: Node) -> NodeId {
        let id = self.nodes.insert(node);
        if let Some(p) = self.nodes.get_mut(parent) {
            p.children.push(id);
        }
        id
    }

    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id)
    }

    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.nodes.get_mut(id)
    }

    pub fn retain_roots(&mut self, mut keep: impl FnMut(NodeId) -> bool) {
        self.roots.retain(|&id| keep(id));
    }

    pub fn remove(&mut self, id: NodeId) -> Option<Node> {
        let node = self.nodes.remove(id)?;
        self.roots.retain(|&root| root != id);
        if let Some(parent) = self.parent_of(id) {
            if let Some(p) = self.nodes.get_mut(parent) {
                p.children.retain(|&child| child != id);
            }
        }
        Some(node)
    }

    /// Parent of `id` when nested under a Group, Clip, or ElementAnchor.
    pub fn parent_of(&self, id: NodeId) -> Option<NodeId> {
        for (parent_id, parent) in self.nodes.iter() {
            if parent.children.contains(&id) {
                return Some(parent_id);
            }
        }
        None
    }

    /// Remove `id` and every descendant from the graph.
    pub fn remove_subtree(&mut self, id: NodeId) {
        let children = self
            .nodes
            .get(id)
            .map(|n| n.children.clone())
            .unwrap_or_default();
        for child in children {
            self.remove_subtree(child);
        }
        let _ = self.remove(id);
    }

    /// First root node (backward compat).
    pub fn root(&self) -> Option<NodeId> {
        self.roots.first().copied()
    }

    /// All top-level nodes in paint order.
    pub fn roots(&self) -> &[NodeId] {
        &self.roots
    }

    pub fn iter(&self) -> impl Iterator<Item = (NodeId, &Node)> {
        self.nodes.iter()
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

impl Default for SceneGraph {
    fn default() -> Self {
        Self::new()
    }
}
