//! compositing layer の境界判定と `layer_dirty` 導出（ADR-0125 コア半分）。
//!
//! ADR-0125 は「raster は内容が変わったレイヤだけ・高コスト／composite は毎フレーム・低コスト」の
//! 分離を core/backend に敷く。本モジュールはその **コア半分**で、`ElementTree` を起動しない純関数
//! 群として、(1) どの要素が compositing layer の境界か、(2) 要素 dirty からそれを内包する最近接
//! レイヤを `layer_dirty` として導出する、を担う（ADR-0099/0086 の reach-table 方式と同形）。
//!
//! バックエンドは本 slice ではレイヤを無視してよい（layers があっても全面描画＝出力同一）。ここは
//! seam を敷くだけの prefactor で、出力は一切変えない。レイヤ id ＝境界要素の `ElementId`。

use std::collections::{HashMap, HashSet};

use crate::element::id::ElementId;

/// 要素を独立 compositing layer の境界にする trigger（ADR-0125 の初期集合）。
///
/// 名前付き enum で表現し、判定ロジックにマジックナンバー/文字列が散在しないようにする。
/// `opacity` は damage 的に静的なので初期集合には含めない（ADR-0125 Decision 1）。作者 opt-in の
/// `will-change` 等の primitive も導入しない。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompositingTrigger {
    /// アクティブな transition / animation。補間中で毎フレーム再描画され得るため、
    /// 専用レイヤに隔離して composite 側の安いフレームで動かす。
    Transition,
    /// `transform` group（ADR-0020）。キャッシュ texture の平行移動/変換で安く動かせる。
    Transform,
    /// scroll コンテナ。内容を texture 化し、スクロールをキャッシュ面の平行移動で済ませる。
    ScrollContainer,
}

impl CompositingTrigger {
    /// 初期 trigger 集合（ADR-0125）。レンダラ横断・テストで参照する単一の正本。順序は判定の
    /// 安定性のため固定する。
    pub const INITIAL_SET: [CompositingTrigger; 3] = [
        CompositingTrigger::Transition,
        CompositingTrigger::Transform,
        CompositingTrigger::ScrollContainer,
    ];
}

/// 1 要素の compositing 関連の事実。lowering が live state（進行中トランジション / `transform` の
/// 有無 / `ScrollView` か）から組み、判定はこの事実だけを受け取る純関数に閉じる（ElementTree 非依存）。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CompositingFacts {
    /// アクティブな transition / animation を持つ（`AnchorEntry.transitions.is_active()`）。
    pub has_active_transition: bool,
    /// `transform`（ADR-0020 の group node）を持つ。
    pub has_transform: bool,
    /// scroll コンテナ（`ElementKind::ScrollView`）。
    pub is_scroll_container: bool,
}

impl CompositingFacts {
    fn has(self, trigger: CompositingTrigger) -> bool {
        match trigger {
            CompositingTrigger::Transition => self.has_active_transition,
            CompositingTrigger::Transform => self.has_transform,
            CompositingTrigger::ScrollContainer => self.is_scroll_container,
        }
    }

    /// この要素に当たる trigger を初期集合の順で返す。
    pub fn triggers(self) -> Vec<CompositingTrigger> {
        CompositingTrigger::INITIAL_SET
            .into_iter()
            .filter(|&t| self.has(t))
            .collect()
    }

    /// trigger が 1 つでも当たれば独立 compositing layer の境界。
    pub fn forms_layer(self) -> bool {
        CompositingTrigger::INITIAL_SET.iter().any(|&t| self.has(t))
    }
}

/// `element` を内包する最近接の compositing layer を返す。`element` 自身を含めて親チェーンを上り、
/// 最初に出会う境界要素を返す。どの境界にも内包されなければ `None`（ルートレイヤ外）。
///
/// `parent_of` は document ツリーの親引き（owner はあくまで document ツリー、ADR-0125）。本関数は
/// `ElementTree` を起動しない純関数。
pub fn nearest_enclosing_layer<F>(
    element: ElementId,
    boundaries: &HashSet<ElementId>,
    parent_of: F,
) -> Option<ElementId>
where
    F: Fn(ElementId) -> Option<ElementId>,
{
    let mut cursor = Some(element);
    while let Some(id) = cursor {
        if boundaries.contains(&id) {
            return Some(id);
        }
        cursor = parent_of(id);
    }
    None
}

/// dirty 要素集合から `layer_dirty`（再 raster すべきレイヤ集合）を導出する（ADR-0125）。各 dirty
/// 要素を内包する最近接レイヤを dirty とし、どのレイヤにも内包されない dirty は無視する。同一レイヤに
/// 複数 dirty が落ちても集合で 1 つに畳まれる。
pub fn derive_layer_dirty<I, F>(
    dirty: I,
    boundaries: &HashSet<ElementId>,
    parent_of: F,
) -> HashSet<ElementId>
where
    I: IntoIterator<Item = ElementId>,
    F: Fn(ElementId) -> Option<ElementId>,
{
    dirty
        .into_iter()
        .filter_map(|d| nearest_enclosing_layer(d, boundaries, &parent_of))
        .collect()
}

/// compositing layer の親子関係（境界要素のみのツリー）。レイヤ id ＝境界要素の `ElementId`。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LayerTree {
    /// 全レイヤ id（境界要素）を投入順で保持する。
    pub layers: Vec<ElementId>,
    /// 各レイヤの親レイヤ（最近接の祖先境界）。ルートレイヤは `None`。
    pub parent: HashMap<ElementId, Option<ElementId>>,
}

/// 境界要素集合と document 親引きから compositing layer ツリーを構築する。各レイヤの親は、その境界
/// 要素の親チェーン上にある最近接の別境界（自身は除く）。
pub fn build_layer_tree<I, F>(
    boundaries_in_order: I,
    boundaries: &HashSet<ElementId>,
    parent_of: F,
) -> LayerTree
where
    I: IntoIterator<Item = ElementId>,
    F: Fn(ElementId) -> Option<ElementId>,
{
    let mut tree = LayerTree::default();
    for layer in boundaries_in_order {
        let parent_layer = parent_of(layer)
            .and_then(|p| nearest_enclosing_layer(p, boundaries, &parent_of));
        tree.layers.push(layer);
        tree.parent.insert(layer, parent_layer);
    }
    tree
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(raw: u64) -> ElementId {
        ElementId::from_u64(raw)
    }

    /// 親リンクの map から `parent_of` クロージャを作る（ElementTree 非依存のテスト位相）。
    fn parents(pairs: &[(u64, u64)]) -> impl Fn(ElementId) -> Option<ElementId> + '_ {
        move |child: ElementId| {
            pairs
                .iter()
                .find(|(c, _)| id(*c) == child)
                .map(|(_, p)| id(*p))
        }
    }

    fn boundary_set(ids: &[u64]) -> HashSet<ElementId> {
        ids.iter().map(|&r| id(r)).collect()
    }

    #[test]
    fn default_facts_form_no_layer() {
        let facts = CompositingFacts::default();
        assert!(facts.triggers().is_empty());
        assert!(!facts.forms_layer());
    }

    #[test]
    fn scroll_container_alone_forms_a_layer() {
        let facts = CompositingFacts {
            is_scroll_container: true,
            ..CompositingFacts::default()
        };
        assert_eq!(facts.triggers(), vec![CompositingTrigger::ScrollContainer]);
        assert!(facts.forms_layer());
    }

    #[test]
    fn triggers_are_reported_in_initial_set_order() {
        // transition と transform が両方当たっても、報告順は INITIAL_SET の固定順。
        let facts = CompositingFacts {
            has_active_transition: true,
            has_transform: true,
            is_scroll_container: false,
        };
        assert_eq!(
            facts.triggers(),
            vec![CompositingTrigger::Transition, CompositingTrigger::Transform],
        );
    }

    #[test]
    fn nearest_layer_is_the_innermost_enclosing_boundary() {
        // 位相: root(1) > layerA(2) > mid(3) > layerB(4) > leaf(5)
        let parent_of = parents(&[(2, 1), (3, 2), (4, 3), (5, 4)]);
        let boundaries = boundary_set(&[2, 4]);
        // leaf(5) は layerB(4) に内包される（最近接）。
        assert_eq!(nearest_enclosing_layer(id(5), &boundaries, &parent_of), Some(id(4)));
        // mid(3) は layerA(2) に内包される。
        assert_eq!(nearest_enclosing_layer(id(3), &boundaries, &parent_of), Some(id(2)));
    }

    #[test]
    fn boundary_element_is_its_own_layer() {
        let parent_of = parents(&[(2, 1)]);
        let boundaries = boundary_set(&[2]);
        assert_eq!(nearest_enclosing_layer(id(2), &boundaries, &parent_of), Some(id(2)));
    }

    #[test]
    fn element_outside_any_layer_has_no_enclosing_layer() {
        let parent_of = parents(&[(2, 1)]);
        let boundaries = boundary_set(&[]); // 境界なし
        assert_eq!(nearest_enclosing_layer(id(2), &boundaries, &parent_of), None);
    }

    #[test]
    fn layer_dirty_routes_each_dirty_to_its_nearest_layer_and_dedups() {
        // root(1) > layerA(2) > a1(3), a2(4); layerB(5) > b1(6)
        let parent_of = parents(&[(2, 1), (3, 2), (4, 2), (5, 1), (6, 5)]);
        let boundaries = boundary_set(&[2, 5]);
        // a1,a2 はどちらも layerA(2)→集合で 1 つ。b1 は layerB(5)。root(1) はレイヤ外で無視。
        let dirty = derive_layer_dirty([id(3), id(4), id(6), id(1)], &boundaries, &parent_of);
        assert_eq!(dirty, boundary_set(&[2, 5]));
    }

    #[test]
    fn layer_tree_parent_is_nearest_ancestor_boundary() {
        // root(1) > layerA(2) > mid(3) > layerB(4)
        let parent_of = parents(&[(2, 1), (3, 2), (4, 3)]);
        let boundaries = boundary_set(&[2, 4]);
        let tree = build_layer_tree([id(2), id(4)], &boundaries, &parent_of);
        assert_eq!(tree.layers, vec![id(2), id(4)]);
        // layerA(2) はルートレイヤ（祖先に境界なし）。layerB(4) の親は layerA(2)。
        assert_eq!(tree.parent.get(&id(2)), Some(&None));
        assert_eq!(tree.parent.get(&id(4)), Some(&Some(id(2))));
    }
}
