use std::collections::{HashMap, HashSet};

use crate::element::id::ElementId;
use crate::element::inline_text::{is_ifc_root, is_inline_text_element};
use crate::element::style::StyleProp;
use crate::element::tree::ElementTree;

/// How far scene re-lowering must reach for a visual-dirty element (issue #185).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum VisualInvalidationReach {
    /// Box visuals on the element only (background, border, opacity).
    SelfOnly,
    /// Self-only plus sibling reorder in the parent (`z-index`).
    ZIndex,
    /// Text descendants within the inline formatting context (ch1).
    TextLocal,
    /// Whole subtree (ch2 ambient `default-*` and other block-piercing changes).
    Subtree,
}

impl VisualInvalidationReach {
    pub(crate) fn merge(self, other: Self) -> Self {
        self.max(other)
    }
}

/// Classify a visual style property's invalidation reach.
pub(crate) fn invalidation_reach_for_prop(prop: &StyleProp) -> VisualInvalidationReach {
    match prop {
        StyleProp::BackgroundColor(_)
        | StyleProp::Opacity(_)
        | StyleProp::BorderRadius(_)
        | StyleProp::BorderWidth(_)
        | StyleProp::BorderColor(_) => VisualInvalidationReach::SelfOnly,
        StyleProp::ZIndex(_) => VisualInvalidationReach::ZIndex,
        StyleProp::Color(_) | StyleProp::FontStyle(_) | StyleProp::TextDecoration(_) => {
            VisualInvalidationReach::TextLocal
        }
        StyleProp::DefaultColor(_)
        | StyleProp::DefaultFontFamily(_)
        | StyleProp::DefaultFontSize(_)
        | StyleProp::DefaultFontWeight(_) => VisualInvalidationReach::Subtree,
        StyleProp::FontSize(_) | StyleProp::FontFamily(_) | StyleProp::FontWeight(_) => {
            VisualInvalidationReach::Subtree
        }
        _ => VisualInvalidationReach::Subtree,
    }
}

/// Widest reach among non-layout props (defaults to self-only when empty).
pub(crate) fn invalidation_reach_for_props(props: &[StyleProp]) -> VisualInvalidationReach {
    props
        .iter()
        .filter(|p| !p.is_layout())
        .map(invalidation_reach_for_prop)
        .max()
        .unwrap_or(VisualInvalidationReach::SelfOnly)
}

pub(crate) fn merge_reach(
    map: &mut HashMap<ElementId, VisualInvalidationReach>,
    id: ElementId,
    reach: VisualInvalidationReach,
) {
    map.entry(id)
        .and_modify(|existing| *existing = existing.merge(reach))
        .or_insert(reach);
}

/// Parents whose element-anchor children must be reordered after a `z-index` change.
pub(crate) fn z_index_reorder_parent(
    tree: &ElementTree,
    id: ElementId,
) -> Option<ElementId> {
    tree.elements.get(&id).and_then(|el| el.parent)
}

/// Insert `id` and any text-local descendants that must be re-lowered.
pub(crate) fn expand_text_local(
    tree: &ElementTree,
    id: ElementId,
    out: &mut HashMap<ElementId, VisualInvalidationReach>,
) {
    merge_reach(out, id, VisualInvalidationReach::TextLocal);
    if is_ifc_root(&tree.elements, id) {
        if let Some(el) = tree.elements.get(&id) {
            for &child in &el.children {
                if is_inline_text_element(&tree.elements, child) {
                    merge_reach(out, child, VisualInvalidationReach::TextLocal);
                }
            }
        }
    }
}

pub(crate) fn expand_subtree(
    tree: &ElementTree,
    root: ElementId,
    out: &mut HashMap<ElementId, VisualInvalidationReach>,
) {
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        merge_reach(out, id, VisualInvalidationReach::Subtree);
        if let Some(el) = tree.elements.get(&id) {
            stack.extend(el.children.iter().copied());
        }
    }
}

pub(crate) fn apply_visual_invalidation(
    tree: &ElementTree,
    id: ElementId,
    reach: VisualInvalidationReach,
    elements: &mut HashMap<ElementId, VisualInvalidationReach>,
    z_index_parents: &mut HashSet<ElementId>,
) {
    match reach {
        VisualInvalidationReach::SelfOnly => {
            merge_reach(elements, id, reach);
        }
        VisualInvalidationReach::ZIndex => {
            merge_reach(elements, id, reach);
            if let Some(parent) = z_index_reorder_parent(tree, id) {
                z_index_parents.insert(parent);
            }
        }
        VisualInvalidationReach::TextLocal => {
            expand_text_local(tree, id, elements);
        }
        VisualInvalidationReach::Subtree => {
            expand_subtree(tree, id, elements);
        }
    }
}
