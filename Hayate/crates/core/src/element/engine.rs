use std::collections::{HashMap, HashSet};

use crate::element::id::ElementId;
use crate::element::visual_invalidation::{
    self, VisualInvalidationReach,
};
use crate::element::layout_pass::LayoutPass;
use crate::element::tree::{Element, Event};

/// `ElementTree::commit_frame()` を駆動する dirty 追跡集合
/// （`structure_dirty` / `shape_dirty` / `fonts_dirty`）を保持する（ADR-0075）。
///
/// dirty マーキングの*ポリシー*（どの変更が何を dirty にするか）は `tree.rs` の
/// `element_set_*` 側に残す。`ElementEngine` は dirty 集合の保持と解決のみを担う。
pub(crate) struct ElementEngine {
    pub(crate) structure_dirty: HashSet<ElementId>,
    /// レイアウト前に Parley 再コンポーズが必要な IFC ルート（ADR-0063）。
    pub(crate) shape_dirty: HashSet<ElementId>,
    /// `shape_dirty` シードに対するシーン再 lowering の到達範囲。
    pub(crate) shape_lowering_reach: HashMap<ElementId, VisualInvalidationReach>,
    /// シーンのみの視覚変更。各 `render()` 後に drain される。
    pub(crate) visual_dirty: HashMap<ElementId, VisualInvalidationReach>,
    /// 直近の `resolve()` レイアウトパスで絶対ボックス幾何 `(x, y, w, h)` が変化
    /// （または出現）した要素。layout→lowering のギャップを橋渡しし、祖先や兄弟へ
    /// 波及した flex リフローが（古くなった）retained ボックスを再 lowering できる
    /// ようにする。`resolve` で埋め、`commit_frame()` 後の `render` で drain する。
    pub(crate) layout_geometry_dirty: HashSet<ElementId>,
    /// `register_font` でセットし、次の `resolve` 冒頭でクリアする。
    /// 新規登録フォントで全テキスト要素を再シェイプさせる。
    pub(crate) fonts_dirty: bool,
    /// transform 係数だけが変わった要素（Some→Some、#633）。レイヤ内容は不変なので visual dirty
    /// （re-lower）には流さず、`render()` が保持シーンの Group ノードだけを patch する。present 側
    /// composite-only フレーム（quad transform 更新のみで raster ゼロ）の core 前提。
    pub(crate) transform_dirty: HashSet<ElementId>,
}

impl ElementEngine {
    pub fn new() -> Self {
        Self {
            structure_dirty: HashSet::new(),
            shape_dirty: HashSet::new(),
            shape_lowering_reach: HashMap::new(),
            visual_dirty: HashMap::new(),
            layout_geometry_dirty: HashSet::new(),
            fonts_dirty: false,
            transform_dirty: HashSet::new(),
        }
    }

    pub fn mark_transform_dirty(&mut self, id: ElementId) {
        self.transform_dirty.insert(id);
    }

    pub fn drain_transform_dirty(&mut self) -> HashSet<ElementId> {
        std::mem::take(&mut self.transform_dirty)
    }

    pub fn mark_structure_dirty(&mut self, id: ElementId) {
        self.structure_dirty.insert(id);
    }

    pub fn mark_shape_dirty(&mut self, id: ElementId, reach: VisualInvalidationReach) {
        self.shape_dirty.insert(id);
        visual_invalidation::merge_reach(&mut self.shape_lowering_reach, id, reach);
    }

    pub fn mark_visual_dirty(&mut self, id: ElementId, reach: VisualInvalidationReach) {
        visual_invalidation::merge_reach(&mut self.visual_dirty, id, reach);
    }

    pub fn mark_fonts_dirty(&mut self) {
        self.fonts_dirty = true;
    }

    pub fn drain_visual_dirty(&mut self) -> HashMap<ElementId, VisualInvalidationReach> {
        std::mem::take(&mut self.visual_dirty)
    }

    pub fn drain_layout_geometry_dirty(&mut self) -> HashSet<ElementId> {
        std::mem::take(&mut self.layout_geometry_dirty)
    }

    pub fn drain_shape_lowering_reach(&mut self) -> HashMap<ElementId, VisualInvalidationReach> {
        std::mem::take(&mut self.shape_lowering_reach)
    }

    /// dirty 状態を解決しレイアウトを確定する。Taffy 投影の reconcile + Parley
    /// シェイピング + レイアウトキャッシュ更新（`LayoutPass::run()` 相当、ADR-0075）。
    pub fn resolve(
        &mut self,
        layout: &mut LayoutPass,
        elements: &mut HashMap<ElementId, Element>,
        root: ElementId,
        viewport: (f32, f32),
        event_queue: &mut Vec<Event>,
    ) {
        // 集約レイアウトインターフェース。単一の `settle` が
        // reconcile → compute → cache → geometry diff を畳み込む。返る diff
        // （移動・リサイズ・出現したボックス）を `layout_geometry_dirty` に畳み込み、
        // `render` が古い retained ボックスを再 lowering できるようにする。祖先や
        // 兄弟へ波及する flex リフローでは、移動した各 id が独立してここに入る。
        let geometry_dirty = layout.settle(
            elements,
            root,
            viewport,
            event_queue,
            &mut self.structure_dirty,
            &mut self.shape_dirty,
            &mut self.fonts_dirty,
        );
        self.layout_geometry_dirty.extend(geometry_dirty);
    }
}

impl Default for ElementEngine {
    fn default() -> Self {
        Self::new()
    }
}
