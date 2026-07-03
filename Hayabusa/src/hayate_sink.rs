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
use crate::style::{self, StyleProp};
use hayate_core::{
    AlignValue, Color, Dimension, DisplayValue, ElementId, ElementKind as CoreKind, ElementTree,
    FlexDirectionValue, JustifyValue, StyleProp as CoreStyleProp,
};

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

/// Hayabusa の `Length` を core の `Dimension` へ写す。
fn to_core_dim(len: style::Length) -> Dimension {
    match len {
        style::Length::Px(v) => Dimension::px(v),
        style::Length::Percent(v) => Dimension::percent(v),
        style::Length::Auto => Dimension::AUTO,
    }
}

/// Hayabusa の `Rgba`（0..1）を core の `Color` へ写す。
fn to_core_color(c: style::Rgba) -> Color {
    Color::new(c.r as f64, c.g as f64, c.b as f64, c.a as f64)
}

/// Hayabusa の static `StyleProp` を core の `StyleProp` へ写す（ADR-0010）。
pub(crate) fn to_core_style(prop: StyleProp) -> CoreStyleProp {
    match prop {
        StyleProp::Width(l) => CoreStyleProp::Width(to_core_dim(l)),
        StyleProp::Height(l) => CoreStyleProp::Height(to_core_dim(l)),
        StyleProp::Padding(l) => CoreStyleProp::Padding(to_core_dim(l)),
        StyleProp::Margin(l) => CoreStyleProp::Margin(to_core_dim(l)),
        StyleProp::Gap(l) => CoreStyleProp::Gap(to_core_dim(l)),
        StyleProp::Display(d) => CoreStyleProp::Display(match d {
            style::Display::Flex => DisplayValue::Flex,
            style::Display::Block => DisplayValue::Block,
            style::Display::None => DisplayValue::None,
        }),
        StyleProp::FlexDirection(f) => CoreStyleProp::FlexDirection(match f {
            style::FlexDirection::Row => FlexDirectionValue::Row,
            style::FlexDirection::Column => FlexDirectionValue::Column,
        }),
        StyleProp::AlignItems(a) => CoreStyleProp::AlignItems(match a {
            style::Align::FlexStart => AlignValue::FlexStart,
            style::Align::Center => AlignValue::Center,
            style::Align::FlexEnd => AlignValue::FlexEnd,
            style::Align::Stretch => AlignValue::Stretch,
        }),
        StyleProp::JustifyContent(j) => CoreStyleProp::JustifyContent(match j {
            style::Justify::FlexStart => JustifyValue::FlexStart,
            style::Justify::Center => JustifyValue::Center,
            style::Justify::FlexEnd => JustifyValue::FlexEnd,
            style::Justify::SpaceBetween => JustifyValue::SpaceBetween,
            style::Justify::SpaceAround => JustifyValue::SpaceAround,
            style::Justify::SpaceEvenly => JustifyValue::SpaceEvenly,
        }),
        StyleProp::BackgroundColor(c) => CoreStyleProp::BackgroundColor(to_core_color(c)),
        StyleProp::TextColor(c) => CoreStyleProp::Color(to_core_color(c)),
        StyleProp::FontSize(v) => CoreStyleProp::FontSize(v),
    }
}

/// Hayabusa の static スタイル列を core の列へ写す。
fn to_core_styles(props: &[StyleProp]) -> Vec<CoreStyleProp> {
    props.iter().copied().map(to_core_style).collect()
}

/// `ElementSink` の8操作それぞれが実 `ElementTree` へどう写るかの唯一の配線。
/// live 経路（`impl ElementSink for HayateSink`）と buffered 経路（[`apply_mutation`]）の
/// 両方がこれらを呼ぶ——どちらかを消してももう一方に書き直す必要が無いよう、ここに集約する
/// （値の変換自体は `to_core_*` に既に集約済みで、ここが集約するのは tree メソッド呼び出しの配線）。
fn write_create(tree: &mut ElementTree, id: u64, kind: ElementKind) {
    tree.element_create(id, to_core_kind(kind));
}

fn write_set_text(tree: &mut ElementTree, id: ElId, text: &str) {
    tree.element_set_text(to_core_id(id), text);
}

fn write_append_child(tree: &mut ElementTree, parent: ElId, child: ElId) {
    tree.element_append_child(to_core_id(parent), to_core_id(child));
}

fn write_insert_before(tree: &mut ElementTree, parent: ElId, child: ElId, before: ElId) {
    tree.element_insert_before(to_core_id(parent), to_core_id(child), to_core_id(before));
}

fn write_remove(tree: &mut ElementTree, id: ElId) {
    tree.element_remove(to_core_id(id));
}

fn write_set_root(tree: &mut ElementTree, id: ElId) {
    tree.set_root(to_core_id(id));
}

/// 差分・非組成中ガードは core 側（ADR-0007）。戻り値（適用されたか）は捨てる。
fn write_value(tree: &mut ElementTree, id: ElId, text: &str) {
    tree.element_set_text_content_if_idle(to_core_id(id), text);
}

fn write_style(tree: &mut ElementTree, id: ElId, props: &[StyleProp]) {
    tree.element_set_style(to_core_id(id), &to_core_styles(props));
}

/// 記録済みの 1 件の [`Mutation`] を実 `ElementTree` へ適用する。`RecordingSink` を
/// buffering sink として使う経路（App Host への drain・src/app_host.rs）が、effect が
/// 積んだ mutation 列をフレーム内でこの関数で借用ツリーへ出し切る。`HayateSink` の
/// ライブ転送と同じ `write_*` 配線を使い、id は記録時に払い出し済みのものをそのまま使う。
pub(crate) fn apply_mutation(tree: &mut ElementTree, m: &Mutation) {
    match m {
        Mutation::Create { id, kind } => write_create(tree, id.0, *kind),
        Mutation::SetText { id, text } => write_set_text(tree, *id, text),
        Mutation::AppendChild { parent, child } => write_append_child(tree, *parent, *child),
        Mutation::InsertBefore {
            parent,
            child,
            before,
        } => write_insert_before(tree, *parent, *child, *before),
        Mutation::Remove { id } => write_remove(tree, *id),
        Mutation::SetRoot { id } => write_set_root(tree, *id),
        Mutation::SetValue { id, text } => write_value(tree, *id, text),
        Mutation::SetStyle { id, props } => write_style(tree, *id, props),
    }
}

impl ElementSink for HayateSink {
    fn create_element(&mut self, kind: ElementKind) -> ElId {
        let raw = self.next_id;
        self.next_id += 1;
        write_create(&mut self.tree, raw, kind);
        ElId(raw)
    }

    fn set_text(&mut self, id: ElId, text: &str) {
        write_set_text(&mut self.tree, id, text);
    }

    fn append_child(&mut self, parent: ElId, child: ElId) {
        write_append_child(&mut self.tree, parent, child);
    }

    fn insert_before(&mut self, parent: ElId, child: ElId, before: ElId) {
        write_insert_before(&mut self.tree, parent, child, before);
    }

    fn remove(&mut self, id: ElId) {
        write_remove(&mut self.tree, id);
    }

    fn set_root(&mut self, id: ElId) {
        write_set_root(&mut self.tree, id);
    }

    fn set_value(&mut self, id: ElId, text: &str) {
        write_value(&mut self.tree, id, text);
    }

    fn set_style(&mut self, id: ElId, props: &[StyleProp]) {
        write_style(&mut self.tree, id, props);
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
