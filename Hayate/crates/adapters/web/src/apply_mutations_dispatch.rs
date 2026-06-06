//! Seam for applying parsed `apply_mutations` ops to the Canvas Mode ElementTree.
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
