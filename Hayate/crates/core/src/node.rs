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
    // Retained nodes are structurally shared by committed snapshots. When the UI detaches the
    // scene index for its next frame, these Arc handles keep unchanged nodes shared.
    nodes: SlotMap<NodeId, Arc<Node>>,
    /// 描画順のトップレベルノード（親なし）。Group/Clip の子はここに載らない。
    roots: Vec<NodeId>,
}

#[derive(Debug, Clone)]
pub struct SceneGraph {
    // A handoff clone is O(1). The first subsequent UI mutation detaches this small index, while
    // individual retained nodes remain shared until that specific node changes.
    data: Arc<SceneGraphData>,
}

impl SceneGraph {
    pub fn new() -> Self {
        Self {
            data: Arc::new(SceneGraphData {
                nodes: SlotMap::new(),
                roots: Vec::new(),
            }),
        }
    }

    /// トップレベル（root）ノードを挿入する。
    pub fn insert(&mut self, node: Node) -> NodeId {
        let data = Arc::make_mut(&mut self.data);
        let id = data.nodes.insert(Arc::new(node));
        data.roots.push(id);
        id
    }

    /// 既存ノードの子としてノードを挿入する。
    pub fn insert_child(&mut self, parent: NodeId, node: Node) -> NodeId {
        let data = Arc::make_mut(&mut self.data);
        let id = data.nodes.insert(Arc::new(node));
        if let Some(p) = data.nodes.get_mut(parent) {
            Arc::make_mut(p).children.push(id);
        }
        id
    }

    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.data.nodes.get(id).map(Arc::as_ref)
    }

    pub fn get_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        Arc::make_mut(&mut self.data)
            .nodes
            .get_mut(id)
            .map(Arc::make_mut)
    }

    pub fn retain_roots(&mut self, mut keep: impl FnMut(NodeId) -> bool) {
        Arc::make_mut(&mut self.data).roots.retain(|&id| keep(id));
    }

    pub fn remove(&mut self, id: NodeId) -> Option<Node> {
        let node = {
            let data = Arc::make_mut(&mut self.data);
            let node = data.nodes.remove(id)?;
            data.roots.retain(|&root| root != id);
            node
        };
        if let Some(parent) = self.parent_of(id) {
            if let Some(p) = Arc::make_mut(&mut self.data).nodes.get_mut(parent) {
                Arc::make_mut(p).children.retain(|&child| child != id);
            }
        }
        Some(Arc::unwrap_or_clone(node))
    }

    /// Group / Clip / ElementAnchor 配下にネストしているときの `id` の親。
    pub fn parent_of(&self, id: NodeId) -> Option<NodeId> {
        for (parent_id, parent) in self.data.nodes.iter() {
            if parent.children.contains(&id) {
                return Some(parent_id);
            }
        }
        None
    }

    /// `id` とその全子孫をグラフから削除する。
    pub fn remove_subtree(&mut self, id: NodeId) {
        let children = self
            .data
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
        self.data.roots.first().copied()
    }

    /// 描画順の全トップレベルノード。
    pub fn roots(&self) -> &[NodeId] {
        &self.data.roots
    }

    pub fn iter(&self) -> impl Iterator<Item = (NodeId, &Node)> {
        self.data.nodes.iter().map(|(id, node)| (id, node.as_ref()))
    }

    pub fn len(&self) -> usize {
        self.data.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.nodes.is_empty()
    }
}

impl Default for SceneGraph {
    fn default() -> Self {
        Self::new()
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
    fn cloned_scene_reuses_unchanged_nodes_and_detaches_only_the_mutated_node() {
        let mut ui_scene = SceneGraph::new();
        let unchanged = ui_scene.insert(group());
        let changed = ui_scene.insert(group());
        let committed = ui_scene.clone();

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
