use std::collections::{HashMap, HashSet};

use crate::element::id::ElementId;
use crate::element::inline_text::{is_ifc_root, is_inline_text_element};
use crate::element::kind::ElementKind;
use crate::element::style::StyleProp;
use crate::element::tree::{Element, ElementTree};

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

/// Build an [`ElementContext`] from a bare `elements` map — the same topology
/// read `ElementTree::element_context` performs, but available to callers that
/// hold only the element map (e.g. the Taffy projection reconcile, which has no
/// live `ElementTree`). The reach kernel stays pure over the resulting context.
pub(crate) fn element_context_in(
    elements: &HashMap<ElementId, Element>,
    id: ElementId,
) -> ElementContext {
    let el = elements.get(&id);
    let kind = el.map_or(ElementKind::View, |e| e.kind);
    let has_text_parent = el
        .and_then(|e| e.parent)
        .and_then(|p| elements.get(&p))
        .is_some_and(|p| p.kind == ElementKind::Text);
    ElementContext {
        kind,
        is_ifc_root: is_ifc_root(elements, id),
        has_text_parent,
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

/// Read-only topology a reach walk needs: the parent chain, each element's
/// invalidation context, and its z-ordered children. `ElementTree` implements
/// this over the live tree; [`ElementMapTopology`] implements it over a bare
/// element map for callers without a live tree. Keeping it a trait lets the
/// reach traversals below be unit-tested against a fake — the same way the
/// `Change → sinks` routing is tested against a `RecordingSink`.
pub(crate) trait ReachTopology {
    fn parent(&self, id: ElementId) -> Option<ElementId>;
    fn element_context(&self, id: ElementId) -> ElementContext;
    fn ordered_children(&self, id: ElementId) -> Vec<ElementId>;
}

impl ReachTopology for ElementTree {
    fn parent(&self, id: ElementId) -> Option<ElementId> {
        self.elements.get(&id).and_then(|el| el.parent)
    }
    fn element_context(&self, id: ElementId) -> ElementContext {
        ElementTree::element_context(self, id)
    }
    fn ordered_children(&self, id: ElementId) -> Vec<ElementId> {
        ElementTree::ordered_children(self, id)
    }
}

/// [`ReachTopology`] over a bare `elements` map. The Taffy projection reconcile
/// has no live `ElementTree`, only its element map, so it walks reach through
/// this. Children come back in document order (the projection routes only the
/// patch-root search, which reads the parent chain, never children).
pub(crate) struct ElementMapTopology<'a> {
    pub elements: &'a HashMap<ElementId, Element>,
}

impl ReachTopology for ElementMapTopology<'_> {
    fn parent(&self, id: ElementId) -> Option<ElementId> {
        self.elements.get(&id).and_then(|el| el.parent)
    }
    fn element_context(&self, id: ElementId) -> ElementContext {
        element_context_in(self.elements, id)
    }
    fn ordered_children(&self, id: ElementId) -> Vec<ElementId> {
        self.elements
            .get(&id)
            .map(|el| el.children.clone())
            .unwrap_or_default()
    }
}

/// The children a reach walk descends into from `id` under `reach`, each paired
/// with the reach it carries. Derived entirely from the single-source
/// [`step_reach`]: a child is descended into iff `step_reach` returns `Some`.
/// Both the retained scene walk's child descent route through here.
pub(crate) fn children_for_reach<T: ReachTopology>(
    topology: &T,
    id: ElementId,
    reach: VisualInvalidationReach,
) -> Vec<(ElementId, VisualInvalidationReach)> {
    let parent_ctx = topology.element_context(id);
    topology
        .ordered_children(id)
        .into_iter()
        .filter_map(|child| {
            step_reach(reach, parent_ctx, topology.element_context(child))
                .map(|child_reach| (child, child_reach))
        })
        .collect()
}

/// The minimal patch roots for a reach-tagged dirty set: every dirty element
/// that no dirty ancestor's reach would itself re-emit. The single source for
/// both the retained scene walk's partial re-lower and the Taffy projection's
/// structure reconcile — [`step_reach`] is the kernel deciding whether an
/// ancestor's reach actually propagates down to a given descendant.
pub(crate) fn minimal_patch_roots<T: ReachTopology>(
    topology: &T,
    dirty: &HashMap<ElementId, VisualInvalidationReach>,
) -> Vec<ElementId> {
    dirty
        .keys()
        .copied()
        .filter(|&id| !covered_by_dirty_ancestor(topology, id, dirty))
        .collect()
}

/// Whether re-walking some dirty ancestor will itself re-emit `id`, so `id` need
/// not be its own patch root. True only when the ancestor's reach actually
/// propagates down the ancestor→id path: a `SelfOnly` / `ZIndex` ancestor
/// re-emits only itself, so a dirty descendant under it (e.g. an independent
/// in-flight transition beneath a transitioning parent, issue #228) must remain
/// its own patch root or its re-lowering would be skipped.
fn covered_by_dirty_ancestor<T: ReachTopology>(
    topology: &T,
    id: ElementId,
    dirty: &HashMap<ElementId, VisualInvalidationReach>,
) -> bool {
    // Path id → root: chain[0] = id, chain[i+1] = parent(chain[i]).
    let mut chain = vec![id];
    let mut current = topology.parent(id);
    while let Some(parent) = current {
        chain.push(parent);
        current = topology.parent(parent);
    }
    // For each dirty ancestor, simulate the reach propagating down to `id` via
    // the single-source `step_reach`, exactly as the retained walk would.
    for (ancestor_idx, &ancestor) in chain.iter().enumerate().skip(1) {
        let Some(&ancestor_reach) = dirty.get(&ancestor) else {
            continue;
        };
        let mut reach = ancestor_reach;
        let mut parent = ancestor;
        let mut reached = true;
        for child_idx in (0..ancestor_idx).rev() {
            let child = chain[child_idx];
            match step_reach(
                reach,
                topology.element_context(parent),
                topology.element_context(child),
            ) {
                Some(next) => {
                    reach = next;
                    parent = child;
                }
                None => {
                    reached = false;
                    break;
                }
            }
        }
        if reached {
            return true;
        }
    }
    false
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

    /// A `ReachTopology` built from plain maps, so the consolidated reach
    /// traversals (`children_for_reach` / `minimal_patch_roots`) are exercised
    /// through their public interface without booting an `ElementTree` — the
    /// same testing posture the `RecordingSink` gives the routing seam.
    #[derive(Default)]
    struct FakeTopology {
        parents: HashMap<ElementId, ElementId>,
        contexts: HashMap<ElementId, ElementContext>,
        children: HashMap<ElementId, Vec<ElementId>>,
    }

    impl FakeTopology {
        fn add(&mut self, id: u64, parent: Option<u64>, context: ElementContext) {
            let eid = ElementId::from_u64(id);
            if let Some(p) = parent {
                let pid = ElementId::from_u64(p);
                self.parents.insert(eid, pid);
                self.children.entry(pid).or_default().push(eid);
            }
            self.contexts.insert(eid, context);
        }
    }

    impl ReachTopology for FakeTopology {
        fn parent(&self, id: ElementId) -> Option<ElementId> {
            self.parents.get(&id).copied()
        }
        fn element_context(&self, id: ElementId) -> ElementContext {
            self.contexts
                .get(&id)
                .copied()
                .unwrap_or_else(|| ctx(ElementKind::View, false, false))
        }
        fn ordered_children(&self, id: ElementId) -> Vec<ElementId> {
            self.children.get(&id).cloned().unwrap_or_default()
        }
    }

    fn dirty_map(
        entries: &[(u64, VisualInvalidationReach)],
    ) -> HashMap<ElementId, VisualInvalidationReach> {
        entries
            .iter()
            .map(|&(id, reach)| (ElementId::from_u64(id), reach))
            .collect()
    }

    fn id_set(ids: &[u64]) -> HashSet<ElementId> {
        ids.iter().map(|&id| ElementId::from_u64(id)).collect()
    }

    #[test]
    fn children_for_reach_subtree_descends_into_every_child() {
        let mut topo = FakeTopology::default();
        topo.add(1, None, ctx(ElementKind::View, false, false));
        topo.add(2, Some(1), ctx(ElementKind::View, false, false));
        topo.add(3, Some(1), ctx(ElementKind::View, false, false));

        let descended = children_for_reach(
            &topo,
            ElementId::from_u64(1),
            VisualInvalidationReach::Subtree,
        );
        assert_eq!(
            descended,
            vec![
                (ElementId::from_u64(2), VisualInvalidationReach::Subtree),
                (ElementId::from_u64(3), VisualInvalidationReach::Subtree),
            ]
        );
    }

    #[test]
    fn children_for_reach_text_local_descends_only_inline_text_under_ifc_root() {
        let mut topo = FakeTopology::default();
        topo.add(1, None, ctx(ElementKind::Text, true, false));
        topo.add(2, Some(1), ctx(ElementKind::Text, false, true)); // inline text
        topo.add(3, Some(1), ctx(ElementKind::View, false, false)); // block child

        let descended = children_for_reach(
            &topo,
            ElementId::from_u64(1),
            VisualInvalidationReach::TextLocal,
        );
        assert_eq!(
            descended,
            vec![(ElementId::from_u64(2), VisualInvalidationReach::TextLocal)]
        );
    }

    #[test]
    fn children_for_reach_self_only_descends_into_nothing() {
        let mut topo = FakeTopology::default();
        topo.add(1, None, ctx(ElementKind::View, false, false));
        topo.add(2, Some(1), ctx(ElementKind::View, false, false));

        let descended = children_for_reach(
            &topo,
            ElementId::from_u64(1),
            VisualInvalidationReach::SelfOnly,
        );
        assert!(descended.is_empty());
    }

    #[test]
    fn minimal_patch_roots_subtree_ancestor_covers_descendant() {
        // 1 → 2 → 3, both 1 and 3 dirty with Subtree reach: re-walking 1 will
        // itself re-emit 3, so 3 is not its own patch root.
        let mut topo = FakeTopology::default();
        topo.add(1, None, ctx(ElementKind::View, false, false));
        topo.add(2, Some(1), ctx(ElementKind::View, false, false));
        topo.add(3, Some(2), ctx(ElementKind::View, false, false));

        let dirty = dirty_map(&[
            (1, VisualInvalidationReach::Subtree),
            (3, VisualInvalidationReach::Subtree),
        ]);
        let roots: HashSet<ElementId> =
            minimal_patch_roots(&topo, &dirty).into_iter().collect();
        assert_eq!(roots, id_set(&[1]));
    }

    #[test]
    fn minimal_patch_roots_self_only_ancestor_does_not_cover_descendant() {
        // A `SelfOnly` ancestor re-emits only itself, so an independently dirty
        // descendant (issue #228) must remain its own patch root.
        let mut topo = FakeTopology::default();
        topo.add(1, None, ctx(ElementKind::View, false, false));
        topo.add(2, Some(1), ctx(ElementKind::View, false, false));
        topo.add(3, Some(2), ctx(ElementKind::View, false, false));

        let dirty = dirty_map(&[
            (1, VisualInvalidationReach::SelfOnly),
            (3, VisualInvalidationReach::Subtree),
        ]);
        let roots: HashSet<ElementId> =
            minimal_patch_roots(&topo, &dirty).into_iter().collect();
        assert_eq!(roots, id_set(&[1, 3]));
    }

    /// Presence-only ancestor check — the projection's old `has_dirty_ancestor`,
    /// kept here as the reference the `Subtree` specialization must still match.
    fn presence_only_roots(topo: &FakeTopology, dirty: &[u64]) -> HashSet<ElementId> {
        let dirty_ids = id_set(dirty);
        dirty_ids
            .iter()
            .copied()
            .filter(|&id| {
                let mut cur = topo.parent(id);
                while let Some(ancestor) = cur {
                    if dirty_ids.contains(&ancestor) {
                        return false;
                    }
                    cur = topo.parent(ancestor);
                }
                true
            })
            .collect()
    }

    #[test]
    fn minimal_patch_roots_all_subtree_equals_presence_only_specialization() {
        // The Taffy projection feeds `structure_dirty` tagged entirely `Subtree`.
        // `step_reach` always propagates `Subtree`, so routing through the shared
        // reach kernel must reproduce the old presence-only patch-root search
        // exactly — the projection's behavior is unchanged.
        let mut topo = FakeTopology::default();
        topo.add(1, None, ctx(ElementKind::View, false, false));
        topo.add(2, Some(1), ctx(ElementKind::Text, true, false)); // IFC root
        topo.add(3, Some(2), ctx(ElementKind::Text, false, true)); // inline text
        topo.add(4, Some(1), ctx(ElementKind::View, false, false));
        topo.add(5, Some(4), ctx(ElementKind::View, false, false));

        // A spread of shapes: nested chains, IFC boundaries, independent branches.
        for dirty in [
            vec![1u64],
            vec![2, 3],
            vec![1, 3, 5],
            vec![2, 4],
            vec![3, 5],
            vec![1, 2, 3, 4, 5],
        ] {
            let map = dirty_map(
                &dirty
                    .iter()
                    .map(|&id| (id, VisualInvalidationReach::Subtree))
                    .collect::<Vec<_>>(),
            );
            let reach_roots: HashSet<ElementId> =
                minimal_patch_roots(&topo, &map).into_iter().collect();
            assert_eq!(
                reach_roots,
                presence_only_roots(&topo, &dirty),
                "all-Subtree reach must match presence-only roots for {dirty:?}"
            );
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
