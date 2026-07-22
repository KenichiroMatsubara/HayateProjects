use std::collections::HashSet;
use std::sync::Arc;

use imbl::HashMap as PersistentHashMap;
use imbl::Vector as PersistentVector;
use slotmap::{DefaultKey, Key, SlotMap};

use crate::render::{RenderFont, RenderGlyph, RenderImage};
use crate::text_resources::{
    ResourceSweepStats, SceneResources, TextResourceInterner, TextResourcePolicy, TextRunId,
};

pub type NodeId = DefaultKey;

/// painter がそのまま適用できるよう core scene lowering で解決済みのフォント合成値
/// （ADR-0054 / ADR-0085）。生の合成角・フォント単位の太らせ計算は core 内部の
/// `render::text_synthesis` が担い、painter は本構造体の値を leaf op として適用する。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TextSynthesis {
    /// 適用準備済みの faux italic スキュー tangent。`None` は直立。
    pub skew_tangent: Option<f32>,
    /// 適用準備済みの faux bold の太らせ量（フォントデザイン単位）。`None` は太らせ無し。
    pub embolden: Option<f32>,
}

/// Renderer-ready font matching attributes for selecting an instance from a variable font.
/// These travel with the shaped glyph run because normalized variation coordinates alone are
/// not consumable by every backend's public low-level glyph API.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextFontAttributes {
    pub weight: f32,
    /// Width relative to the normal face (`1.0` = normal).
    pub width: f32,
    pub slant: TextFontSlant,
}

impl Default for TextFontAttributes {
    fn default() -> Self {
        Self {
            weight: 400.0,
            width: 1.0,
            slant: TextFontSlant::Upright,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextFontSlant {
    #[default]
    Upright,
    Italic,
    Oblique,
}

#[derive(Debug, Clone)]
pub struct TextDecorationLine {
    pub x0: f32,
    pub x1: f32,
    /// run ローカル座標での中心 y（[`RenderGlyph::y`] と同じ空間）。
    pub y: f32,
    pub thickness: f32,
}

#[derive(Debug, Clone)]
/// Scene lowering が immutable text resources を intern するための入力値。
/// Scene node と committed frame はこの payload ではなく [`TextRunId`] を保持する。
pub struct TextRunData {
    pub font: RenderFont,
    pub font_size: f32,
    pub font_attributes: TextFontAttributes,
    pub glyphs: Vec<RenderGlyph>,
    pub decorations: Vec<TextDecorationLine>,
    pub text: Arc<str>,
    /// core scene lowering で解決済みの font synthesis（faux bold / italic skew）。
    pub synthesis: TextSynthesis,
    /// Parley シェーピング由来の正規化 variation 座標。
    pub normalized_coords: Vec<i16>,
}

/// 不透明 owner ボックスが drop shadow を覆う border-box 内側の角丸矩形（issue #659）。
/// 解析 painter はこの領域の影ピクセルを省く（覆われて見えないので出力不変）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadowOccluder {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub corner_radius: f32,
}

impl ShadowOccluder {
    /// シーン座標の点がこの角丸矩形の内側か（境界含む）。角の丸みに追従する。
    pub fn contains(&self, px: f32, py: f32) -> bool {
        if px < self.x || py < self.y || px > self.x + self.width || py > self.y + self.height {
            return false;
        }
        let r = self
            .corner_radius
            .min(self.width * 0.5)
            .min(self.height * 0.5);
        if r <= 0.0 {
            return true;
        }
        // 角の丸み領域だけ半径で判定。直線帯（中央）は常に内側。
        let cxl = self.x + r;
        let cxr = self.x + self.width - r;
        let cyt = self.y + r;
        let cyb = self.y + self.height - r;
        let qx = if px < cxl {
            cxl
        } else if px > cxr {
            cxr
        } else {
            return true;
        };
        let qy = if py < cyt {
            cyt
        } else if py > cyb {
            cyb
        } else {
            return true;
        };
        let dx = px - qx;
        let dy = py - qy;
        dx * dx + dy * dy <= r * r
    }
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
    /// 外側の角丸 rect と内側にインセットした角丸 rect の間を塗りつぶすリング。
    RoundedRing {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        outer_radius: f32,
        border_width: f32,
        color: [f32; 4],
    },
    /// box の外周に沿って描く破線ボーダー（`border-style: dashed`）。
    /// stroke は box 内に収まるよう `border_width / 2` だけインセットする。
    DashedBorder {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        outer_radius: f32,
        border_width: f32,
        color: [f32; 4],
    },
    /// ぼかし角丸矩形（drop shadow）の第一級プリミティブ（issue #657）。`(x, y, width,
    /// height)` は影外形（オフセット・spread 適用済み）、`corner_radius` はその角丸半径、
    /// `std_dev` はガウス σ（= blur/2）、`color` は影色（straight RGBA・不透明度適用済み）。
    /// painter は解析パス（vello `draw_blurred_rounded_rect` / tiny-skia per-pixel）または
    /// erf シェル近似の default フォールバックで描く。ぼかしなしのハードシャドウは `Rect` で表す。
    ///
    /// `occluder`（issue #659）は、この影を覆う不透明 owner ボックスの border-box 内側。設定時、
    /// 解析 painter はこの角丸矩形内のピクセルを描かない——直後に owner の不透明背景で覆われ最終
    /// ピクセルに寄与しない純粋な無駄描画だから（出力は不変）。`None` なら影全面を塗る。
    BlurredRoundedRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        corner_radius: f32,
        std_dev: f32,
        color: [f32; 4],
        occluder: Option<ShadowOccluder>,
    },
    /// inset box-shadow の解析プリミティブ（issue #660）。border-box `(x, y, width, height,
    /// corner_radius)` を塗り領域とし、影が差し込む内側 hole（`(offset_x, offset_y)` ずらし・
    /// `spread` 縮め）の外側で `color`、内側でフェードする（`color.a · (1 − blurred_coverage(hole))`）。
    /// `std_dev` はガウス σ（= blur/2）。border-box への角丸クリップは呼び出し側の `Clip` ノードが
    /// 与える。ぼかしなしのハード inset は `RoundedRing` で表す。解析パスを持たない painter の
    /// default 実装は同心 `RoundedRing` 帯（`SHADOW_BLUR_FALLBACK_LAYERS` 段）で近似する。
    InsetBlurredRoundedRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        corner_radius: f32,
        offset_x: f32,
        offset_y: f32,
        spread: f32,
        std_dev: f32,
        color: [f32; 4],
    },
    TextRun {
        x: f32,
        y: f32,
        color: [f32; 4],
        text_run: TextRunId,
    },
    /// 子にアフィン変換（kurbo 係数 [a,b,c,d,e,f]）を適用する。
    Group { transform: [f64; 6] },
    /// 子を指定の軸並行矩形にクリップする。`corner_radii`（top-left, top-right,
    /// bottom-right, bottom-left）でクリップを角丸化し、全て 0 なら矩形クリップ。
    Clip {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        corner_radii: [f32; 4],
    },
    /// 指定 rect に収まるようスケールしてラスター画像を描く。
    Image {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        data: Arc<RenderImage>,
    },
    /// draw display list を保持する retained ノード（#724 / ADR-0141）。`(x, y)` は
    /// 要素のボーダーボックス左上（絶対・論理 px）で、コマンド内のパス座標は
    /// ボーダーボックス相対のまま保持し painter が平行移動する。クリップは持たず、
    /// 祖先の `overflow`（`Clip` ラップ）に従う（既定 visible = はみ出し可）。
    DrawList {
        x: f32,
        y: f32,
        commands: Arc<Vec<crate::wire::protocol::DrawCommand>>,
    },
    /// 要素に retained シーン上の同一性を与える構造専用ノード。transform は持たず、
    /// painter は子を透過的にたどる。
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
struct SceneGraphData {
    // HAMT-backed indexes detach only paths touched by a mutation. Snapshot creation and the
    // first mutation after a commit never clone the complete retained graph.
    nodes: PersistentHashMap<NodeId, Arc<Node>>,
    order: PersistentVector<NodeId>,
    /// 描画順のトップレベルノード（親なし）。Group/Clip の子はここに載らない。
    roots: Arc<Vec<NodeId>>,
    parents: PersistentHashMap<NodeId, NodeId>,
    anchors: PersistentHashMap<crate::element::id::ElementId, NodeId>,
}

#[derive(Debug)]
pub struct SceneGraph {
    ids: SlotMap<NodeId, ()>,
    data: Arc<SceneGraphData>,
    committed_data: Arc<SceneGraphData>,
    resources: SceneResources,
    resource_interner: TextResourceInterner,
    resource_policy: TextResourcePolicy,
    retired_text_nodes_since_sweep: usize,
    revision: u64,
    pending_changed: HashSet<NodeId>,
    pending_structural: HashSet<NodeId>,
    pending_geometry: HashSet<NodeId>,
    pending_deleted: HashSet<NodeId>,
}

/// Observable facts produced by one scene commit.
#[derive(Debug, Clone, Default)]
pub struct SceneChangeJournal {
    revision: u64,
    changed_nodes: Arc<[NodeId]>,
    structural_nodes: Arc<[NodeId]>,
    geometry_nodes: Arc<[NodeId]>,
    deleted_nodes: Arc<[NodeId]>,
}

impl SceneChangeJournal {
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn changed_nodes(&self) -> &[NodeId] {
        &self.changed_nodes
    }

    pub fn structural_nodes(&self) -> &[NodeId] {
        &self.structural_nodes
    }

    pub fn geometry_nodes(&self) -> &[NodeId] {
        &self.geometry_nodes
    }

    pub fn deleted_nodes(&self) -> &[NodeId] {
        &self.deleted_nodes
    }
}

/// Work performed while freezing one immutable snapshot.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SceneCommitStats {
    changed_nodes: usize,
    storage_entries_written: usize,
}

impl SceneCommitStats {
    pub fn changed_nodes(self) -> usize {
        self.changed_nodes
    }

    pub fn storage_entries_written(self) -> usize {
        self.storage_entries_written
    }
}

/// Immutable, owned value produced by one scene commit.
#[derive(Debug, Clone)]
pub struct SceneSnapshot {
    data: Arc<SceneGraphData>,
    resources: SceneResources,
    journal: SceneChangeJournal,
    commit_stats: SceneCommitStats,
}

/// Read-only scene interface shared by mutable retained graphs, committed snapshots and layer
/// projections. Renderers only need this seam; mutation and storage lifetime stay private.
pub trait SceneRead {
    fn get(&self, id: NodeId) -> Option<&Node>;
    fn roots(&self) -> &[NodeId];
    fn resources(&self) -> &SceneResources;
}

impl<T: SceneRead + ?Sized> SceneRead for &T {
    fn get(&self, id: NodeId) -> Option<&Node> {
        (**self).get(id)
    }

    fn roots(&self) -> &[NodeId] {
        (**self).roots()
    }

    fn resources(&self) -> &SceneResources {
        (**self).resources()
    }
}

impl SceneSnapshot {
    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.data.nodes.get(&id).map(Arc::as_ref)
    }

    pub fn resources(&self) -> &SceneResources {
        &self.resources
    }

    pub fn roots(&self) -> &[NodeId] {
        &self.data.roots
    }

    pub fn iter(&self) -> impl Iterator<Item = (NodeId, &Node)> {
        self.data
            .order
            .iter()
            .filter_map(|id| self.data.nodes.get(id).map(|node| (*id, node.as_ref())))
    }

    pub fn parent_of(&self, id: NodeId) -> Option<NodeId> {
        self.data.parents.get(&id).copied()
    }

    pub fn anchor_of(&self, element: crate::element::id::ElementId) -> Option<NodeId> {
        self.data.anchors.get(&element).copied()
    }

    pub fn len(&self) -> usize {
        self.data.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.nodes.is_empty()
    }

    pub fn changes(&self) -> &SceneChangeJournal {
        &self.journal
    }

    pub fn commit_stats(&self) -> SceneCommitStats {
        self.commit_stats
    }
}

impl SceneRead for SceneSnapshot {
    fn get(&self, id: NodeId) -> Option<&Node> {
        self.get(id)
    }

    fn roots(&self) -> &[NodeId] {
        self.roots()
    }

    fn resources(&self) -> &SceneResources {
        self.resources()
    }
}

impl SceneGraph {
    pub fn new() -> Self {
        Self::with_text_resource_policy(TextResourcePolicy::default())
    }

    pub fn with_text_resource_policy(resource_policy: TextResourcePolicy) -> Self {
        let data = Arc::new(SceneGraphData {
            nodes: PersistentHashMap::new(),
            order: PersistentVector::new(),
            roots: Arc::new(Vec::new()),
            parents: PersistentHashMap::new(),
            anchors: PersistentHashMap::new(),
        });
        Self {
            ids: SlotMap::with_key(),
            data: Arc::clone(&data),
            committed_data: data,
            resources: SceneResources::default(),
            resource_interner: TextResourceInterner::new(),
            resource_policy,
            retired_text_nodes_since_sweep: 0,
            revision: 0,
            pending_changed: HashSet::new(),
            pending_structural: HashSet::new(),
            pending_geometry: HashSet::new(),
            pending_deleted: HashSet::new(),
        }
    }

    /// Start an empty structural projection that shares this snapshot's immutable resources.
    /// Layer projection code can copy only node IDs while renderer lookups remain valid.
    pub fn empty_projection(&self) -> Self {
        let mut projection = Self::empty_projection_from(self);
        projection.resource_interner = self.resource_interner.clone();
        projection.resource_policy = self.resource_policy;
        projection
    }

    /// Start an empty projection whose text/image handles refer to the same immutable resources
    /// as `source`. The projection owns only its newly inserted nodes.
    pub fn empty_projection_from(source: &(impl SceneRead + ?Sized)) -> Self {
        let data = Arc::new(SceneGraphData {
            nodes: PersistentHashMap::new(),
            order: PersistentVector::new(),
            roots: Arc::new(Vec::new()),
            parents: PersistentHashMap::new(),
            anchors: PersistentHashMap::new(),
        });
        Self {
            ids: SlotMap::with_key(),
            data: Arc::clone(&data),
            committed_data: data,
            resources: source.resources().clone(),
            resource_interner: TextResourceInterner::new(),
            resource_policy: TextResourcePolicy::default(),
            retired_text_nodes_since_sweep: 0,
            revision: 0,
            pending_changed: HashSet::new(),
            pending_structural: HashSet::new(),
            pending_geometry: HashSet::new(),
            pending_deleted: HashSet::new(),
        }
    }

    /// Intern one immutable shaped run and pin it to this scene snapshot.
    pub fn intern_text_run(&mut self, data: TextRunData) -> TextRunId {
        let interned = self.resource_interner.intern_text_run(data);
        let id = interned.id;
        self.resources.pin(interned);
        id
    }

    pub fn resources(&self) -> &SceneResources {
        &self.resources
    }

    /// Freeze the current retained scene into an owned immutable value. Persistent indexes make
    /// the freeze O(changes), while the returned snapshot itself is an O(1) shared handle.
    pub fn snapshot(&mut self) -> SceneSnapshot {
        self.refresh_indexes();
        self.revision = self.revision.wrapping_add(1);
        let changed_nodes = sorted_nodes(&self.pending_changed);
        let journal = SceneChangeJournal {
            revision: self.revision,
            changed_nodes: changed_nodes.clone().into(),
            structural_nodes: sorted_nodes(&self.pending_structural).into(),
            geometry_nodes: sorted_nodes(&self.pending_geometry).into(),
            deleted_nodes: sorted_nodes(&self.pending_deleted).into(),
        };
        let commit_stats = SceneCommitStats {
            changed_nodes: changed_nodes.len(),
            storage_entries_written: changed_nodes.len(),
        };
        self.pending_changed.clear();
        self.pending_structural.clear();
        self.pending_geometry.clear();
        self.pending_deleted.clear();
        self.committed_data = Arc::clone(&self.data);
        SceneSnapshot {
            data: Arc::clone(&self.data),
            resources: self.resources.clone(),
            journal,
            commit_stats,
        }
    }

    /// Release resource pins no longer referenced by this scene, then reclaim entries that no
    /// concurrently alive scene snapshot still owns.
    pub fn sweep_resources(&mut self) -> ResourceSweepStats {
        let live: HashSet<TextRunId> = self
            .data
            .nodes
            .values()
            .filter_map(|node| match node.kind {
                NodeKind::TextRun { text_run, .. } => Some(text_run),
                _ => None,
            })
            .collect();
        self.resources.retain_text_runs(&live);
        self.retired_text_nodes_since_sweep = 0;
        self.resource_interner.sweep()
    }

    /// Apply the typed sweep threshold at a completed scene-mutation transaction.
    pub fn maintain_resources(&mut self) -> ResourceSweepStats {
        if self.retired_text_nodes_since_sweep >= self.resource_policy.sweep_threshold() {
            self.sweep_resources()
        } else {
            ResourceSweepStats::default()
        }
    }

    /// トップレベル（root）ノードを挿入する。
    pub fn insert(&mut self, node: Node) -> NodeId {
        let id = self.ids.insert(());
        let data = Arc::make_mut(&mut self.data);
        if let NodeKind::ElementAnchor { element_id } = node.kind {
            data.anchors.insert(element_id, id);
        }
        data.nodes.insert(id, Arc::new(node));
        data.order.push_back(id);
        Arc::make_mut(&mut data.roots).push(id);
        self.pending_changed.insert(id);
        self.pending_structural.insert(id);
        self.pending_geometry.insert(id);
        id
    }

    /// 既存ノードの子としてノードを挿入する。
    pub fn insert_child(&mut self, parent: NodeId, node: Node) -> NodeId {
        let id = self.ids.insert(());
        let data = Arc::make_mut(&mut self.data);
        if let NodeKind::ElementAnchor { element_id } = node.kind {
            data.anchors.insert(element_id, id);
        }
        data.nodes.insert(id, Arc::new(node));
        data.order.push_back(id);
        if let Some(parent_node) = data.nodes.get_mut(&parent) {
            Arc::make_mut(parent_node).children.push(id);
            data.parents.insert(id, parent);
            self.pending_changed.insert(parent);
            self.pending_structural.insert(parent);
        }
        self.pending_changed.insert(id);
        self.pending_structural.insert(id);
        self.pending_geometry.insert(id);
        id
    }

    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.data.nodes.get(&id).map(Arc::as_ref)
    }

    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        if !self.data.nodes.contains_key(&id) {
            return None;
        }
        self.pending_changed.insert(id);
        self.pending_geometry.insert(id);
        Arc::make_mut(&mut self.data)
            .nodes
            .get_mut(&id)
            .map(Arc::make_mut)
    }

    /// Mutate one retained child list while keeping the persistent parent index coherent.
    pub fn edit_children<R>(
        &mut self,
        parent: NodeId,
        edit: impl FnOnce(&mut Vec<NodeId>) -> R,
    ) -> Option<R> {
        let old_children = self.data.nodes.get(&parent)?.children.clone();
        let (result, new_children) = {
            let data = Arc::make_mut(&mut self.data);
            let node = data.nodes.get_mut(&parent)?;
            let node = Arc::make_mut(node);
            let result = edit(&mut node.children);
            (result, node.children.clone())
        };
        let data = Arc::make_mut(&mut self.data);
        for child in old_children
            .iter()
            .filter(|child| !new_children.contains(child))
        {
            if data.parents.get(child) == Some(&parent) {
                data.parents.remove(child);
            }
        }
        for child in new_children
            .iter()
            .filter(|child| !old_children.contains(child))
        {
            data.parents.insert(*child, parent);
        }
        self.pending_changed.insert(parent);
        self.pending_structural.insert(parent);
        Some(result)
    }

    pub fn retain_roots(&mut self, mut keep: impl FnMut(NodeId) -> bool) {
        let removed: Vec<NodeId> = self
            .data
            .roots
            .iter()
            .copied()
            .filter(|id| !keep(*id))
            .collect();
        let data = Arc::make_mut(&mut self.data);
        Arc::make_mut(&mut data.roots).retain(|id| !removed.contains(id));
        for id in removed {
            self.pending_changed.insert(id);
            self.pending_structural.insert(id);
        }
    }

    pub fn remove(&mut self, id: NodeId) -> Option<Node> {
        let parent = self.parent_of(id);
        let node = Arc::make_mut(&mut self.data).nodes.remove(&id)?;
        let _ = self.ids.remove(id);
        if let Some(parent) = parent {
            self.edit_children(parent, |children| children.retain(|child| *child != id));
        }
        let data = Arc::make_mut(&mut self.data);
        Arc::make_mut(&mut data.roots).retain(|root| *root != id);
        data.parents.remove(&id);
        for child in &node.children {
            if data.parents.get(child) == Some(&id) {
                data.parents.remove(child);
            }
        }
        if let NodeKind::ElementAnchor { element_id } = node.kind {
            if data.anchors.get(&element_id) == Some(&id) {
                data.anchors.remove(&element_id);
            }
        }
        self.pending_changed.insert(id);
        self.pending_structural.insert(id);
        self.pending_deleted.insert(id);
        let node = Arc::unwrap_or_clone(node);
        if matches!(node.kind, NodeKind::TextRun { .. }) {
            self.retired_text_nodes_since_sweep += 1;
        }
        Some(node)
    }

    /// Group / Clip / ElementAnchor 配下にネストしているときの `id` の親。
    pub fn parent_of(&self, id: NodeId) -> Option<NodeId> {
        self.data.parents.get(&id).copied()
    }

    /// Resolve an element's retained anchor without a whole-graph scan.
    pub fn anchor_of(&self, element: crate::element::id::ElementId) -> Option<NodeId> {
        self.data.anchors.get(&element).copied()
    }

    /// `id` とその全子孫をグラフから削除する。
    pub fn remove_subtree(&mut self, id: NodeId) {
        let children = self
            .data
            .nodes
            .get(&id)
            .map(|n| n.children.clone())
            .unwrap_or_default();
        for child in children {
            self.remove_subtree(child);
        }
        let _ = self.remove(id);
    }

    /// 最初の root ノード（後方互換）。
    pub fn root(&self) -> Option<NodeId> {
        self.data.roots.first().copied()
    }

    /// 描画順の全トップレベルノード。
    pub fn roots(&self) -> &[NodeId] {
        &self.data.roots
    }

    pub fn iter(&self) -> impl Iterator<Item = (NodeId, &Node)> {
        self.data
            .order
            .iter()
            .filter_map(|id| self.data.nodes.get(id).map(|node| (*id, node.as_ref())))
    }

    pub fn len(&self) -> usize {
        self.data.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.nodes.is_empty()
    }

    fn refresh_indexes(&mut self) {
        let changed: Vec<NodeId> = self.pending_changed.iter().copied().collect();
        for id in changed {
            let old_children = self
                .committed_data
                .nodes
                .get(&id)
                .map(|node| node.children.clone())
                .unwrap_or_default();
            let new_children = self
                .data
                .nodes
                .get(&id)
                .map(|node| node.children.clone())
                .unwrap_or_default();
            if old_children != new_children {
                self.pending_structural.insert(id);
            }
            let data = Arc::make_mut(&mut self.data);
            for child in old_children
                .iter()
                .filter(|child| !new_children.contains(child))
            {
                if data.parents.get(child) == Some(&id) {
                    data.parents.remove(child);
                }
            }
            for child in new_children
                .iter()
                .filter(|child| !old_children.contains(child))
            {
                data.parents.insert(*child, id);
            }
            let old_anchor = self
                .committed_data
                .nodes
                .get(&id)
                .and_then(|node| match node.kind {
                    NodeKind::ElementAnchor { element_id } => Some(element_id),
                    _ => None,
                });
            let new_anchor = data.nodes.get(&id).and_then(|node| match node.kind {
                NodeKind::ElementAnchor { element_id } => Some(element_id),
                _ => None,
            });
            if old_anchor != new_anchor {
                if let Some(element) = old_anchor {
                    if data.anchors.get(&element) == Some(&id) {
                        data.anchors.remove(&element);
                    }
                }
                if let Some(element) = new_anchor {
                    data.anchors.insert(element, id);
                }
            }
        }
    }
}

fn sorted_nodes(nodes: &HashSet<NodeId>) -> Vec<NodeId> {
    let mut nodes: Vec<_> = nodes.iter().copied().collect();
    nodes.sort_by_key(|id| id.data().as_ffi());
    nodes
}

impl Default for SceneGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl SceneRead for SceneGraph {
    fn get(&self, id: NodeId) -> Option<&Node> {
        self.get(id)
    }

    fn roots(&self) -> &[NodeId] {
        self.roots()
    }

    fn resources(&self) -> &SceneResources {
        self.resources()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group() -> Node {
        Node {
            kind: NodeKind::Group {
                transform: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            },
            children: Vec::new(),
        }
    }

    #[test]
    fn snapshot_reuses_unchanged_nodes_and_detaches_only_the_mutated_node() {
        let mut ui_scene = SceneGraph::new();
        let unchanged = ui_scene.insert(group());
        let changed = ui_scene.insert(group());
        let committed = ui_scene.snapshot();

        assert!(
            std::ptr::eq(ui_scene.get(unchanged).unwrap(), committed.get(unchanged).unwrap()),
            "committing a scene must structurally share unchanged nodes instead of deep-cloning them"
        );
        assert!(
            std::ptr::eq(
                ui_scene.get(changed).unwrap(),
                committed.get(changed).unwrap()
            ),
            "the initial committed snapshot must share every retained node"
        );

        ui_scene.get_mut(changed).unwrap().children.push(unchanged);

        assert!(
            std::ptr::eq(
                ui_scene.get(unchanged).unwrap(),
                committed.get(unchanged).unwrap()
            ),
            "updating the next frame must keep unrelated retained nodes shared"
        );
        assert!(
            !std::ptr::eq(
                ui_scene.get(changed).unwrap(),
                committed.get(changed).unwrap()
            ),
            "the updated node must detach from the immutable committed snapshot"
        );
        assert!(committed.get(changed).unwrap().children.is_empty());
        assert_eq!(ui_scene.get(changed).unwrap().children, vec![unchanged]);
    }
}
