//! Hayate wire プロトコルの decode と中立 `apply_mutations` dispatch（生成物。
//! ADR-0052 / ADR-0112）。
//!
//! Tsubame ↔ Hayate の wire 契約（op レコード・style packet・event エンコード）の
//! decode と、パース済み op を `ElementTree` へ適用する中立 dispatch を、Hayate 側で
//! 単一所有する（Hayate Protocol Contract: decode は Hayate 側が正本）。
//! `proto/generator` が `protocol.rs`（wire 定数・`Op` decode・style packet decode・
//! event-wire encode）と `dispatch.rs`（`&mut ElementTree` への適用。trait を介さない）を
//! 生成し、ここに include する。
//!
//! 各プラットフォームアダプタは本モジュールの公開 API（[`apply_mutations`] ほか）を
//! 使い、decode/dispatch を再実装も再 include もしない。これにより以前は Web/Android の
//! 両クレートへ複製コンパイルされていた中立 decode が core 一箇所に集約される。

use crate::element::tree::ElementTree;

/// wire 定数・`Op`/`StyleTag` decode・style packet decode・event-wire encode（生成物）。
pub mod protocol {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../proto/generated/protocol.rs"
    ));
}

/// パース済み op を `&mut ElementTree` へ適用する中立 dispatch（生成物）。
mod dispatch {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../proto/generated/dispatch.rs"
    ));
}

pub use dispatch::unset_kind_from_u32;
pub use protocol::*;

/// 中立 `apply_mutations`（ADR-0112）。wire ops（op レコード列・style packet の f32 列・
/// `OP_SET_TEXT` 等が参照する文字列テーブル）を decode して `ElementTree` に適用する。
/// Web（wasm 境界）と Android（埋め込み Hermes）の両アダプタが共有する唯一の適用経路。
pub fn apply_mutations(
    tree: &mut ElementTree,
    ops: &[f64],
    styles: &[f32],
    texts: &[String],
) -> Result<(), String> {
    dispatch::apply_mutations_batch(tree, ops, styles, texts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ElementTree;

    // 空バッチは no-op で Ok（境界の配線確認）。
    #[test]
    fn empty_batch_is_ok() {
        let mut tree = ElementTree::new();
        let texts: Vec<String> = Vec::new();
        assert!(apply_mutations(&mut tree, &[], &[], &texts).is_ok());
    }
}
