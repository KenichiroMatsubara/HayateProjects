use std::collections::{HashMap, HashSet};

use crate::element::id::ElementId;
use crate::element::inline_text::{is_ifc_root, is_inline_text_element};
use crate::element::kind::ElementKind;
use crate::element::style::StyleProp;
use crate::element::tree::{Element, ElementTree};

/// 要素1つのトポロジ的文脈。`ElementTree` を起動せずに `classify` /
/// `step_reach` が必要とする位置情報を持つ。`tree.rs` がライブトポロジから
/// 構築し、ここの無効化セマンティクスは `(prop, ctx)` / `(reach, ctx)` 上の
/// 純粋関数に保つ。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ElementContext {
    pub kind: ElementKind,
    /// インライン整形コンテキスト(IFC)の根となる `text` 要素（ADR-0063）。
    pub is_ifc_root: bool,
    /// 親が `text` 要素である。
    pub has_text_parent: bool,
}

impl ElementContext {
    /// 別の `text` の直下にネストした `text`。Taffy ボックスを持たない。
    pub(crate) fn is_inline_text(self) -> bool {
        self.kind == ElementKind::Text && self.has_text_parent
    }
}

/// 素の `elements` マップから [`ElementContext`] を構築する。
/// `ElementTree::element_context` と同じトポロジ読み取りを、要素マップしか
/// 持たない呼び出し側（例: ライブな `ElementTree` を持たない Taffy 投影の
/// reconcile）に提供する。reach カーネルは得られた文脈上で純粋に保つ。
pub(crate) fn element_context_in(
    elements: &HashMap<ElementId, Element>,
    id: ElementId,
) -> ElementContext {
    let el = elements.get(&id);
    let kind = el.map_or(ElementKind::View, |e| e.kind);
    let has_text_parent = el
        .and_then(|e| e.parent)
        .and_then(|p| elements.get(&p))
        .is_some_and(|p| p.kind == ElementKind::Text);
    ElementContext {
        kind,
        is_ifc_root: is_ifc_root(elements, id),
        has_text_parent,
    }
}

/// 変更がどの dirty セットに流れるか。`merge` が最も広い関心を残すよう順序付け。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum DirtyKind {
    /// シーンのみの視覚変更 → `visual_dirty`。
    Visual,
    /// IFC 再構成（テキストシェーピング）→ `shape_dirty` + 投影マーク。
    Shape,
    /// ツリー構造の変化 → `structure_dirty`。
    Structure,
}

impl DirtyKind {
    fn merge(self, other: Self) -> Self {
        self.max(other)
    }
}

/// 単一の変更が無効化にとって何を意味するか: どの dirty セットへ、どこまで届くか。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Change {
    pub dirty_kind: DirtyKind,
    pub reach: VisualInvalidationReach,
}

impl Change {
    pub(crate) fn merge(self, other: Self) -> Self {
        Change {
            dirty_kind: self.dirty_kind.merge(other.dirty_kind),
            reach: self.reach.merge(other.reach),
        }
    }

    /// 分類可能な視覚プロパティを持たないスタイル変更（例: 空のプロパティ列）
    /// に対するフォールバック: シーンのみの自要素再描画。
    pub(crate) fn visual_self_only() -> Self {
        Change {
            dirty_kind: DirtyKind::Visual,
            reach: VisualInvalidationReach::SelfOnly,
        }
    }
}

/// `prop` の変更が IFC のシェープ済みテキストを無効化するか。
/// `tree::apply_visual` の `text_dirty` ケース（フォントメトリクス、Parley の
/// run brush に焼き込まれる色、行詰めの上限）に対応する。
pub(crate) fn prop_affects_text_shaping(prop: &StyleProp) -> bool {
    matches!(
        prop,
        StyleProp::MaxLines(_)
            | StyleProp::TextOverflow(_)
            | StyleProp::FontSize(_)
            | StyleProp::FontFamily(_)
            | StyleProp::FontWeight(_)
            | StyleProp::Color(_)
            | StyleProp::FontStyle(_)
            | StyleProp::TextDecoration(_)
    )
}

/// ルーティングされた `Change` が到達しうる dirty セット群。ライブの
/// `ElementEngine` + `TaffyProjection` 組が `tree.rs` で実装する。テストでは
/// 記録用フェイクにより `Change → マークされたセット` 表を `ElementTree` を
/// 起動せず検証できる（ADR-0099）。
pub(crate) trait DirtySink {
    fn mark_visual(&mut self, id: ElementId, reach: VisualInvalidationReach);
    fn mark_shape(&mut self, id: ElementId, reach: VisualInvalidationReach);
    fn mark_structure(&mut self, id: ElementId);
    fn mark_geometry(&mut self, id: ElementId);
}

/// 視覚無効化ルーティングの唯一の接合点（ADR-0099）: 分類済みの `Change` を、
/// 到達すべき全 dirty セットへアトミックに配送する。対応表全体をここに集約し、
/// 呼び出し側が `engine.mark_*` / `projection.mark_dirty` の組を手配線しないため、
/// shape 変更が投影ジオメトリをマークせずエンジンへ届くことはない。
///
/// *どの要素か*（例: shape 変更で囲う IFC 根を解決する）はトポロジであり呼び出し
/// 側に残る。ここは `dirty_kind → sinks` の対応のみを担う。
pub(crate) fn route_change<S: DirtySink>(sink: &mut S, id: ElementId, change: Change) {
    match change.dirty_kind {
        DirtyKind::Visual => sink.mark_visual(id, change.reach),
        DirtyKind::Shape => {
            sink.mark_shape(id, change.reach);
            sink.mark_geometry(id);
        }
        DirtyKind::Structure => sink.mark_structure(id),
    }
}

/// スタイルプロパティ変更を要素文脈に対して分類する（*何を*: dirty kind +
/// reach）。*どの*要素をマークするか（例: shape 変更で囲う IFC 根を解決する）は
/// `tree.rs` に残る。
pub(crate) fn classify(prop: &StyleProp, _ctx: ElementContext) -> Change {
    let dirty_kind = if prop_affects_text_shaping(prop) {
        DirtyKind::Shape
    } else {
        DirtyKind::Visual
    };
    Change {
        dirty_kind,
        reach: invalidation_reach_for_prop(prop),
    }
}

/// 子の追加/取り外しを分類する。IFC への追加（IFC 根直下の `text` 子）が IFC を
/// 再シェープするか、構造投影の reconcile で済むかはトポロジが決める。
pub(crate) fn classify_attachment(
    parent_ctx: ElementContext,
    child_ctx: ElementContext,
) -> Change {
    if parent_ctx.is_ifc_root && child_ctx.kind == ElementKind::Text {
        Change {
            dirty_kind: DirtyKind::Shape,
            reach: VisualInvalidationReach::Subtree,
        }
    } else {
        Change {
            dirty_kind: DirtyKind::Structure,
            reach: VisualInvalidationReach::Subtree,
        }
    }
}

/// reach 伝播の唯一の源: `reach` のもとで `parent_ctx` から `child_ctx` へ走査が
/// 運ぶ reach を返す。その子へ降りない場合は `None`。保持シーン走査と patch-root
/// 探索の両方がここを経由する。
pub(crate) fn step_reach(
    reach: VisualInvalidationReach,
    parent_ctx: ElementContext,
    child_ctx: ElementContext,
) -> Option<VisualInvalidationReach> {
    match reach {
        VisualInvalidationReach::Subtree => Some(VisualInvalidationReach::Subtree),
        VisualInvalidationReach::TextLocal
            if parent_ctx.is_ifc_root && child_ctx.is_inline_text() =>
        {
            Some(VisualInvalidationReach::TextLocal)
        }
        _ => None,
    }
}

/// reach 走査が必要とする読み取り専用トポロジ: 親チェーン、各要素の無効化文脈、
/// z 順の子。`ElementTree` がライブツリー上で、[`ElementMapTopology`] が素の要素
/// マップ上で（ライブツリーを持たない呼び出し側向けに）実装する。trait に保つ
/// ことで、以下の reach 走査をフェイク相手に単体テストできる（`Change → sinks`
/// ルーティングを `RecordingSink` でテストするのと同じ）。
pub(crate) trait ReachTopology {
    fn parent(&self, id: ElementId) -> Option<ElementId>;
    fn element_context(&self, id: ElementId) -> ElementContext;
    fn ordered_children(&self, id: ElementId) -> Vec<ElementId>;
}

impl ReachTopology for ElementTree {
    fn parent(&self, id: ElementId) -> Option<ElementId> {
        self.elements.get(&id).and_then(|el| el.parent)
    }
    fn element_context(&self, id: ElementId) -> ElementContext {
        ElementTree::element_context(self, id)
    }
    fn ordered_children(&self, id: ElementId) -> Vec<ElementId> {
        ElementTree::ordered_children(self, id)
    }
}

/// 素の `elements` マップ上の [`ReachTopology`]。Taffy 投影の reconcile は
/// ライブな `ElementTree` を持たず要素マップのみを持つため、これで reach を
/// 走査する。子はドキュメント順で返る（投影が経由するのは親チェーンを読む
/// patch-root 探索のみで、子は読まない）。
pub(crate) struct ElementMapTopology<'a> {
    pub elements: &'a HashMap<ElementId, Element>,
}

impl ReachTopology for ElementMapTopology<'_> {
    fn parent(&self, id: ElementId) -> Option<ElementId> {
        self.elements.get(&id).and_then(|el| el.parent)
    }
    fn element_context(&self, id: ElementId) -> ElementContext {
        element_context_in(self.elements, id)
    }
    fn ordered_children(&self, id: ElementId) -> Vec<ElementId> {
        self.elements
            .get(&id)
            .map(|el| el.children.clone())
            .unwrap_or_default()
    }
}

/// `reach` のもとで `id` から reach 走査が降りる子を、各々が運ぶ reach と対にして
/// 返す。すべて唯一の源 [`step_reach`] から導かれる: `step_reach` が `Some` を
/// 返すときに限り子へ降りる。保持シーン走査の子降下もここを経由する。
pub(crate) fn children_for_reach<T: ReachTopology>(
    topology: &T,
    id: ElementId,
    reach: VisualInvalidationReach,
) -> Vec<(ElementId, VisualInvalidationReach)> {
    let parent_ctx = topology.element_context(id);
    topology
        .ordered_children(id)
        .into_iter()
        .filter_map(|child| {
            step_reach(reach, parent_ctx, topology.element_context(child))
                .map(|child_reach| (child, child_reach))
        })
        .collect()
}

/// reach タグ付き dirty セットの最小 patch root: どの dirty 祖先の reach でも
/// 自動的に再発行されない dirty 要素すべて。保持シーン走査の部分再ロワーと
/// Taffy 投影の構造 reconcile 双方の唯一の源。祖先の reach が実際に特定の子孫まで
/// 伝播するかは [`step_reach`] が決める。
pub(crate) fn minimal_patch_roots<T: ReachTopology>(
    topology: &T,
    dirty: &HashMap<ElementId, VisualInvalidationReach>,
) -> Vec<ElementId> {
    dirty
        .keys()
        .copied()
        .filter(|&id| !covered_by_dirty_ancestor(topology, id, dirty))
        .collect()
}

/// ある dirty 祖先の再走査が `id` を自動的に再発行し、`id` が自前の patch root に
/// なる必要がないか。祖先の reach が祖先→id の経路を実際に伝播する場合のみ真。
/// `SelfOnly` / `ZIndex` の祖先は自分自身のみ再発行するため、その下の dirty な
/// 子孫（例: トランジション中の親の下にある独立した進行中トランジション）は
/// 自前の patch root のままでないと再ロワーがスキップされる。
fn covered_by_dirty_ancestor<T: ReachTopology>(
    topology: &T,
    id: ElementId,
    dirty: &HashMap<ElementId, VisualInvalidationReach>,
) -> bool {
    // id → 根の経路: chain[0] = id, chain[i+1] = parent(chain[i])。
    let mut chain = vec![id];
    let mut current = topology.parent(id);
    while let Some(parent) = current {
        chain.push(parent);
        current = topology.parent(parent);
    }
    // 各 dirty 祖先について、保持走査と同じく唯一の源 `step_reach` で reach が
    // `id` まで降りる様子をシミュレートする。
    for (ancestor_idx, &ancestor) in chain.iter().enumerate().skip(1) {
        let Some(&ancestor_reach) = dirty.get(&ancestor) else {
            continue;
        };
        let mut reach = ancestor_reach;
        let mut parent = ancestor;
        let mut reached = true;
        for child_idx in (0..ancestor_idx).rev() {
            let child = chain[child_idx];
            match step_reach(
                reach,
                topology.element_context(parent),
                topology.element_context(child),
            ) {
                Some(next) => {
                    reach = next;
                    parent = child;
                }
                None => {
                    reached = false;
                    break;
                }
            }
        }
        if reached {
            return true;
        }
    }
    false
}

/// visual-dirty な要素に対しシーン再ロワーがどこまで届くべきか。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum VisualInvalidationReach {
    /// 当該要素のボックス視覚のみ（背景・ボーダー・不透明度）。
    SelfOnly,
    /// SelfOnly に加え親内の兄弟並べ替え（`z-index`）。
    ZIndex,
    /// IFC 内のテキスト子孫。
    TextLocal,
    /// サブツリー全体（環境的な `default-*` 等、ブロックを貫く変更）。
    Subtree,
}

impl VisualInvalidationReach {
    pub(crate) fn merge(self, other: Self) -> Self {
        self.max(other)
    }
}

/// 視覚スタイルプロパティの無効化 reach を分類する。
pub(crate) fn invalidation_reach_for_prop(prop: &StyleProp) -> VisualInvalidationReach {
    match prop {
        StyleProp::BackgroundColor(_)
        | StyleProp::Opacity(_)
        | StyleProp::BorderRadius(_)
        | StyleProp::BorderWidth(_)
        | StyleProp::BorderColor(_) => VisualInvalidationReach::SelfOnly,
        StyleProp::ZIndex(_) => VisualInvalidationReach::ZIndex,
        StyleProp::Color(_) | StyleProp::FontStyle(_) | StyleProp::TextDecoration(_) => {
            VisualInvalidationReach::TextLocal
        }
        StyleProp::DefaultColor(_)
        | StyleProp::DefaultFontFamily(_)
        | StyleProp::DefaultFontSize(_)
        | StyleProp::DefaultFontWeight(_) => VisualInvalidationReach::Subtree,
        StyleProp::FontSize(_) | StyleProp::FontFamily(_) | StyleProp::FontWeight(_) => {
            VisualInvalidationReach::Subtree
        }
        _ => VisualInvalidationReach::Subtree,
    }
}

pub(crate) fn merge_reach(
    map: &mut HashMap<ElementId, VisualInvalidationReach>,
    id: ElementId,
    reach: VisualInvalidationReach,
) {
    map.entry(id)
        .and_modify(|existing| *existing = existing.merge(reach))
        .or_insert(reach);
}

/// `z-index` 変更後に要素アンカーの子を並べ替えるべき親。
pub(crate) fn z_index_reorder_parent(
    tree: &ElementTree,
    id: ElementId,
) -> Option<ElementId> {
    tree.elements.get(&id).and_then(|el| el.parent)
}

/// `id` と、再ロワーが必要な text-local 子孫を挿入する。
pub(crate) fn expand_text_local(
    tree: &ElementTree,
    id: ElementId,
    out: &mut HashMap<ElementId, VisualInvalidationReach>,
) {
    merge_reach(out, id, VisualInvalidationReach::TextLocal);
    if is_ifc_root(&tree.elements, id) {
        if let Some(el) = tree.elements.get(&id) {
            for &child in &el.children {
                if is_inline_text_element(&tree.elements, child) {
                    merge_reach(out, child, VisualInvalidationReach::TextLocal);
                }
            }
        }
    }
}

pub(crate) fn expand_subtree(
    tree: &ElementTree,
    root: ElementId,
    out: &mut HashMap<ElementId, VisualInvalidationReach>,
) {
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        merge_reach(out, id, VisualInvalidationReach::Subtree);
        if let Some(el) = tree.elements.get(&id) {
            stack.extend(el.children.iter().copied());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;
    use crate::element::kind::ElementKind;

    fn ctx(kind: ElementKind, is_ifc_root: bool, has_text_parent: bool) -> ElementContext {
        ElementContext {
            kind,
            is_ifc_root,
            has_text_parent,
        }
    }

    /// `Change → マークされたセット` 表を `ElementTree` を起動せず検証できるよう、
    /// すべての dirty セットヒットを記録する（ADR-0099）。
    #[derive(Default)]
    struct RecordingSink {
        calls: Vec<SinkCall>,
    }

    #[derive(Debug, PartialEq, Eq)]
    enum SinkCall {
        Visual(ElementId, VisualInvalidationReach),
        Shape(ElementId, VisualInvalidationReach),
        Structure(ElementId),
        Geometry(ElementId),
    }

    impl DirtySink for RecordingSink {
        fn mark_visual(&mut self, id: ElementId, reach: VisualInvalidationReach) {
            self.calls.push(SinkCall::Visual(id, reach));
        }
        fn mark_shape(&mut self, id: ElementId, reach: VisualInvalidationReach) {
            self.calls.push(SinkCall::Shape(id, reach));
        }
        fn mark_structure(&mut self, id: ElementId) {
            self.calls.push(SinkCall::Structure(id));
        }
        fn mark_geometry(&mut self, id: ElementId) {
            self.calls.push(SinkCall::Geometry(id));
        }
    }

    #[test]
    fn route_shape_atomically_marks_shape_and_geometry() {
        let id = ElementId::from_u64(7);
        let mut sink = RecordingSink::default();
        route_change(
            &mut sink,
            id,
            Change {
                dirty_kind: DirtyKind::Shape,
                reach: VisualInvalidationReach::TextLocal,
            },
        );
        assert_eq!(
            sink.calls,
            vec![
                SinkCall::Shape(id, VisualInvalidationReach::TextLocal),
                SinkCall::Geometry(id),
            ]
        );
    }

    #[test]
    fn route_visual_marks_visual_only() {
        let id = ElementId::from_u64(3);
        let mut sink = RecordingSink::default();
        route_change(
            &mut sink,
            id,
            Change {
                dirty_kind: DirtyKind::Visual,
                reach: VisualInvalidationReach::SelfOnly,
            },
        );
        assert_eq!(
            sink.calls,
            vec![SinkCall::Visual(id, VisualInvalidationReach::SelfOnly)]
        );
    }

    #[test]
    fn route_structure_marks_structure_only() {
        let id = ElementId::from_u64(5);
        let mut sink = RecordingSink::default();
        route_change(
            &mut sink,
            id,
            Change {
                dirty_kind: DirtyKind::Structure,
                reach: VisualInvalidationReach::Subtree,
            },
        );
        // Structure はエンジンの visual/shape セットや投影ジオメトリを直接触らない。
        // reconcile が種点からサブツリーを展開する。
        assert_eq!(sink.calls, vec![SinkCall::Structure(id)]);
    }

    #[test]
    fn classify_font_size_reaches_subtree() {
        let c = classify(
            &StyleProp::FontSize(20.0),
            ctx(ElementKind::Text, true, false),
        );
        assert_eq!(c.reach, VisualInvalidationReach::Subtree);
        assert_eq!(c.dirty_kind, DirtyKind::Shape);
    }

    #[test]
    fn classify_background_is_self_only_visual() {
        let c = classify(
            &StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
            ctx(ElementKind::View, false, false),
        );
        assert_eq!(c.dirty_kind, DirtyKind::Visual);
        assert_eq!(c.reach, VisualInvalidationReach::SelfOnly);
    }

    #[test]
    fn classify_color_is_text_local_shape() {
        // 色は Parley の run brush に焼き込まれるため再シェープを起こすが、
        // その reach は IFC に閉じる。
        let c = classify(
            &StyleProp::Color(Color::new(0.0, 1.0, 0.0, 1.0)),
            ctx(ElementKind::Text, true, false),
        );
        assert_eq!(c.dirty_kind, DirtyKind::Shape);
        assert_eq!(c.reach, VisualInvalidationReach::TextLocal);
    }

    #[test]
    fn classify_z_index_is_visual_zindex_reach() {
        let c = classify(
            &StyleProp::ZIndex(3),
            ctx(ElementKind::View, false, false),
        );
        assert_eq!(c.dirty_kind, DirtyKind::Visual);
        assert_eq!(c.reach, VisualInvalidationReach::ZIndex);
    }

    #[test]
    fn classify_default_color_pierces_whole_subtree() {
        let c = classify(
            &StyleProp::DefaultColor(Color::new(1.0, 0.0, 0.0, 1.0)),
            ctx(ElementKind::View, false, false),
        );
        assert_eq!(c.reach, VisualInvalidationReach::Subtree);
    }

    #[test]
    fn classify_merge_takes_widest_concern_and_reach() {
        let visual = classify(
            &StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
            ctx(ElementKind::Text, true, false),
        );
        let shape = classify(
            &StyleProp::FontSize(18.0),
            ctx(ElementKind::Text, true, false),
        );
        let merged = visual.merge(shape);
        assert_eq!(merged.dirty_kind, DirtyKind::Shape);
        assert_eq!(merged.reach, VisualInvalidationReach::Subtree);
    }

    #[test]
    fn attachment_into_ifc_root_reshapes() {
        let parent = ctx(ElementKind::Text, true, false);
        let child = ctx(ElementKind::Text, false, true);
        assert_eq!(
            classify_attachment(parent, child).dirty_kind,
            DirtyKind::Shape
        );
    }

    #[test]
    fn attachment_into_plain_parent_restructures() {
        let parent = ctx(ElementKind::View, false, false);
        let child = ctx(ElementKind::View, false, false);
        assert_eq!(
            classify_attachment(parent, child).dirty_kind,
            DirtyKind::Structure
        );
    }

    #[test]
    fn step_reach_subtree_always_descends_subtree() {
        let parent = ctx(ElementKind::View, false, false);
        let child = ctx(ElementKind::View, false, false);
        assert_eq!(
            step_reach(VisualInvalidationReach::Subtree, parent, child),
            Some(VisualInvalidationReach::Subtree)
        );
    }

    #[test]
    fn step_reach_text_local_descends_only_ifc_root_to_inline_text() {
        let ifc_root = ctx(ElementKind::Text, true, false);
        let inline = ctx(ElementKind::Text, false, true);
        let block = ctx(ElementKind::View, false, false);
        // IFC 根 → インラインテキスト子孫: 伝播する。
        assert_eq!(
            step_reach(VisualInvalidationReach::TextLocal, ifc_root, inline),
            Some(VisualInvalidationReach::TextLocal)
        );
        // IFC 根 → 非インライン子: 降りない。
        assert_eq!(
            step_reach(VisualInvalidationReach::TextLocal, ifc_root, block),
            None
        );
        // インラインテキスト（IFC 根ではない）→ 子: ここで止まる。
        assert_eq!(
            step_reach(VisualInvalidationReach::TextLocal, inline, inline),
            None
        );
    }

    #[test]
    fn step_reach_self_only_and_z_index_never_descend() {
        let parent = ctx(ElementKind::Text, true, false);
        let child = ctx(ElementKind::Text, false, true);
        assert_eq!(
            step_reach(VisualInvalidationReach::SelfOnly, parent, child),
            None
        );
        assert_eq!(
            step_reach(VisualInvalidationReach::ZIndex, parent, child),
            None
        );
    }

    /// 素のマップから構築した `ReachTopology`。集約した reach 走査
    /// (`children_for_reach` / `minimal_patch_roots`) を `ElementTree` を起動せず
    /// 公開インターフェース越しに検証する（`RecordingSink` がルーティング接合点に
    /// 与えるのと同じテスト姿勢）。
    #[derive(Default)]
    struct FakeTopology {
        parents: HashMap<ElementId, ElementId>,
        contexts: HashMap<ElementId, ElementContext>,
        children: HashMap<ElementId, Vec<ElementId>>,
    }

    impl FakeTopology {
        fn add(&mut self, id: u64, parent: Option<u64>, context: ElementContext) {
            let eid = ElementId::from_u64(id);
            if let Some(p) = parent {
                let pid = ElementId::from_u64(p);
                self.parents.insert(eid, pid);
                self.children.entry(pid).or_default().push(eid);
            }
            self.contexts.insert(eid, context);
        }
    }

    impl ReachTopology for FakeTopology {
        fn parent(&self, id: ElementId) -> Option<ElementId> {
            self.parents.get(&id).copied()
        }
        fn element_context(&self, id: ElementId) -> ElementContext {
            self.contexts
                .get(&id)
                .copied()
                .unwrap_or_else(|| ctx(ElementKind::View, false, false))
        }
        fn ordered_children(&self, id: ElementId) -> Vec<ElementId> {
            self.children.get(&id).cloned().unwrap_or_default()
        }
    }

    fn dirty_map(
        entries: &[(u64, VisualInvalidationReach)],
    ) -> HashMap<ElementId, VisualInvalidationReach> {
        entries
            .iter()
            .map(|&(id, reach)| (ElementId::from_u64(id), reach))
            .collect()
    }

    fn id_set(ids: &[u64]) -> HashSet<ElementId> {
        ids.iter().map(|&id| ElementId::from_u64(id)).collect()
    }

    #[test]
    fn children_for_reach_subtree_descends_into_every_child() {
        let mut topo = FakeTopology::default();
        topo.add(1, None, ctx(ElementKind::View, false, false));
        topo.add(2, Some(1), ctx(ElementKind::View, false, false));
        topo.add(3, Some(1), ctx(ElementKind::View, false, false));

        let descended = children_for_reach(
            &topo,
            ElementId::from_u64(1),
            VisualInvalidationReach::Subtree,
        );
        assert_eq!(
            descended,
            vec![
                (ElementId::from_u64(2), VisualInvalidationReach::Subtree),
                (ElementId::from_u64(3), VisualInvalidationReach::Subtree),
            ]
        );
    }

    #[test]
    fn children_for_reach_text_local_descends_only_inline_text_under_ifc_root() {
        let mut topo = FakeTopology::default();
        topo.add(1, None, ctx(ElementKind::Text, true, false));
        topo.add(2, Some(1), ctx(ElementKind::Text, false, true)); // インラインテキスト
        topo.add(3, Some(1), ctx(ElementKind::View, false, false)); // ブロック子

        let descended = children_for_reach(
            &topo,
            ElementId::from_u64(1),
            VisualInvalidationReach::TextLocal,
        );
        assert_eq!(
            descended,
            vec![(ElementId::from_u64(2), VisualInvalidationReach::TextLocal)]
        );
    }

    #[test]
    fn children_for_reach_self_only_descends_into_nothing() {
        let mut topo = FakeTopology::default();
        topo.add(1, None, ctx(ElementKind::View, false, false));
        topo.add(2, Some(1), ctx(ElementKind::View, false, false));

        let descended = children_for_reach(
            &topo,
            ElementId::from_u64(1),
            VisualInvalidationReach::SelfOnly,
        );
        assert!(descended.is_empty());
    }

    #[test]
    fn minimal_patch_roots_subtree_ancestor_covers_descendant() {
        // 1 → 2 → 3、1 と 3 が共に Subtree reach で dirty: 1 の再走査が 3 を
        // 自動的に再発行するため、3 は自前の patch root にならない。
        let mut topo = FakeTopology::default();
        topo.add(1, None, ctx(ElementKind::View, false, false));
        topo.add(2, Some(1), ctx(ElementKind::View, false, false));
        topo.add(3, Some(2), ctx(ElementKind::View, false, false));

        let dirty = dirty_map(&[
            (1, VisualInvalidationReach::Subtree),
            (3, VisualInvalidationReach::Subtree),
        ]);
        let roots: HashSet<ElementId> =
            minimal_patch_roots(&topo, &dirty).into_iter().collect();
        assert_eq!(roots, id_set(&[1]));
    }

    #[test]
    fn minimal_patch_roots_self_only_ancestor_does_not_cover_descendant() {
        // `SelfOnly` の祖先は自分自身のみ再発行するため、独立に dirty な子孫は
        // 自前の patch root のままでなければならない。
        let mut topo = FakeTopology::default();
        topo.add(1, None, ctx(ElementKind::View, false, false));
        topo.add(2, Some(1), ctx(ElementKind::View, false, false));
        topo.add(3, Some(2), ctx(ElementKind::View, false, false));

        let dirty = dirty_map(&[
            (1, VisualInvalidationReach::SelfOnly),
            (3, VisualInvalidationReach::Subtree),
        ]);
        let roots: HashSet<ElementId> =
            minimal_patch_roots(&topo, &dirty).into_iter().collect();
        assert_eq!(roots, id_set(&[1, 3]));
    }

    /// 存在のみで判定する祖先チェック。投影の旧 `has_dirty_ancestor` であり、
    /// `Subtree` 特殊化が今も一致すべき基準としてここに残す。
    fn presence_only_roots(topo: &FakeTopology, dirty: &[u64]) -> HashSet<ElementId> {
        let dirty_ids = id_set(dirty);
        dirty_ids
            .iter()
            .copied()
            .filter(|&id| {
                let mut cur = topo.parent(id);
                while let Some(ancestor) = cur {
                    if dirty_ids.contains(&ancestor) {
                        return false;
                    }
                    cur = topo.parent(ancestor);
                }
                true
            })
            .collect()
    }

    #[test]
    fn minimal_patch_roots_all_subtree_equals_presence_only_specialization() {
        // Taffy 投影は全エントリを `Subtree` でタグ付けした `structure_dirty` を
        // 供給する。`step_reach` は `Subtree` を常に伝播するため、共有 reach
        // カーネル経由でも旧来の存在のみ patch-root 探索を完全に再現する必要が
        // ある（投影の挙動は不変）。
        let mut topo = FakeTopology::default();
        topo.add(1, None, ctx(ElementKind::View, false, false));
        topo.add(2, Some(1), ctx(ElementKind::Text, true, false)); // IFC 根
        topo.add(3, Some(2), ctx(ElementKind::Text, false, true)); // インラインテキスト
        topo.add(4, Some(1), ctx(ElementKind::View, false, false));
        topo.add(5, Some(4), ctx(ElementKind::View, false, false));

        // 多様な形状: ネストしたチェーン、IFC 境界、独立した枝。
        for dirty in [
            vec![1u64],
            vec![2, 3],
            vec![1, 3, 5],
            vec![2, 4],
            vec![3, 5],
            vec![1, 2, 3, 4, 5],
        ] {
            let map = dirty_map(
                &dirty
                    .iter()
                    .map(|&id| (id, VisualInvalidationReach::Subtree))
                    .collect::<Vec<_>>(),
            );
            let reach_roots: HashSet<ElementId> =
                minimal_patch_roots(&topo, &map).into_iter().collect();
            assert_eq!(
                reach_roots,
                presence_only_roots(&topo, &dirty),
                "all-Subtree reach must match presence-only roots for {dirty:?}"
            );
        }
    }
}

pub(crate) fn apply_visual_invalidation(
    tree: &ElementTree,
    id: ElementId,
    reach: VisualInvalidationReach,
    elements: &mut HashMap<ElementId, VisualInvalidationReach>,
    z_index_parents: &mut HashSet<ElementId>,
) {
    match reach {
        VisualInvalidationReach::SelfOnly => {
            merge_reach(elements, id, reach);
        }
        VisualInvalidationReach::ZIndex => {
            merge_reach(elements, id, reach);
            if let Some(parent) = z_index_reorder_parent(tree, id) {
                z_index_parents.insert(parent);
            }
        }
        VisualInvalidationReach::TextLocal => {
            expand_text_local(tree, id, elements);
        }
        VisualInvalidationReach::Subtree => {
            expand_subtree(tree, id, elements);
        }
    }
}
