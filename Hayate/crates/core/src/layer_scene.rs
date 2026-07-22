use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::{
    ElementId, LayerRasterBounds, Node, NodeId, NodeKind, SceneRead, SceneResources, SceneSnapshot,
};

/// Renderer-neutral facts describing the compositing-layer tree for one committed revision.
#[derive(Debug, Clone, Default)]
pub struct LayerTopology {
    revision: u64,
    paint_order: Arc<[ElementId]>,
    parents: Arc<HashMap<ElementId, Option<ElementId>>>,
    structural_changed: Arc<HashSet<ElementId>>,
    geometry_changed: Arc<HashSet<ElementId>>,
    content_changed: Arc<HashSet<ElementId>>,
    chrome_changed: Arc<HashSet<ElementId>>,
    transform_changed: Arc<HashSet<ElementId>>,
    raster_bounds: Arc<[LayerRasterBounds]>,
    placements: Arc<[LayerPlacement]>,
    placement_nodes_visited: usize,
}

impl LayerTopology {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        revision: u64,
        paint_order: Vec<ElementId>,
        parents: HashMap<ElementId, Option<ElementId>>,
        structural_changed: HashSet<ElementId>,
        geometry_changed: HashSet<ElementId>,
        content_changed: HashSet<ElementId>,
        chrome_changed: HashSet<ElementId>,
        transform_changed: HashSet<ElementId>,
        raster_bounds: Vec<LayerRasterBounds>,
    ) -> Self {
        Self {
            revision,
            paint_order: paint_order.into(),
            parents: Arc::new(parents),
            structural_changed: Arc::new(structural_changed),
            geometry_changed: Arc::new(geometry_changed),
            content_changed: Arc::new(content_changed),
            chrome_changed: Arc::new(chrome_changed),
            transform_changed: Arc::new(transform_changed),
            raster_bounds: raster_bounds.into(),
            placements: Arc::new([]),
            placement_nodes_visited: 0,
        }
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn paint_order(&self) -> &[ElementId] {
        &self.paint_order
    }

    pub fn parent_of(&self, layer: ElementId) -> Option<ElementId> {
        self.parents.get(&layer).copied().flatten()
    }

    pub fn structural_changed(&self) -> &HashSet<ElementId> {
        &self.structural_changed
    }

    pub fn geometry_changed(&self) -> &HashSet<ElementId> {
        &self.geometry_changed
    }

    pub fn content_changed(&self) -> &HashSet<ElementId> {
        &self.content_changed
    }

    pub fn chrome_changed(&self) -> &HashSet<ElementId> {
        &self.chrome_changed
    }

    pub fn transform_changed(&self) -> &HashSet<ElementId> {
        &self.transform_changed
    }

    pub fn raster_bounds(&self) -> &[LayerRasterBounds] {
        &self.raster_bounds
    }

    pub fn placements(&self) -> &[LayerPlacement] {
        &self.placements
    }

    /// Snapshot nodes visited while refreshing placements for this commit. Clean commits report
    /// zero because they share the preceding immutable placement table.
    pub fn placement_nodes_visited(&self) -> usize {
        self.placement_nodes_visited
    }

    pub fn contains(&self, layer: ElementId) -> bool {
        self.parents.contains_key(&layer)
    }

    pub(crate) fn refresh_placements(&mut self, scene: &SceneSnapshot) {
        let Some(&root) = self.paint_order.first() else {
            self.placements = Arc::new([]);
            self.placement_nodes_visited = 0;
            return;
        };
        let mut out = vec![LayerPlacement {
            layer: root,
            transform: IDENTITY,
            clip: None,
        }];
        let mut visited = 0;
        for &node in scene.roots() {
            walk_placements(
                scene,
                self,
                node,
                root,
                IDENTITY,
                None,
                &mut out,
                &mut visited,
            );
        }
        self.placements = out.into();
        self.placement_nodes_visited = visited;
    }

    pub(crate) fn retain_placements_from(&mut self, previous: &LayerTopology) {
        self.placements = Arc::clone(&previous.placements);
        self.placement_nodes_visited = 0;
    }

    /// Preserve invalidations from a superseded frame when a raster mailbox coalesces it into
    /// a newer snapshot. Topology/order/bounds remain those of the newer frame.
    pub fn absorb_changes_from(&mut self, older: &LayerTopology) {
        Arc::make_mut(&mut self.structural_changed)
            .extend(older.structural_changed.iter().copied());
        Arc::make_mut(&mut self.geometry_changed).extend(older.geometry_changed.iter().copied());
        Arc::make_mut(&mut self.content_changed).extend(older.content_changed.iter().copied());
        Arc::make_mut(&mut self.chrome_changed).extend(older.chrome_changed.iter().copied());
        Arc::make_mut(&mut self.transform_changed).extend(older.transform_changed.iter().copied());
    }
}

pub const IDENTITY: [f64; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerPlacement {
    pub layer: ElementId,
    pub transform: [f64; 6],
    pub clip: Option<[f32; 4]>,
}

pub fn compose(outer: [f64; 6], inner: [f64; 6]) -> [f64; 6] {
    let [oa, ob, oc, od, oe, of] = outer;
    let [ia, ib, ic, id, ie, if_] = inner;
    [
        oa * ia + oc * ib,
        ob * ia + od * ib,
        oa * ic + oc * id,
        ob * ic + od * id,
        oa * ie + oc * if_ + oe,
        ob * ie + od * if_ + of,
    ]
}

fn transform_rect(t: [f64; 6], rect: [f32; 4]) -> [f32; 4] {
    let [x, y, width, height] = rect;
    let corners = [
        (x, y),
        (x + width, y),
        (x, y + height),
        (x + width, y + height),
    ];
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for (x, y) in corners {
        let dx = (t[0] * x as f64 + t[2] * y as f64 + t[4]) as f32;
        let dy = (t[1] * x as f64 + t[3] * y as f64 + t[5]) as f32;
        min_x = min_x.min(dx);
        min_y = min_y.min(dy);
        max_x = max_x.max(dx);
        max_y = max_y.max(dy);
    }
    [
        min_x,
        min_y,
        (max_x - min_x).max(0.0),
        (max_y - min_y).max(0.0),
    ]
}

fn intersect(a: Option<[f32; 4]>, b: [f32; 4]) -> [f32; 4] {
    match a {
        None => b,
        Some([ax, ay, aw, ah]) => {
            let x0 = ax.max(b[0]);
            let y0 = ay.max(b[1]);
            let x1 = (ax + aw).min(b[0] + b[2]);
            let y1 = (ay + ah).min(b[1] + b[3]);
            [x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0)]
        }
    }
}

fn own_boundary_clip(
    scene: &SceneSnapshot,
    anchor: &Node,
    placement_transform: [f64; 6],
) -> Option<[f32; 4]> {
    let parent = outer_transform_group(scene, anchor)
        .and_then(|id| scene.get(id))
        .unwrap_or(anchor);
    parent
        .children
        .iter()
        .find_map(|child| match scene.get(*child)?.kind {
            NodeKind::Clip {
                x,
                y,
                width,
                height,
                ..
            } => Some(transform_rect(placement_transform, [x, y, width, height])),
            _ => None,
        })
}

fn walk_placements(
    scene: &SceneSnapshot,
    topology: &LayerTopology,
    node_id: NodeId,
    root: ElementId,
    acc: [f64; 6],
    clip: Option<[f32; 4]>,
    out: &mut Vec<LayerPlacement>,
    visited: &mut usize,
) {
    let Some(node) = scene.get(node_id) else {
        return;
    };
    *visited += 1;
    match node.kind {
        NodeKind::Group { transform } => {
            let acc = compose(acc, transform);
            for &child in &node.children {
                walk_placements(scene, topology, child, root, acc, clip, out, visited);
            }
        }
        NodeKind::Clip {
            x,
            y,
            width,
            height,
            ..
        } => {
            let clip = Some(intersect(clip, transform_rect(acc, [x, y, width, height])));
            for &child in &node.children {
                walk_placements(scene, topology, child, root, acc, clip, out, visited);
            }
        }
        NodeKind::ElementAnchor { element_id }
            if topology.contains(element_id) && element_id != root =>
        {
            let own = outer_transform_group(scene, node)
                .and_then(|id| scene.get(id))
                .and_then(|node| match node.kind {
                    NodeKind::Group { transform } => Some(transform),
                    _ => None,
                })
                .unwrap_or(IDENTITY);
            let transform = compose(acc, own);
            let clip = own_boundary_clip(scene, node, transform)
                .map(|own| intersect(clip, own))
                .or(clip);
            out.push(LayerPlacement {
                layer: element_id,
                transform,
                clip,
            });
            for &child in &node.children {
                walk_placements(scene, topology, child, root, acc, clip, out, visited);
            }
        }
        _ => {
            for &child in &node.children {
                walk_placements(scene, topology, child, root, acc, clip, out, visited);
            }
        }
    }
}

/// Which canonical slice of a layer is rasterized into one cache plane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayerSceneKind {
    Content,
    ScrollContent { scroll_affine: [f64; 6] },
    ScrollChrome,
}

/// An owned zero-copy projection: immutable snapshot handle + layer identity + traversal roots.
/// Nodes and resources remain in the shared snapshot; this value never clones a node subtree.
#[derive(Debug, Clone)]
pub struct LayerScene {
    snapshot: SceneSnapshot,
    topology: LayerTopology,
    layer: ElementId,
    kind: LayerSceneKind,
    roots: Arc<[NodeId]>,
    excluded_layers: Arc<HashSet<ElementId>>,
}

impl LayerScene {
    pub fn new(
        snapshot: SceneSnapshot,
        topology: LayerTopology,
        layer: ElementId,
        kind: LayerSceneKind,
    ) -> Option<Self> {
        if !topology.contains(layer) {
            return None;
        }
        let root_layer = topology.paint_order().first().copied()?;
        let roots = if layer == root_layer {
            snapshot.roots().to_vec()
        } else {
            let anchor = snapshot.get(snapshot.anchor_of(layer)?)?;
            let content_root = outer_transform_group(&snapshot, anchor)
                .and_then(|id| snapshot.get(id))
                .unwrap_or(anchor);
            match kind {
                LayerSceneKind::Content => content_root.children.clone(),
                LayerSceneKind::ScrollContent { scroll_affine } => {
                    scroll_content_roots(&snapshot, content_root, scroll_affine)
                }
                LayerSceneKind::ScrollChrome => content_root
                    .children
                    .iter()
                    .copied()
                    .filter(|id| {
                        !snapshot
                            .get(*id)
                            .is_some_and(|node| matches!(node.kind, NodeKind::Clip { .. }))
                    })
                    .collect(),
            }
        };
        let excluded_layers = topology
            .paint_order()
            .iter()
            .copied()
            .filter(|candidate| *candidate != layer)
            .collect();
        Some(Self {
            snapshot,
            topology,
            layer,
            kind,
            roots: roots.into(),
            excluded_layers: Arc::new(excluded_layers),
        })
    }

    pub fn layer(&self) -> ElementId {
        self.layer
    }

    pub fn kind(&self) -> LayerSceneKind {
        self.kind
    }

    pub fn snapshot(&self) -> &SceneSnapshot {
        &self.snapshot
    }

    pub fn topology(&self) -> &LayerTopology {
        &self.topology
    }
}

impl SceneRead for LayerScene {
    fn get(&self, id: NodeId) -> Option<&Node> {
        let node = self.snapshot.get(id)?;
        if matches!(node.kind, NodeKind::ElementAnchor { element_id } if self.excluded_layers.contains(&element_id))
        {
            None
        } else {
            Some(node)
        }
    }

    fn roots(&self) -> &[NodeId] {
        &self.roots
    }

    fn resources(&self) -> &SceneResources {
        self.snapshot.resources()
    }
}

fn outer_transform_group(scene: &SceneSnapshot, anchor: &Node) -> Option<NodeId> {
    anchor.children.iter().copied().find(|child| {
        scene
            .get(*child)
            .is_some_and(|node| matches!(node.kind, NodeKind::Group { .. }))
    })
}

fn scroll_content_roots(
    scene: &SceneSnapshot,
    content_root: &Node,
    scroll_affine: [f64; 6],
) -> Vec<NodeId> {
    let Some(clip) = content_root.children.iter().find_map(|child| {
        scene
            .get(*child)
            .filter(|node| matches!(node.kind, NodeKind::Clip { .. }))
    }) else {
        return content_root.children.clone();
    };
    if scroll_affine != IDENTITY && clip.children.len() == 1 {
        if let Some(group) = scene.get(clip.children[0]).filter(
            |node| matches!(node.kind, NodeKind::Group { transform } if transform == scroll_affine),
        ) {
            return group.children.clone();
        }
    }
    clip.children.clone()
}
