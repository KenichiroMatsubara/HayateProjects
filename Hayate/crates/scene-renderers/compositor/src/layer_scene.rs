//! 保持シーンからのレイヤ分解（#633・ADR-0125 backend 半分）。
//!
//! 全面 raster を「レイヤごとの texture raster ＋ quad 合成」へ分解するための GPU 非依存な
//! 純関数群。backend（wgpu compositor / tiny-skia `draw_pixmap`）は同じ分解を消費するだけなので、
//! 分解の正しさはホストの CPU ピクセルパリティで固定できる（`tests/layer_scene_parity.rs`）。
//!
//! - [`extract_layer_scene`]: レイヤ境界要素の anchor 配下だけの sub-scene。外側 transform
//!   Group（anchor 直下の最初の Group ＝ `scene_build` の transform ラッパ規約）は**含めない**
//!   （transform は合成時の quad が適用する＝transform だけのフレームで再 raster しない前提）。
//!   子孫の別レイヤ境界 anchor はまるごと除外する（それぞれ自分の texture に raster される）。
//! - [`extract_root_scene`]: root 暗黙レイヤ＝グラフ全 roots からレイヤ境界 anchor を除いた残り
//!   （選択ツールバー等のドキュメントレベル overlay も root レイヤに属する）。
//! - [`collect_layer_placements`]: 各レイヤ quad の配置（accumulated transform ＋ 軸並行 clip）を
//!   ペイント順に集める。ネストしたレイヤは祖先レイヤの transform / scroll Group / Clip を
//!   合成した placement を持つ（フラット合成でも祖先文脈が失われない）。
//!
//! 既知の制限（v1・ADR-0125 の焼き込み系と同種）:
//! - 祖先 transform が軸保存でない（回転など）場合、clip 矩形は変換後 bbox で近似する。
//! - texture はサーフェスサイズで raster するため、transform 前の座標がビューポート外にある
//!   内容は texture に載らない（レイアウト位置が画面内にある通常の transition は影響なし）。

use std::collections::HashSet;

use hayate_core::element::id::ElementId;
use hayate_core::{Node, NodeId, NodeKind, SceneGraph, SceneRead};

/// 恒等アフィン（kurbo 係数 [a,b,c,d,e,f]）。
pub const IDENTITY: [f64; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// アフィン合成 `outer ∘ inner`（kurbo 係数。M = [a c e; b d f]）。
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

/// 1 レイヤ quad の合成配置。`transform` は祖先レイヤの transform / scroll Group を合成した
/// accumulated アフィン（texture の絶対座標内容へそのまま適用する）。`clip` は祖先および
/// レイヤ境界自身の軸並行 Clip をデバイス空間へ写した交差矩形 `[x, y, w, h]`（なければ None）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerPlacement {
    pub layer: ElementId,
    pub transform: [f64; 6],
    pub clip: Option<[f32; 4]>,
}

fn transform_rect(t: [f64; 6], rect: [f32; 4]) -> [f32; 4] {
    let [x, y, w, h] = rect;
    let corners = [(x, y), (x + w, y), (x, y + h), (x + w, y + h)];
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for (cx, cy) in corners {
        let dx = (t[0] * cx as f64 + t[2] * cy as f64 + t[4]) as f32;
        let dy = (t[1] * cx as f64 + t[3] * cy as f64 + t[5]) as f32;
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

/// anchor 直下の最初の Group 子（`scene_build` の transform ラッパ規約）。あればその
/// (NodeId, transform) を返す。
fn outer_transform_group(
    graph: &(impl SceneRead + ?Sized),
    anchor: &Node,
) -> Option<(NodeId, [f64; 6])> {
    anchor
        .children
        .iter()
        .copied()
        .find_map(|child| match graph.get(child).map(|n| &n.kind) {
            Some(&NodeKind::Group { transform }) => Some((child, transform)),
            _ => None,
        })
}

fn anchor_node_of(graph: &(impl SceneRead + ?Sized), layer: ElementId) -> Option<NodeId> {
    fn find(graph: &(impl SceneRead + ?Sized), id: NodeId, layer: ElementId) -> Option<NodeId> {
        let node = graph.get(id)?;
        if matches!(&node.kind, NodeKind::ElementAnchor { element_id } if *element_id == layer) {
            return Some(id);
        }
        node.children
            .iter()
            .find_map(|child| find(graph, *child, layer))
    }

    graph
        .roots()
        .iter()
        .find_map(|root| find(graph, *root, layer))
}

/// レイヤ境界自身が確立する overflow/scroll clip。scene lowering の規約は
/// `ElementAnchor -> [CSS transform Group] -> Clip` なので、外側 transform を剥がした
/// 直下だけを調べればよい。scroll の overscan texture からはこの Clip を剥がすため、
/// viewport 外へ動く quad を合成時に必ずこの clip で制限する。
fn own_boundary_clip(
    graph: &(impl SceneRead + ?Sized),
    anchor: &Node,
    placement_transform: [f64; 6],
) -> Option<[f32; 4]> {
    let parent = outer_transform_group(graph, anchor)
        .and_then(|(id, _)| graph.get(id))
        .unwrap_or(anchor);
    parent
        .children
        .iter()
        .find_map(|child| match graph.get(*child).map(|node| &node.kind) {
            Some(NodeKind::Clip {
                x,
                y,
                width,
                height,
                ..
            }) => Some(transform_rect(
                placement_transform,
                [*x, *y, *width, *height],
            )),
            _ => None,
        })
}

/// レイヤ quad の配置（accumulated transform / clip）をペイント順に集める。root レイヤ
/// （恒等・clip なし）が先頭。ネスト境界は祖先境界の transform / scroll Group / Clip を
/// 合成した placement を持つ。
pub fn collect_layer_placements(
    graph: &(impl SceneRead + ?Sized),
    root: ElementId,
    boundaries: &HashSet<ElementId>,
) -> Vec<LayerPlacement> {
    let mut out = vec![LayerPlacement {
        layer: root,
        transform: IDENTITY,
        clip: None,
    }];
    for &top in graph.roots() {
        walk_placements(graph, top, root, boundaries, IDENTITY, None, &mut out);
    }
    out
}

fn walk_placements(
    graph: &(impl SceneRead + ?Sized),
    node_id: NodeId,
    root: ElementId,
    boundaries: &HashSet<ElementId>,
    acc: [f64; 6],
    clip: Option<[f32; 4]>,
    out: &mut Vec<LayerPlacement>,
) {
    let Some(node) = graph.get(node_id) else {
        return;
    };
    match &node.kind {
        NodeKind::Group { transform } => {
            let acc = compose(acc, *transform);
            for &child in &node.children {
                walk_placements(graph, child, root, boundaries, acc, clip, out);
            }
        }
        NodeKind::Clip {
            x,
            y,
            width,
            height,
            ..
        } => {
            let device = transform_rect(acc, [*x, *y, *width, *height]);
            let clip = Some(intersect(clip, device));
            for &child in &node.children {
                walk_placements(graph, child, root, boundaries, acc, clip, out);
            }
        }
        NodeKind::ElementAnchor { element_id }
            if boundaries.contains(element_id) && *element_id != root =>
        {
            // 境界 anchor：外側 transform Group（あれば）を合成した placement を記録する。
            // さらに深いネスト境界を拾うため、内側の走査は継続する（Group 側の arm が
            // own transform を acc に積む）。
            let own = outer_transform_group(graph, node)
                .map(|(_, t)| t)
                .unwrap_or(IDENTITY);
            let transform = compose(acc, own);
            let clip = own_boundary_clip(graph, node, transform)
                .map(|own_clip| intersect(clip, own_clip))
                .or(clip);
            out.push(LayerPlacement {
                layer: *element_id,
                transform,
                clip,
            });
            for &child in &node.children {
                walk_placements(graph, child, root, boundaries, acc, clip, out);
            }
        }
        _ => {
            for &child in &node.children {
                walk_placements(graph, child, root, boundaries, acc, clip, out);
            }
        }
    }
}

fn copy_subtree(
    src: &(impl SceneRead + ?Sized),
    node_id: NodeId,
    dst: &mut SceneGraph,
    dst_parent: Option<NodeId>,
    exclude: &HashSet<ElementId>,
) {
    let Some(node) = src.get(node_id) else { return };
    if let NodeKind::ElementAnchor { element_id } = &node.kind {
        if exclude.contains(element_id) {
            return; // 別レイヤの内容は除外（自分の texture に raster される）
        }
    }
    let copied = Node {
        kind: node.kind.clone(),
        children: Vec::new(),
    };
    let new_id = match dst_parent {
        Some(parent) => dst.insert_child(parent, copied),
        None => dst.insert(copied),
    };
    for &child in &node.children {
        copy_subtree(src, child, dst, Some(new_id), exclude);
    }
}

/// `parent` 直下にあるレイヤ境界自身の Clip だけを剥がし、その内容を texture へコピーする。
/// scroll cache は viewport 外の overscan も保持する必要があるため、境界 Clip は
/// [`LayerPlacement::clip`] として合成時にだけ適用する。子孫側の Clip は `copy_subtree` が保持する。
fn copy_layer_contents(
    src: &(impl SceneRead + ?Sized),
    parent: &Node,
    dst: &mut SceneGraph,
    exclude: &HashSet<ElementId>,
    strip_boundary_clip: bool,
) {
    for &child in &parent.children {
        let Some(node) = src.get(child) else { continue };
        if strip_boundary_clip && matches!(&node.kind, NodeKind::Clip { .. }) {
            for &grand_child in &node.children {
                copy_subtree(src, grand_child, dst, None, exclude);
            }
        } else {
            copy_subtree(src, child, dst, None, exclude);
        }
    }
}

/// scroll boundary の viewport Clip 配下だけをコピーする。scrollbar 等の固定 chrome は
/// Clip の sibling として lowering されるため、overscan と一緒に平行移動させない。
fn copy_scroll_contents(
    src: &(impl SceneRead + ?Sized),
    parent: &Node,
    dst: &mut SceneGraph,
    exclude: &HashSet<ElementId>,
    scroll_affine: [f64; 6],
) {
    if let Some(clip) = parent.children.iter().find_map(|child| {
        src.get(*child)
            .filter(|node| matches!(&node.kind, NodeKind::Clip { .. }))
    }) {
        // A non-identity scroll affine is lowered as the Clip's sole Group child. Do not bake
        // that live transform into the cache texture: composite-only frames must be able to
        // update it (including Android stretch) without re-rastering. Identity is omitted by
        // scene lowering, so an identity Group here could belong to authored child content and
        // must not be stripped.
        if scroll_affine != IDENTITY && clip.children.len() == 1 {
            if let Some(group) = src.get(clip.children[0]).filter(|node| {
                matches!(&node.kind, NodeKind::Group { transform } if *transform == scroll_affine)
            }) {
                for &child in &group.children {
                    copy_subtree(src, child, dst, None, exclude);
                }
                return;
            }
        }
        for &child in &clip.children {
            copy_subtree(src, child, dst, None, exclude);
        }
        return;
    }

    // lowering 規約外の入力でも内容を消さない。合成 clip は無いが、従来の抽出と同じ
    // sub-scene を返す安全側の fallback。
    copy_layer_contents(src, parent, dst, exclude, false);
}

/// scroll boundary の viewport Clip 外にある固定 chrome だけをコピーする。
fn copy_scroll_chrome(
    src: &(impl SceneRead + ?Sized),
    parent: &Node,
    dst: &mut SceneGraph,
    exclude: &HashSet<ElementId>,
) {
    for &child in &parent.children {
        let Some(node) = src.get(child) else { continue };
        if !matches!(&node.kind, NodeKind::Clip { .. }) {
            copy_subtree(src, child, dst, None, exclude);
        }
    }
}

/// root 暗黙レイヤの sub-scene：グラフ全 roots（overlay 含む）からレイヤ境界 anchor を除いた残り。
pub fn extract_root_scene(
    graph: &(impl SceneRead + ?Sized),
    root: ElementId,
    boundaries: &HashSet<ElementId>,
) -> SceneGraph {
    let mut exclude = boundaries.clone();
    exclude.remove(&root); // root 自身の anchor は root レイヤの内容
    let mut out = SceneGraph::empty_projection_from(graph);
    for &top in graph.roots() {
        copy_subtree(graph, top, &mut out, None, &exclude);
    }
    out
}

fn extract_layer_scene_inner(
    graph: &(impl SceneRead + ?Sized),
    layer: ElementId,
    boundaries: &HashSet<ElementId>,
    strip_boundary_clip: bool,
) -> Option<SceneGraph> {
    let anchor_id = anchor_node_of(graph, layer)?;
    let anchor = graph.get(anchor_id)?;
    let mut exclude = boundaries.clone();
    exclude.remove(&layer);

    let mut out = SceneGraph::empty_projection_from(graph);
    // CSS transform ラッパがあればその内側を content root とする。そこからさらに direct Clip
    // （overflow / scroll viewport）だけを剥がし、overscan 内容を cache texture に残す。
    let content_root = outer_transform_group(graph, anchor)
        .and_then(|(id, _)| graph.get(id))
        .unwrap_or(anchor);
    copy_layer_contents(graph, content_root, &mut out, &exclude, strip_boundary_clip);
    Some(out)
}

/// レイヤ境界要素の anchor 配下の sub-scene（texture へ raster する内容）。外側 transform Group は
/// 含めず（quad が適用）、境界自身と子孫の Clip は保持し、子孫の別レイヤ境界は除外する。
/// anchor が未 lowering なら `None`。
pub fn extract_layer_scene(
    graph: &(impl SceneRead + ?Sized),
    layer: ElementId,
    boundaries: &HashSet<ElementId>,
) -> Option<SceneGraph> {
    extract_layer_scene_inner(graph, layer, boundaries, false)
}

/// scroll overscan cache 用の sub-scene。境界自身の viewport Clip だけを剥がして帯全体を texture に
/// 保持する。viewport Clip は [`collect_layer_placements`] が合成時に適用する。
pub fn extract_scroll_layer_scene(
    graph: &(impl SceneRead + ?Sized),
    layer: ElementId,
    boundaries: &HashSet<ElementId>,
    scroll_affine: [f64; 6],
) -> Option<SceneGraph> {
    let anchor_id = anchor_node_of(graph, layer)?;
    let anchor = graph.get(anchor_id)?;
    let mut exclude = boundaries.clone();
    exclude.remove(&layer);
    let content_root = outer_transform_group(graph, anchor)
        .and_then(|(id, _)| graph.get(id))
        .unwrap_or(anchor);
    let mut out = SceneGraph::empty_projection_from(graph);
    copy_scroll_contents(graph, content_root, &mut out, &exclude, scroll_affine);
    Some(out)
}

/// scroll layer の固定 chrome（scrollbar 等）用 sub-scene。viewport Clip の sibling だけを
/// 抽出し、overscan content texture とは別に通常 placement で合成する。
pub fn extract_scroll_chrome_scene(
    graph: &(impl SceneRead + ?Sized),
    layer: ElementId,
    boundaries: &HashSet<ElementId>,
) -> Option<SceneGraph> {
    let anchor_id = anchor_node_of(graph, layer)?;
    let anchor = graph.get(anchor_id)?;
    let mut exclude = boundaries.clone();
    exclude.remove(&layer);
    let content_root = outer_transform_group(graph, anchor)
        .and_then(|(id, _)| graph.get(id))
        .unwrap_or(anchor);
    let mut out = SceneGraph::empty_projection_from(graph);
    copy_scroll_chrome(graph, content_root, &mut out, &exclude);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compose_applies_inner_then_outer() {
        // translate(10,0) ∘ translate(0,5) = translate(10,5)
        let t = compose(
            [1.0, 0.0, 0.0, 1.0, 10.0, 0.0],
            [1.0, 0.0, 0.0, 1.0, 0.0, 5.0],
        );
        assert_eq!(t, [1.0, 0.0, 0.0, 1.0, 10.0, 5.0]);
        // scale(2) ∘ translate(3,0)：先に translate、次に scale → 実座標 +6
        let t = compose(
            [2.0, 0.0, 0.0, 2.0, 0.0, 0.0],
            [1.0, 0.0, 0.0, 1.0, 3.0, 0.0],
        );
        assert_eq!(t, [2.0, 0.0, 0.0, 2.0, 6.0, 0.0]);
    }

    #[test]
    fn clip_rects_intersect_in_device_space() {
        let device = transform_rect([1.0, 0.0, 0.0, 1.0, 10.0, 0.0], [0.0, 0.0, 100.0, 50.0]);
        assert_eq!(device, [10.0, 0.0, 100.0, 50.0]);
        let both = intersect(Some([0.0, 0.0, 60.0, 60.0]), [10.0, 0.0, 100.0, 50.0]);
        assert_eq!(both, [10.0, 0.0, 50.0, 50.0]);
    }
}
