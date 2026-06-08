use std::collections::{HashMap, HashSet};

use taffy::{NodeId, TaffyTree};

use crate::element::id::ElementId;
use crate::element::kind::ElementKind;
use crate::element::taffy_bridge::MeasureCtx;
use crate::element::tree::Element;

/// Derived Taffy layout tree for the block-box subset of an `ElementTree`.
///
/// Inline text elements (text whose parent is also text) are excluded per ADR-0063/0064.
/// IFC roots (`text` under non-text parent) are measured leaves.
pub(crate) struct TaffyProjection {
    pub(crate) taffy: TaffyTree<MeasureCtx>,
    element_to_node: HashMap<ElementId, NodeId>,
    structure_dirty: HashSet<ElementId>,
    built: bool,
}

impl TaffyProjection {
    pub fn new() -> Self {
        Self {
            taffy: TaffyTree::new(),
            element_to_node: HashMap::new(),
            structure_dirty: HashSet::new(),
            built: false,
        }
    }

    pub fn mark_structure_dirty(&mut self, id: ElementId) {
        self.structure_dirty.insert(id);
        self.built = false;
    }

    pub fn mark_dirty(&mut self, id: ElementId) {
        if let Some(node) = self.element_to_node.get(&id) {
            let _ = self.taffy.mark_dirty(*node);
        }
    }

    pub fn set_style(&mut self, id: ElementId, style: taffy::Style) {
        if let Some(node) = self.element_to_node.get(&id) {
            let _ = self.taffy.set_style(*node, style);
        }
    }

    /// Rebuild the Taffy projection when structure has changed.
    pub fn reconcile(&mut self, elements: &mut HashMap<ElementId, Element>, root: ElementId) {
        if self.built && self.structure_dirty.is_empty() {
            return;
        }
        self.structure_dirty.clear();
        self.taffy = TaffyTree::new();
        self.element_to_node.clear();

        if elements.contains_key(&root) {
            build_subtree(self, elements, root);
        }
        self.built = true;
    }
}

impl Default for TaffyProjection {
    fn default() -> Self {
        Self::new()
    }
}

/// `text` element whose parent is also `text` — no Taffy box (ADR-0063).
pub(crate) fn is_inline_text_element(
    elements: &HashMap<ElementId, Element>,
    id: ElementId,
) -> bool {
    let el = match elements.get(&id) {
        Some(e) => e,
        None => return false,
    };
    if el.kind != ElementKind::Text {
        return false;
    }
    el.parent
        .and_then(|p| elements.get(&p))
        .is_some_and(|p| p.kind == ElementKind::Text)
}

fn is_ifc_root(elements: &HashMap<ElementId, Element>, id: ElementId) -> bool {
    elements
        .get(&id)
        .is_some_and(|el| el.kind == ElementKind::Text && !is_inline_text_element(elements, id))
}

fn build_subtree(
    projection: &mut TaffyProjection,
    elements: &mut HashMap<ElementId, Element>,
    id: ElementId,
) -> Option<NodeId> {
    if is_inline_text_element(elements, id) {
        if let Some(el) = elements.get_mut(&id) {
            el.taffy_node = None;
        }
        let children = elements
            .get(&id)
            .map(|el| el.children.clone())
            .unwrap_or_default();
        for child in children {
            build_subtree(projection, elements, child);
        }
        return None;
    }

    let (layout_style, measure_ctx) = {
        let el = elements.get(&id)?;
        let measure_ctx = if is_ifc_root(elements, id) {
            MeasureCtx::Text(id)
        } else {
            MeasureCtx::None
        };
        (el.layout_style.clone(), measure_ctx)
    };

    let node = projection
        .taffy
        .new_leaf_with_context(layout_style, measure_ctx)
        .expect("taffy new_leaf_with_context");
    if let Some(el) = elements.get_mut(&id) {
        el.taffy_node = Some(node);
    }
    projection.element_to_node.insert(id, node);

    let children = elements
        .get(&id)
        .map(|el| el.children.clone())
        .unwrap_or_default();
    for child in children {
        if let Some(child_node) = build_subtree(projection, elements, child) {
            let _ = projection.taffy.add_child(node, child_node);
        }
    }

    Some(node)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::tree::Visual;

    fn make_text(id: u64, parent: Option<ElementId>, text: &str) -> (ElementId, Element) {
        let eid = ElementId::from_u64(id);
        let el = Element {
            kind: ElementKind::Text,
            parent,
            children: Vec::new(),
            taffy_node: None,
            layout_style: taffy::Style::default(),
            visual: Visual::default(),
            text: Some(text.to_string()),
            src: None,
            text_layout: None,
            transform: None,
            scroll_offset: (0.0, 0.0),
            src_image: None,
            text_content: String::new(),
            preedit: None,
            cursor_byte_index: 0,
            cursor_visible: false,
            content_layout: None,
            aria_label: None,
            role: None,
            pseudo_styles: Default::default(),
        };
        (eid, el)
    }

    #[test]
    fn reconcile_excludes_inline_text_from_projection_map() {
        let mut projection = TaffyProjection::new();
        let mut elements = HashMap::new();

        let (root_id, root) = make_text(1, None, "outer");
        let (inline_id, inline) = make_text(2, Some(root_id), "inner");
        elements.insert(root_id, root);
        elements.insert(inline_id, inline);
        elements
            .get_mut(&root_id)
            .unwrap()
            .children
            .push(inline_id);

        projection.reconcile(&mut elements, root_id);

        assert!(elements[&inline_id].taffy_node.is_none());
        assert!(elements[&root_id].taffy_node.is_some());
    }
}
