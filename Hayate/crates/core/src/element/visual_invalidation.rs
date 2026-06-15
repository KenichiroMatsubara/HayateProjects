use std::collections::{HashMap, HashSet};

use crate::element::id::ElementId;
use crate::element::inline_text::{is_ifc_root, is_inline_text_element};
use crate::element::kind::ElementKind;
use crate::element::style::StyleProp;
use crate::element::tree::ElementTree;

/// Topological context of one element — everything `classify` / `step_reach`
/// need to know about an element's position without booting an `ElementTree`.
/// `tree.rs` reads the live topology and builds this; the invalidation
/// semantics here stay pure functions over `(prop, ctx)` / `(reach, ctx)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ElementContext {
    pub kind: ElementKind,
    /// `text` element that roots an inline formatting context (ADR-0063).
    pub is_ifc_root: bool,
    /// The element's parent is a `text` element.
    pub has_text_parent: bool,
}

impl ElementContext {
    /// `text` element nested directly under another `text` — no Taffy box.
    pub(crate) fn is_inline_text(self) -> bool {
        self.kind == ElementKind::Text && self.has_text_parent
    }
}

/// Which dirty set a change feeds. Ordered so `merge` keeps the widest concern.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum DirtyKind {
    /// Scene-only visual change → `visual_dirty`.
    Visual,
    /// IFC re-compose (text shaping) → `shape_dirty` + projection mark.
    Shape,
    /// Tree structure changed → `structure_dirty`.
    Structure,
}

impl DirtyKind {
    fn merge(self, other: Self) -> Self {
        self.max(other)
    }
}

/// What a single mutation means for invalidation: which dirty set, how far.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Change {
    pub dirty_kind: DirtyKind,
    pub reach: VisualInvalidationReach,
}

impl Change {
    pub(crate) fn merge(self, other: Self) -> Self {
        Change {
            dirty_kind: self.dirty_kind.merge(other.dirty_kind),
            reach: self.reach.merge(other.reach),
        }
    }

    /// The fallback for a style change with no classifiable visual props
    /// (e.g. an empty prop list): a scene-only self repaint.
    pub(crate) fn visual_self_only() -> Self {
        Change {
            dirty_kind: DirtyKind::Visual,
            reach: VisualInvalidationReach::SelfOnly,
        }
    }
}

/// Whether changing `prop` invalidates the shaped text of its IFC — mirrors the
/// `text_dirty` cases of `tree::apply_visual` (font metrics, color baked into
/// the Parley run brush, and line-fitting limits).
pub(crate) fn prop_affects_text_shaping(prop: &StyleProp) -> bool {
    matches!(
        prop,
        StyleProp::MaxLines(_)
            | StyleProp::TextOverflow(_)
            | StyleProp::FontSize(_)
            | StyleProp::FontFamily(_)
            | StyleProp::FontWeight(_)
            | StyleProp::Color(_)
            | StyleProp::FontStyle(_)
            | StyleProp::TextDecoration(_)
    )
}

/// The dirty sets a routed `Change` can reach. The live `ElementEngine` +
/// `TaffyProjection` pair implements this in `tree.rs`; a recording fake in the
/// tests lets the `Change → marked sets` table be verified without booting an
/// `ElementTree` (ADR-0099).
pub(crate) trait DirtySink {
    fn mark_visual(&mut self, id: ElementId, reach: VisualInvalidationReach);
    fn mark_shape(&mut self, id: ElementId, reach: VisualInvalidationReach);
    fn mark_structure(&mut self, id: ElementId);
    fn mark_geometry(&mut self, id: ElementId);
}

/// The single visual-invalidation routing seam (ADR-0099): deliver a classified
/// `Change` to every dirty set it must reach, atomically. The whole
/// correspondence table lives here alone — callers no longer hand-wire which
/// `engine.mark_*` / `projection.mark_dirty` calls go together, so a shape change
/// can never reach the engine without also marking projection geometry.
///
/// The *which element* (e.g. resolving the enclosing IFC root for a shape change)
/// is topology and stays caller-side (#238); this only maps `dirty_kind → sinks`.
pub(crate) fn route_change<S: DirtySink>(sink: &mut S, id: ElementId, change: Change) {
    match change.dirty_kind {
        DirtyKind::Visual => sink.mark_visual(id, change.reach),
        DirtyKind::Shape => {
            sink.mark_shape(id, change.reach);
            sink.mark_geometry(id);
        }
        DirtyKind::Structure => sink.mark_structure(id),
    }
}

/// Classify a style property change against its element context (the *what*:
/// dirty kind + reach). The *which* element to mark (e.g. resolving the
/// enclosing IFC root for a shape change) stays in `tree.rs`.
pub(crate) fn classify(prop: &StyleProp, _ctx: ElementContext) -> Change {
    let dirty_kind = if prop_affects_text_shaping(prop) {
        DirtyKind::Shape
    } else {
        DirtyKind::Visual
    };
    Change {
        dirty_kind,
        reach: invalidation_reach_for_prop(prop),
    }
}

/// Classify a child attach/detach. Topology decides whether appending into an
/// inline formatting context re-shapes the IFC (`text` child under an IFC root)
/// or just reconciles the structure projection.
pub(crate) fn classify_attachment(
    parent_ctx: ElementContext,
    child_ctx: ElementContext,
) -> Change {
    if parent_ctx.is_ifc_root && child_ctx.kind == ElementKind::Text {
        Change {
            dirty_kind: DirtyKind::Shape,
            reach: VisualInvalidationReach::Subtree,
        }
    } else {
        Change {
            dirty_kind: DirtyKind::Structure,
            reach: VisualInvalidationReach::Subtree,
        }
    }
}

/// Single source for reach propagation: the reach a walk carries from
/// `parent_ctx` into `child_ctx` under `reach`, or `None` when the walk does
/// not descend into that child. Both the retained scene walk and the
/// patch-root search route through here.
pub(crate) fn step_reach(
    reach: VisualInvalidationReach,
    parent_ctx: ElementContext,
    child_ctx: ElementContext,
) -> Option<VisualInvalidationReach> {
    match reach {
        VisualInvalidationReach::Subtree => Some(VisualInvalidationReach::Subtree),
        VisualInvalidationReach::TextLocal
            if parent_ctx.is_ifc_root && child_ctx.is_inline_text() =>
        {
            Some(VisualInvalidationReach::TextLocal)
        }
        _ => None,
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;
    use crate::element::kind::ElementKind;

    fn ctx(kind: ElementKind, is_ifc_root: bool, has_text_parent: bool) -> ElementContext {
        ElementContext {
            kind,
            is_ifc_root,
            has_text_parent,
        }
    }

    /// Records every dirty-set hit so the `Change → marked sets` table can be
    /// asserted without booting an `ElementTree` (ADR-0099).
    #[derive(Default)]
    struct RecordingSink {
        calls: Vec<SinkCall>,
    }

    #[derive(Debug, PartialEq, Eq)]
    enum SinkCall {
        Visual(ElementId, VisualInvalidationReach),
        Shape(ElementId, VisualInvalidationReach),
        Structure(ElementId),
        Geometry(ElementId),
    }

    impl DirtySink for RecordingSink {
        fn mark_visual(&mut self, id: ElementId, reach: VisualInvalidationReach) {
            self.calls.push(SinkCall::Visual(id, reach));
        }
        fn mark_shape(&mut self, id: ElementId, reach: VisualInvalidationReach) {
            self.calls.push(SinkCall::Shape(id, reach));
        }
        fn mark_structure(&mut self, id: ElementId) {
            self.calls.push(SinkCall::Structure(id));
        }
        fn mark_geometry(&mut self, id: ElementId) {
            self.calls.push(SinkCall::Geometry(id));
        }
    }

    #[test]
    fn route_shape_atomically_marks_shape_and_geometry() {
        let id = ElementId::from_u64(7);
        let mut sink = RecordingSink::default();
        route_change(
            &mut sink,
            id,
            Change {
                dirty_kind: DirtyKind::Shape,
                reach: VisualInvalidationReach::TextLocal,
            },
        );
        assert_eq!(
            sink.calls,
            vec![
                SinkCall::Shape(id, VisualInvalidationReach::TextLocal),
                SinkCall::Geometry(id),
            ]
        );
    }

    #[test]
    fn route_visual_marks_visual_only() {
        let id = ElementId::from_u64(3);
        let mut sink = RecordingSink::default();
        route_change(
            &mut sink,
            id,
            Change {
                dirty_kind: DirtyKind::Visual,
                reach: VisualInvalidationReach::SelfOnly,
            },
        );
        assert_eq!(
            sink.calls,
            vec![SinkCall::Visual(id, VisualInvalidationReach::SelfOnly)]
        );
    }

    #[test]
    fn route_structure_marks_structure_only() {
        let id = ElementId::from_u64(5);
        let mut sink = RecordingSink::default();
        route_change(
            &mut sink,
            id,
            Change {
                dirty_kind: DirtyKind::Structure,
                reach: VisualInvalidationReach::Subtree,
            },
        );
        // Structure never touches the engine's visual/shape sets or projection
        // geometry directly — reconcile expands the subtree from the seed.
        assert_eq!(sink.calls, vec![SinkCall::Structure(id)]);
    }

    #[test]
    fn classify_font_size_reaches_subtree() {
        let c = classify(
            &StyleProp::FontSize(20.0),
            ctx(ElementKind::Text, true, false),
        );
        assert_eq!(c.reach, VisualInvalidationReach::Subtree);
        assert_eq!(c.dirty_kind, DirtyKind::Shape);
    }

    #[test]
    fn classify_background_is_self_only_visual() {
        let c = classify(
            &StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
            ctx(ElementKind::View, false, false),
        );
        assert_eq!(c.dirty_kind, DirtyKind::Visual);
        assert_eq!(c.reach, VisualInvalidationReach::SelfOnly);
    }

    #[test]
    fn classify_color_is_text_local_shape() {
        // Color is baked into the Parley run brush, so it re-shapes, but its
        // reach is confined to the inline formatting context.
        let c = classify(
            &StyleProp::Color(Color::new(0.0, 1.0, 0.0, 1.0)),
            ctx(ElementKind::Text, true, false),
        );
        assert_eq!(c.dirty_kind, DirtyKind::Shape);
        assert_eq!(c.reach, VisualInvalidationReach::TextLocal);
    }

    #[test]
    fn classify_z_index_is_visual_zindex_reach() {
        let c = classify(
            &StyleProp::ZIndex(3),
            ctx(ElementKind::View, false, false),
        );
        assert_eq!(c.dirty_kind, DirtyKind::Visual);
        assert_eq!(c.reach, VisualInvalidationReach::ZIndex);
    }

    #[test]
    fn classify_default_color_pierces_whole_subtree() {
        let c = classify(
            &StyleProp::DefaultColor(Color::new(1.0, 0.0, 0.0, 1.0)),
            ctx(ElementKind::View, false, false),
        );
        assert_eq!(c.reach, VisualInvalidationReach::Subtree);
    }

    #[test]
    fn classify_merge_takes_widest_concern_and_reach() {
        let visual = classify(
            &StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
            ctx(ElementKind::Text, true, false),
        );
        let shape = classify(
            &StyleProp::FontSize(18.0),
            ctx(ElementKind::Text, true, false),
        );
        let merged = visual.merge(shape);
        assert_eq!(merged.dirty_kind, DirtyKind::Shape);
        assert_eq!(merged.reach, VisualInvalidationReach::Subtree);
    }

    #[test]
    fn attachment_into_ifc_root_reshapes() {
        let parent = ctx(ElementKind::Text, true, false);
        let child = ctx(ElementKind::Text, false, true);
        assert_eq!(
            classify_attachment(parent, child).dirty_kind,
            DirtyKind::Shape
        );
    }

    #[test]
    fn attachment_into_plain_parent_restructures() {
        let parent = ctx(ElementKind::View, false, false);
        let child = ctx(ElementKind::View, false, false);
        assert_eq!(
            classify_attachment(parent, child).dirty_kind,
            DirtyKind::Structure
        );
    }

    #[test]
    fn step_reach_subtree_always_descends_subtree() {
        let parent = ctx(ElementKind::View, false, false);
        let child = ctx(ElementKind::View, false, false);
        assert_eq!(
            step_reach(VisualInvalidationReach::Subtree, parent, child),
            Some(VisualInvalidationReach::Subtree)
        );
    }

    #[test]
    fn step_reach_text_local_descends_only_ifc_root_to_inline_text() {
        let ifc_root = ctx(ElementKind::Text, true, false);
        let inline = ctx(ElementKind::Text, false, true);
        let block = ctx(ElementKind::View, false, false);
        // IFC root → inline text descendant: propagate.
        assert_eq!(
            step_reach(VisualInvalidationReach::TextLocal, ifc_root, inline),
            Some(VisualInvalidationReach::TextLocal)
        );
        // IFC root → non-inline child: does not descend.
        assert_eq!(
            step_reach(VisualInvalidationReach::TextLocal, ifc_root, block),
            None
        );
        // Inline text (not an IFC root) → child: stops here.
        assert_eq!(
            step_reach(VisualInvalidationReach::TextLocal, inline, inline),
            None
        );
    }

    #[test]
    fn step_reach_self_only_and_z_index_never_descend() {
        let parent = ctx(ElementKind::Text, true, false);
        let child = ctx(ElementKind::Text, false, true);
        assert_eq!(
            step_reach(VisualInvalidationReach::SelfOnly, parent, child),
            None
        );
        assert_eq!(
            step_reach(VisualInvalidationReach::ZIndex, parent, child),
            None
        );
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
