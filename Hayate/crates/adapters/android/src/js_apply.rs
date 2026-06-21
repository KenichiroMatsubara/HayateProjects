//! 埋め込み Hermes（ADR-0112）が呼ぶ `apply_mutations` を ElementTree へ適用する
//! Rust 側ブリッジ。JSI/C++ ホストは flat-C ABI 越しにここへ降りてくる。
//!
//! 本モジュールは wasm に一切依存せず、共有の中立 dispatch
//! （[`crate::apply_mutations_dispatch`]）を使って Web と同じ適用ロジックを通す。
//! C++/Hermes 配線はデバイスビルド側にあるため、ここはホストで `cargo check` /
//! 単体テスト可能な純 Rust に保つ。
use hayate_core::{ElementId, ElementTree};

use crate::apply_mutations_dispatch::{apply_mutations_batch, ApplyMutationsHost};

/// `&mut ElementTree` を借用するだけの薄い適用ホスト。Web の
/// `HayateElementRenderer` における `ApplyMutationsHost` 実装に対応する。
pub(crate) struct TreeApplyHost<'a> {
    tree: &'a mut ElementTree,
}

impl<'a> TreeApplyHost<'a> {
    pub(crate) fn new(tree: &'a mut ElementTree) -> Self {
        Self { tree }
    }
}

impl ApplyMutationsHost for TreeApplyHost<'_> {
    fn tree_mut(&mut self) -> &mut ElementTree {
        self.tree
    }

    fn remove_subtree(&mut self, id: ElementId) {
        self.tree.element_remove(id);
    }

    fn apply_focus(&mut self, id: ElementId) {
        self.tree.on_focus(id);
    }

    fn apply_blur(&mut self, id: ElementId) {
        self.tree.on_blur(id);
    }
}

/// 中立化した `apply_mutations_batch`（ADR-0112）を ElementTree に適用する。
/// `ops` は Float64 レコード列、`styles` は style_packet の f32 列、`texts` は
/// OP_SET_TEXT 等が参照する文字列テーブル（JS の `string[]` 相当）。
pub(crate) fn apply_mutations(
    tree: &mut ElementTree,
    ops: &[f64],
    styles: &[f32],
    texts: &[String],
) -> Result<(), String> {
    let mut host = TreeApplyHost::new(tree);
    apply_mutations_batch(&mut host, ops, styles, texts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hayate_core::ElementTree;

    // 中立 dispatch を Android クレートから呼べることのスモークテスト。空バッチは
    // no-op で Ok を返す（境界の配線が通っていることの確認）。
    #[test]
    fn empty_batch_is_ok() {
        let mut tree = ElementTree::new();
        let texts: Vec<String> = Vec::new();
        assert!(apply_mutations(&mut tree, &[], &[], &texts).is_ok());
    }
}
