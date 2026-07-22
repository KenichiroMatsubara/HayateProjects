use std::collections::{HashMap, HashSet};

use crate::element::id::ElementId;
use crate::element::transition::ElementTransitions;
use crate::element::tree::{ElementTree, Visual};
use crate::element::visual_invalidation::{self, VisualInvalidationReach};
use crate::node::{NodeId, SceneGraph};

#[derive(Debug, Clone)]
pub(crate) struct AnchorEntry {
    pub anchor_id: NodeId,
    /// 前フレームの表示済み（ブレンド後）visual。新規トランジションの `from` 元となる（ADR-0093）。
    /// 真実の源ではなく要素ライフタイムに紐づく派生キャッシュ（ADR-0057）。初回 emit では `None` で、
    /// 初期スタイルはトランジションしない。
    pub last_displayed: Option<Visual>,
    /// この要素の進行中トランジション（プロパティ単位）。
    pub transitions: ElementTransitions,
    /// この要素が compositing layer の境界か（ADR-0125）。`load_compositing_layers` が live state
    /// から導出して焼き付ける派生キャッシュで、真実の源は要素の trigger（transition/transform/
    /// scroll）。バックエンドは本 slice では参照しない（layers があっても出力同一）。
    pub is_compositing_layer: bool,
}

impl AnchorEntry {
    pub fn new(anchor_id: NodeId) -> Self {
        Self {
            anchor_id,
            last_displayed: None,
            transitions: ElementTransitions::default(),
            is_compositing_layer: false,
        }
    }

    /// `resolved`（変更後の実効 visual）を表示済み値と補間し、`now_ms` まで進めて、
    /// 結果を次フレームの `from` としてメモ化する。今フレームに描画する visual を返す。
    pub fn resolve_displayed(&mut self, resolved: &Visual, now_ms: f64) -> Visual {
        let displayed = self
            .transitions
            .blend(self.last_displayed.as_ref(), resolved, now_ms);
        self.last_displayed = Some(displayed.clone());
        displayed
    }

    /// `resolve_displayed` の読み取り専用版。保持中のトランジション状態を進めずに、
    /// `now_ms` 時点の表示済み visual を補間する。描画パスと同じ `blend` を進行中トラックの
    /// 使い捨てクローンに対して実行するため、メモ化済みの `last_displayed` とプロパティ単位の
    /// クロックには触れない。クエリと描画フレームループは独立に保たれる（ADR-0093）。
    pub fn sample_displayed(&self, resolved: &Visual, now_ms: f64) -> Visual {
        self.transitions
            .clone()
            .blend(self.last_displayed.as_ref(), resolved, now_ms)
    }
}

/// 要素→シーンの lowering 状態（保持）。
#[derive(Debug, Default)]
pub(crate) struct SceneLowering {
    pub anchors: std::collections::HashMap<ElementId, AnchorEntry>,
    pub built: bool,
    pub walk_count: usize,
    /// 現在描画中なら、フローティング選択ツールバーオーバーレイのルートノード（ADR-0097）。
    /// ツールバーはどの要素にもアンカーされないドキュメントレベルの chrome なので、
    /// 毎フレーム最上位オーバーレイとして再 emit し、次の emit 前にこの id で除去する。
    pub toolbar_root: Option<NodeId>,
    /// 現在描画中なら、選択ドラッグハンドルオーバーレイのルートノード（ADR-0097）。
    /// ツールバーと同様、毎フレーム最上位オーバーレイとして再 emit され、次の emit 前にこの id で除去される。
    pub handles_root: Option<NodeId>,
}

impl SceneLowering {
    pub fn reset(&mut self) {
        self.anchors.clear();
        self.built = false;
        self.walk_count = 0;
        self.toolbar_root = None;
        self.handles_root = None;
    }

    /// 進行中トランジションを持つ要素。`render` が visual-dirty に保ち続けることで、
    /// 収束するまでフレームループが進め続ける。
    pub fn active_transition_ids(&self) -> Vec<ElementId> {
        self.anchors
            .iter()
            .filter(|(_, entry)| entry.transitions.is_active())
            .map(|(&id, _)| id)
            .collect()
    }
}

/// 今フレームのシーン再 lowering 対象としてスケジュールされた dirty 要素。
#[derive(Debug, Default)]
pub(crate) struct LoweringDirtySnapshot {
    pub elements: HashMap<ElementId, VisualInvalidationReach>,
    pub z_index_reorder_parents: HashSet<ElementId>,
    pub fonts: bool,
    pub full_rebuild: bool,
}

pub(crate) fn collect_lowering_dirty(
    tree: &ElementTree,
    structure_dirty: &HashSet<ElementId>,
    shape_dirty: &HashSet<ElementId>,
    shape_lowering_reach: &HashMap<ElementId, VisualInvalidationReach>,
    visual_dirty: &HashMap<ElementId, VisualInvalidationReach>,
    fonts_dirty: bool,
) -> LoweringDirtySnapshot {
    let mut snapshot = LoweringDirtySnapshot::default();
    if fonts_dirty {
        snapshot.full_rebuild = true;
        return snapshot;
    }

    for (&id, &reach) in visual_dirty {
        visual_invalidation::apply_visual_invalidation(
            tree,
            id,
            reach,
            &mut snapshot.elements,
            &mut snapshot.z_index_reorder_parents,
        );
    }
    for &id in structure_dirty {
        visual_invalidation::expand_subtree(tree, id, &mut snapshot.elements);
    }
    for &id in shape_dirty {
        let reach = shape_lowering_reach
            .get(&id)
            .copied()
            .unwrap_or(VisualInvalidationReach::Subtree);
        visual_invalidation::apply_visual_invalidation(
            tree,
            id,
            reach,
            &mut snapshot.elements,
            &mut snapshot.z_index_reorder_parents,
        );
    }
    snapshot
}

pub(crate) fn clear_lowered_content(
    sg: &mut SceneGraph,
    anchor_id: NodeId,
    element_children: &[ElementId],
    lowering: &SceneLowering,
) {
    let preserve: HashSet<NodeId> = element_children
        .iter()
        .filter_map(|child| lowering.anchors.get(child).map(|e| e.anchor_id))
        .collect();

    // 子アンカーは前パスの一時的な Clip/Group ラッパーの下にいることがある。
    // ラッパー破棄で remove_subtree されないよう `anchor_id` 直下へ引き上げる。
    for &child_anchor in &preserve {
        if let Some(parent) = sg.parent_of(child_anchor) {
            if parent != anchor_id {
                detach_child(sg, child_anchor);
                attach_child(sg, anchor_id, child_anchor);
            }
        }
    }

    let to_remove: Vec<NodeId> = sg
        .get(anchor_id)
        .map(|anchor| {
            anchor
                .children
                .iter()
                .copied()
                .filter(|id| !preserve.contains(id))
                .collect()
        })
        .unwrap_or_default();
    for id in to_remove {
        sg.remove_subtree(id);
    }
    sg.edit_children(anchor_id, |children| {
        children.retain(|id| preserve.contains(id))
    });
}

fn detach_child(sg: &mut SceneGraph, child: NodeId) {
    if let Some(parent) = sg.parent_of(child) {
        sg.edit_children(parent, |children| children.retain(|&id| id != child));
    }
}

fn attach_child(sg: &mut SceneGraph, parent: NodeId, child: NodeId) {
    sg.edit_children(parent, |children| {
        if !children.contains(&child) {
            children.push(child);
        }
    });
}
