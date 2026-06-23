//! `HayateSink` — `ElementSink` を実機の `hayate_core::ElementTree` へ転送する薄い層。
//!
//! `ElementSink`（[`crate::sink`]）の各メソッドは設計上 `ElementTree` の対応 API に 1:1 で
//! 写る（ADR-0002 の host-ABI 線）。本モジュールはその写像をそのまま実装し、tracer bullet で
//! `RecordingSink` が記録していた fine-grained patch を**実際の Element Layer に発行**する。
//!
//! `feature = "hayate-core"` でのみコンパイルされる（既定の self-contained ビルドは
//! Hayabusa ADR-0006 のまま外部依存ゼロを保つ）。クロスワークスペースのリンク可否は
//! Hayabusa ADR-0009 の spike で確認済み（`Cargo.toml` の `[patch.crates-io]` 複製が前提）。
//!
//! ## 所有モデルと後続
//!
//! `Instance<S: ElementSink + 'static>` は sink を `Rc<RefCell<S>>` で握るため、sink は
//! `ElementTree` を**所有**する（`RecordingSink` がログを所有するのと同型）。一方 ADR-0117 の
//! App Host は `ElementTree` 実体を自分で所有し、毎フレーム `&mut ElementTree` を借す
//! `DeliverySink` モデルを採る。両者を繋ぐ event-loop 配線（借用ツリーモデル＋ListenerId →
//! handler ルーティング）は次段の実装で、本モジュールは「ElementSink → 実 ElementTree」の
//! 転送が実機コア上で成立することを先に確定させる。

use crate::sink::{ElId, ElementKind, ElementSink, Mutation};
use hayate_core::{ElementId, ElementKind as CoreKind, ElementTree};

/// `ElementSink` を所有する `hayate_core::ElementTree` へ転送する sink。
///
/// `ElId(n)` と `ElementId::from_u64(n)` を同一視し、要素 id は本 sink が単調増加で払い出す
/// （`hayate_core::element_create` は呼び出し側が u64 id を渡す契約のため）。
pub struct HayateSink {
    tree: ElementTree,
    next_id: u64,
}

impl HayateSink {
    /// 空の `ElementTree` を所有する sink を作る。
    pub fn new() -> Self {
        HayateSink {
            tree: ElementTree::new(),
            next_id: 0,
        }
    }

    /// 駆動している `ElementTree` への不変参照（mutation 結果の検証・描画用）。
    pub fn tree(&self) -> &ElementTree {
        &self.tree
    }

    /// 駆動している `ElementTree` への可変参照。
    pub fn tree_mut(&mut self) -> &mut ElementTree {
        &mut self.tree
    }

    /// 所有を手放して `ElementTree` を取り出す。
    pub fn into_tree(self) -> ElementTree {
        self.tree
    }
}

impl Default for HayateSink {
    fn default() -> Self {
        HayateSink::new()
    }
}

/// Hayabusa の element-kind 語彙を core の語彙へ写す。両者は同じ並びだが、判別子の
/// 値表現に依存せず明示で写して将来のドリフトに備える。
pub(crate) fn to_core_kind(kind: ElementKind) -> CoreKind {
    match kind {
        ElementKind::View => CoreKind::View,
        ElementKind::Text => CoreKind::Text,
        ElementKind::Image => CoreKind::Image,
        ElementKind::Button => CoreKind::Button,
        ElementKind::TextInput => CoreKind::TextInput,
        ElementKind::ScrollView => CoreKind::ScrollView,
    }
}

/// `ElId` を core の `ElementId` へ写す（同一 u64 を共有する全単射）。
pub(crate) fn to_core_id(id: ElId) -> ElementId {
    ElementId::from_u64(id.0)
}

/// 記録済みの 1 件の [`Mutation`] を実 `ElementTree` へ適用する。`RecordingSink` を
/// buffering sink として使う経路（App Host への drain・src/app_host.rs）が、effect が
/// 積んだ mutation 列をフレーム内でこの関数で借用ツリーへ出し切る。`HayateSink` の
/// ライブ転送と同じ 1:1 写像で、id は記録時に払い出し済みのものをそのまま使う。
pub(crate) fn apply_mutation(tree: &mut ElementTree, m: &Mutation) {
    match m {
        Mutation::Create { id, kind } => {
            tree.element_create(id.0, to_core_kind(*kind));
        }
        Mutation::SetText { id, text } => tree.element_set_text(to_core_id(*id), text),
        Mutation::AppendChild { parent, child } => {
            tree.element_append_child(to_core_id(*parent), to_core_id(*child))
        }
        Mutation::InsertBefore {
            parent,
            child,
            before,
        } => tree.element_insert_before(to_core_id(*parent), to_core_id(*child), to_core_id(*before)),
        Mutation::Remove { id } => tree.element_remove(to_core_id(*id)),
        Mutation::SetRoot { id } => tree.set_root(to_core_id(*id)),
    }
}

impl ElementSink for HayateSink {
    fn create_element(&mut self, kind: ElementKind) -> ElId {
        let raw = self.next_id;
        self.next_id += 1;
        self.tree.element_create(raw, to_core_kind(kind));
        ElId(raw)
    }

    fn set_text(&mut self, id: ElId, text: &str) {
        self.tree.element_set_text(to_core_id(id), text);
    }

    fn append_child(&mut self, parent: ElId, child: ElId) {
        self.tree
            .element_append_child(to_core_id(parent), to_core_id(child));
    }

    fn insert_before(&mut self, parent: ElId, child: ElId, before: ElId) {
        self.tree
            .element_insert_before(to_core_id(parent), to_core_id(child), to_core_id(before));
    }

    fn remove(&mut self, id: ElId) {
        self.tree.element_remove(to_core_id(id));
    }

    fn set_root(&mut self, id: ElId) {
        self.tree.set_root(to_core_id(id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_assigns_sequential_ids_and_writes_through_to_the_tree() {
        let mut sink = HayateSink::new();
        let view = sink.create_element(ElementKind::View);
        let text = sink.create_element(ElementKind::Text);
        sink.append_child(view, text);
        sink.set_text(text, "hi");
        sink.set_root(view);

        assert_eq!(view, ElId(0));
        assert_eq!(text, ElId(1));
        // text ノードの内容が実 ElementTree に届いている。
        assert_eq!(sink.tree().element_get_text(to_core_id(text)), "hi");
    }

    #[test]
    fn set_text_targets_only_the_given_node() {
        let mut sink = HayateSink::new();
        let view = sink.create_element(ElementKind::View);
        let a = sink.create_element(ElementKind::Text);
        let b = sink.create_element(ElementKind::Text);
        sink.append_child(view, a);
        sink.append_child(view, b);
        sink.set_text(a, "A");
        sink.set_text(b, "B");

        sink.set_text(a, "A2");
        assert_eq!(sink.tree().element_get_text(to_core_id(a)), "A2");
        assert_eq!(sink.tree().element_get_text(to_core_id(b)), "B");
    }
}
