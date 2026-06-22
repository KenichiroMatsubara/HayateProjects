//! 埋め込み Hermes（ADR-0112）が呼ぶ `apply_mutations` を ElementTree へ適用する
//! Rust 側ブリッジ。JSI/C++ ホストは flat-C ABI 越しにここへ降りてくる。
//!
//! 中立な wire decode/dispatch は core が単一所有する（[`hayate_core::wire`] /
//! Hayate Protocol Contract）。本モジュールは wasm に一切依存せず、Web と同じ適用経路
//! （core の `apply_mutations`）を呼ぶ薄い委譲に徹する。C++/Hermes 配線はデバイス
//! ビルド側にあるため、ここはホストで `cargo check` / 単体テスト可能な純 Rust に保つ。
use hayate_core::ElementTree;

/// core の中立 `apply_mutations`（ADR-0112）を ElementTree に適用する。`ops` は
/// Float64 レコード列、`styles` は style_packet の f32 列、`texts` は `OP_SET_TEXT` 等が
/// 参照する文字列テーブル（JS の `string[]` 相当）。
pub(crate) fn apply_mutations(
    tree: &mut ElementTree,
    ops: &[f64],
    styles: &[f32],
    texts: &[String],
) -> Result<(), String> {
    hayate_core::wire::apply_mutations(tree, ops, styles, texts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hayate_core::ElementTree;

    // core 経由の中立 dispatch を Android クレートから呼べることのスモークテスト。
    // 空バッチは no-op で Ok を返す（境界の配線確認）。
    #[test]
    fn empty_batch_is_ok() {
        let mut tree = ElementTree::new();
        let texts: Vec<String> = Vec::new();
        assert!(apply_mutations(&mut tree, &[], &[], &texts).is_ok());
    }
}
