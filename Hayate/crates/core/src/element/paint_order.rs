use std::cell::RefCell;
use std::collections::HashMap;

use crate::element::id::ElementId;

#[derive(Debug, Default)]
struct ParentPaintOrder {
    children: Vec<ElementId>,
    dirty: bool,
    rebuild_count: usize,
}

/// Element Document Runtime が保持する parent 単位の Paint Order。
///
/// 保持形式・dirty 状態・再構築戦略を interface の裏へ閉じ込め、consumer には同じ
/// 借用 slice を渡す。読み取り seam は `&self` のままなので hit-test からも使える。
#[derive(Debug, Default)]
pub(crate) struct PaintOrder {
    parents: HashMap<ElementId, RefCell<ParentPaintOrder>>,
}

impl PaintOrder {
    pub(crate) fn register(&mut self, id: ElementId) {
        self.parents.entry(id).or_insert_with(|| {
            RefCell::new(ParentPaintOrder {
                dirty: true,
                ..ParentPaintOrder::default()
            })
        });
    }

    pub(crate) fn remove(&mut self, id: ElementId) {
        self.parents.remove(&id);
    }

    pub(crate) fn invalidate(&mut self, parent: ElementId) {
        if let Some(order) = self.parents.get_mut(&parent) {
            order.get_mut().dirty = true;
        }
    }

    pub(crate) fn with_order<R>(
        &self,
        parent: ElementId,
        document_children: &[ElementId],
        z_index: impl Fn(ElementId) -> i32,
        consume: impl FnOnce(&[ElementId]) -> R,
    ) -> R {
        let Some(order) = self.parents.get(&parent) else {
            return consume(&[]);
        };
        if order.borrow().dirty {
            let mut order = order.borrow_mut();
            order.children.clear();
            order.children.extend_from_slice(document_children);
            order.children.sort_by_key(|&child| z_index(child));
            order.dirty = false;
            order.rebuild_count += 1;
        }
        let order = order.borrow();
        consume(&order.children)
    }

    #[cfg(any(debug_assertions, feature = "scene-validation"))]
    pub(crate) fn rebuild_count(&self, parent: ElementId) -> usize {
        self.parents
            .get(&parent)
            .map_or(0, |order| order.borrow().rebuild_count)
    }
}
