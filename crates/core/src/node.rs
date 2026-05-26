use std::sync::Arc;

use slotmap::{DefaultKey, SlotMap};
use vello::Glyph;
use vello::peniko::{FontData, ImageData};

pub type NodeId = DefaultKey;

#[derive(Debug, Clone)]
pub struct TextRunData {
    pub font: FontData,
    pub font_size: f32,
    pub glyphs: Vec<Glyph>,
    pub text: Arc<str>,
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
    TextRun {
        x: f32,
        y: f32,
        color: [f32; 4],
        data: Arc<TextRunData>,
    },
    /// Applies an affine transform (kurbo coefficients [a,b,c,d,e,f]) to its children.
    Group {
        transform: [f64; 6],
    },
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
        data: Arc<ImageData>,
    },
}

#[derive(Debug, Clone)]
pub struct Node {
    pub kind: NodeKind,
    pub children: Vec<NodeId>,
}

pub struct SceneGraph {
    nodes: SlotMap<NodeId, Node>,
    /// Top-level nodes in paint order (no parent). Children of Group/Clip are not listed here.
    roots: Vec<NodeId>,
}

impl SceneGraph {
    pub fn new() -> Self {
        Self { nodes: SlotMap::new(), roots: Vec::new() }
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

    pub fn remove(&mut self, id: NodeId) -> Option<Node> {
        self.nodes.remove(id)
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
