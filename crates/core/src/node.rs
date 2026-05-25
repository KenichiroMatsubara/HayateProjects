use std::sync::Arc;

use slotmap::{DefaultKey, SlotMap};
use vello::Glyph;
use vello::peniko::FontData;

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
}

#[derive(Debug, Clone)]
pub struct Node {
    pub kind: NodeKind,
    pub children: Vec<NodeId>,
}

pub struct SceneGraph {
    nodes: SlotMap<NodeId, Node>,
    root: Option<NodeId>,
}

impl SceneGraph {
    pub fn new() -> Self {
        Self {
            nodes: SlotMap::new(),
            root: None,
        }
    }

    pub fn insert(&mut self, node: Node) -> NodeId {
        let id = self.nodes.insert(node);
        if self.root.is_none() {
            self.root = Some(id);
        }
        id
    }

    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id)
    }

    pub fn remove(&mut self, id: NodeId) -> Option<Node> {
        self.nodes.remove(id)
    }

    pub fn root(&self) -> Option<NodeId> {
        self.root
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
