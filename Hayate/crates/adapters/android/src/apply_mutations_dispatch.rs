//! 中立化した `apply_mutations` dispatch を Canvas Mode の ElementTree へ適用する
//! シーム（ADR-0112）。Web 版（`crates/adapters/web/src/apply_mutations_dispatch.rs`）
//! と同じ生成 `dispatch.rs` を共有し、Tsubame↔Hayate の結合点を 1 つに保つ
//! （ADR-0055）。Web との違いは、文字列テーブルが `&[String]`・エラーが `String`
//! という中立シグネチャをそのまま使う点だけ。
use hayate_core::ElementTree;

pub(crate) trait ApplyMutationsHost {
    fn tree_mut(&mut self) -> &mut ElementTree;
    fn remove_subtree(&mut self, id: hayate_core::ElementId);
    fn apply_focus(&mut self, id: hayate_core::ElementId);
    fn apply_blur(&mut self, id: hayate_core::ElementId);
}

mod dispatch {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../proto/generated/dispatch.rs"
    ));
}

pub(crate) use dispatch::{apply_mutations_batch, unset_kind_from_u32};
