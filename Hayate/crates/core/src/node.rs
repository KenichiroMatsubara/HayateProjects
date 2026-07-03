use std::sync::Arc;

use slotmap::{DefaultKey, SlotMap};

use crate::render::{RenderFont, RenderGlyph, RenderImage};

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

#[derive(Debug, Clone)]
pub struct TextDecorationLine {
    pub x0: f32,
    pub x1: f32,
    /// run ローカル座標での中心 y（[`RenderGlyph::y`] と同じ空間）。
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
    /// core scene lowering で解決済みの font synthesis（faux bold / italic skew）。
    pub synthesis: TextSynthesis,
    /// Parley シェーピング由来の正規化 variation 座標。
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
    BlurredRoundedRect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        corner_radius: f32,
        std_dev: f32,
        color: [f32; 4],
    },
    TextRun {
        x: f32,
        y: f32,
        color: [f32; 4],
        data: Arc<TextRunData>,
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
pub struct SceneGraph {
    nodes: SlotMap<NodeId, Node>,
    /// 描画順のトップレベルノード（親なし）。Group/Clip の子はここに載らない。
    roots: Vec<NodeId>,
}

impl SceneGraph {
    pub fn new() -> Self {
        Self {
            nodes: SlotMap::new(),
            roots: Vec::new(),
        }
    }

    /// トップレベル（root）ノードを挿入する。
    pub fn insert(&mut self, node: Node) -> NodeId {
        let id = self.nodes.insert(node);
        self.roots.push(id);
        id
    }

    /// 既存ノードの子としてノードを挿入する。
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

    /// Group / Clip / ElementAnchor 配下にネストしているときの `id` の親。
    pub fn parent_of(&self, id: NodeId) -> Option<NodeId> {
        for (parent_id, parent) in self.nodes.iter() {
            if parent.children.contains(&id) {
                return Some(parent_id);
            }
        }
        None
    }

    /// `id` とその全子孫をグラフから削除する。
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

    /// 最初の root ノード（後方互換）。
    pub fn root(&self) -> Option<NodeId> {
        self.roots.first().copied()
    }

    /// 描画順の全トップレベルノード。
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
