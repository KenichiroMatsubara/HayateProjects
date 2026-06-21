use crate::color::Color;
use crate::element::effective_visual::{
    self, child_inherited_context, InheritedVisualContext,
};
use crate::element::style::{BorderStyleValue, OverflowValue, Shadow};
use crate::element::id::ElementId;
use crate::element::kind::ElementKind;
use crate::element::pointer::PointerKind;
use crate::element::scene_lowering::{
    clear_lowered_content, AnchorEntry, LoweringDirtySnapshot, SceneLowering,
};
use crate::element::visual_invalidation::{self, VisualInvalidationReach};
use crate::element::taffy_projection::TraversalStep;
use crate::element::tree::{ElementTree, Visual};
use crate::node::{Node, NodeId, NodeKind, SceneGraph};
use std::collections::HashSet;

/// Native focus-ring geometry and colour for `:focus-visible` (#335, ADR-0102).
/// Chromium draws a solid ring just outside the border box, following its corner
/// radius. The width/offset/colour approximate Chrome's default `outline: auto`
/// ring and are calibrated against real Chromium rasterisation (tracking #335).
pub const FOCUS_RING_WIDTH: f32 = 2.0;
/// Gap between the element's border box and the ring's inner edge.
pub const FOCUS_RING_OFFSET: f32 = 1.0;
/// Chromium's default accent focus ring (Google Blue), opaque.
pub const FOCUS_RING_COLOR: Color = Color::new(0.102, 0.451, 0.910, 1.0);

/// Scrollbar overlay chrome (ADR-0110, #407). A Mouse/Pen-style always-on thumb
/// painted over the content on each *overflowing* axis of a `ScrollView`; the
/// thumb geometry is derived from the Scroll Offset and content size. Drawn as an
/// overlay — no layout space is reserved (no scrollbar gutter), so it never
/// shrinks the content box. The Touch transient indicator and the drag/track
/// interaction are later slices (ADR-0110).
///
/// Every tunable here is a named placeholder pending Chromium calibration, in the
/// same bucket as the focus ring and selection-chrome values (ADR-0102): the
/// scene-build path must carry no inline scrollbar magic numbers.
///
/// Thickness (cross-axis extent) of the scrollbar bar.
pub const SCROLLBAR_THICKNESS: f32 = 6.0;
/// Inset of the track (hence the thumb) from the scroll-view box edges.
pub const SCROLLBAR_TRACK_MARGIN: f32 = 2.0;
/// Floor on the thumb's length along the scroll axis, so very tall/wide content
/// still leaves a grabbable thumb instead of collapsing to a sliver.
pub const SCROLLBAR_MIN_THUMB_LENGTH: f32 = 24.0;
/// Thumb fill colour (RGB); composited over the content at [`SCROLLBAR_THUMB_OPACITY`].
pub const SCROLLBAR_THUMB_COLOR: Color = Color::new(0.0, 0.0, 0.0, 1.0);
/// Thumb opacity — its translucency as an overlay sitting on top of the content.
pub const SCROLLBAR_THUMB_OPACITY: f32 = 0.4;
/// Scroll Offset distance one track-margin click advances ("page" step, #409).
/// A placeholder value pending Chromium calibration (ADR-0110); a true page step
/// keys off the viewport length, which is a follow-up — like the other
/// `SCROLLBAR_*` values this carries no inline magic number.
pub const SCROLLBAR_PAGE_STEP: f32 = 240.0;

/// Touch transient-indicator dimensions and fade timing (ADR-0110, SCR-04, #410).
/// The Touch form is a non-operable indicator that shows while the content
/// scrolls and fades after it stops (Android-native, ADR-0087); it is thinner
/// than the Mouse/Pen operable thumb and carries no hit region. Every value here
/// is a named placeholder pending Android calibration, in the same bucket as the
/// other `SCROLLBAR_*` constants — the scene-build path holds no inline magic
/// number.
///
/// Cross-axis extent of the indicator bar (thinner than [`SCROLLBAR_THICKNESS`]).
pub const SCROLLBAR_INDICATOR_THICKNESS: f32 = 4.0;
/// Indicator fill colour (RGB); composited at [`SCROLLBAR_INDICATOR_OPACITY`]
/// scaled by the current fade factor.
pub const SCROLLBAR_INDICATOR_COLOR: Color = Color::BLACK;
/// Indicator opacity at full visibility (before any fade).
pub const SCROLLBAR_INDICATOR_OPACITY: f32 = 0.4;
/// How long the indicator stays fully visible after the last scroll before it
/// begins to fade (the "hold" window).
pub const SCROLLBAR_INDICATOR_HOLD_MS: f64 = 600.0;
/// How long the indicator takes to fade from full to invisible once the hold
/// window elapses (the "fade" length).
pub const SCROLLBAR_INDICATOR_FADE_MS: f64 = 400.0;

/// Axis a scrollbar thumb slides along (#409).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollAxis {
    Vertical,
    Horizontal,
}

/// Canvas-space geometry of one overflowing axis's Mouse/Pen scrollbar, derived
/// from the box rect, Scroll Offset and content size (ADR-0110, #409). The
/// single source the overlay paint (`emit_scrollbar_overlay`) and the pointer
/// hit-test (`interaction.rs`) share, so a press lands exactly on the thumb the
/// user sees and operation maps back to the same offset the paint reads.
#[derive(Clone, Copy, Debug)]
pub struct ScrollbarAxisGeometry {
    pub axis: ScrollAxis,
    /// Thumb rect `(x, y, w, h)` in canvas coordinates.
    pub thumb: (f32, f32, f32, f32),
    /// Track rect `(x, y, w, h)` in canvas coordinates — the full slidable span.
    pub track: (f32, f32, f32, f32),
    /// Maximum Scroll Offset on this axis.
    pub max_offset: f32,
    /// Slidable thumb travel in track px (`track_len − thumb_len`); zero when the
    /// thumb fills the track. Maps a drag's track-pixel delta to an offset delta.
    pub thumb_travel: f32,
}

/// Scrollbar geometry for each overflowing axis of `id`, in canvas coords. The
/// public seam the pointer hit-test reads (`interaction.rs`); empty for a
/// non-`ScrollView`, an unlaid-out element, or one whose content fits. Computed
/// from the element's own layout rect so it agrees with the overlay paint.
pub fn scrollbar_axes(tree: &ElementTree, id: ElementId) -> Vec<ScrollbarAxisGeometry> {
    if tree.element_kind(id) != Some(ElementKind::ScrollView) {
        return Vec::new();
    }
    let Some((x, y, w, h)) = tree.element_layout_rect(id) else {
        return Vec::new();
    };
    scrollbar_axes_in_box(tree, id, x, y, w, h)
}

/// Ambient context threaded through the one scene walk shared by both anchor
/// strategies (issue #322). Carries what every emission needs regardless of
/// strategy — the document tree, the scene graph it builds into, the interaction
/// snapshot driving effective-visual resolution (ADR-0067), and the per-node
/// cursor of absolute origin + inherited context. The strategy-specific state
/// (anchors/clock for retained, nothing for ephemeral) lives in the
/// [`AnchorSink`] threaded alongside, not here. Descending into a child is
/// [`WalkCtx::child`].
struct WalkCtx<'a> {
    tree: &'a ElementTree,
    interaction: &'a crate::element::pseudo_state::InteractionSnapshot,
    sg: &'a mut SceneGraph,
    /// Absolute origin (parent box top-left) the child is laid out against.
    ox: f32,
    oy: f32,
    inherited: InheritedVisualContext,
}

impl WalkCtx<'_> {
    /// Reborrow for descending into a child: the ambient fields (tree, scene
    /// graph, interaction) carry over unchanged; the cursor (origin, inherited
    /// context) is replaced.
    fn child(&mut self, ox: f32, oy: f32, inherited: InheritedVisualContext) -> WalkCtx<'_> {
        WalkCtx {
            tree: self.tree,
            interaction: self.interaction,
            sg: &mut *self.sg,
            ox,
            oy,
            inherited,
        }
    }
}

/// How the shared scene walk attaches an element's emitted content (issue #322).
///
/// The emission body — transform/clip wrappers, box shadows, the visual box, and
/// image/text/text-input runs — is identical for the retained incremental
/// lowering (ADR-0086) and the ephemeral full rebuild that backstops golden-frame
/// parity (ADR-0079). They differ only in *anchoring*: retained re-attaches a
/// persistent `ElementAnchor` and interpolates in-flight transitions against the
/// value it remembers (ADR-0093); ephemeral emits fresh nodes under the parent
/// group and paints the resolved target directly. Each is one adapter of this
/// seam ([`RetainedSink`] / [`EphemeralSink`]), so an emission fix lands once.
trait AnchorSink {
    /// Per-node cursor this strategy threads down the walk: the retained attach
    /// parent + re-lowering `reach`; just the parent group for ephemeral.
    type Cursor: Copy;

    /// Called once per visited node (including skipped/None ones) before any work
    /// — the retained walk-count seam (ADR-0086 "clean frame ⇒ zero walks").
    fn enter_node(&mut self);

    /// Establish the scene node element `id`'s own content emits under (the
    /// `effective_parent` seed). Retained ensures the persistent anchor and clears
    /// its prior content; ephemeral forwards the parent group from the cursor.
    fn begin(&mut self, ctx: &mut WalkCtx, cursor: Self::Cursor, id: ElementId) -> Option<NodeId>;

    /// The visual actually painted. Retained interpolates `resolved` against the
    /// anchor's remembered displayed value; ephemeral paints `resolved` directly.
    fn displayed(&mut self, id: ElementId, resolved: Visual) -> Visual;

    /// The children to recurse and their per-child cursors, attaching under
    /// `effective_parent`. Retained narrows by `reach`; ephemeral takes all
    /// ordered children.
    fn children(
        &self,
        tree: &ElementTree,
        cursor: Self::Cursor,
        id: ElementId,
        effective_parent: Option<NodeId>,
    ) -> Vec<(ElementId, Self::Cursor)>;

    /// Settle child placement after this element's content and children are
    /// emitted. Retained re-stacks child anchors after the content; ephemeral is a
    /// no-op (fresh nodes are already in paint order).
    fn end_element(&mut self, ctx: &mut WalkCtx, effective_parent: Option<NodeId>, id: ElementId);
}

/// Per-node cursor for [`RetainedSink`]: where this element attaches and how far
/// the re-lowering reach still propagates.
#[derive(Clone, Copy)]
struct RetainedCursor {
    parent_anchor: Option<NodeId>,
    reach: VisualInvalidationReach,
}

/// Retained incremental lowering adapter (ADR-0086): persistent `ElementAnchor`
/// re-attachment + transition interpolation against the anchor's stored displayed
/// value (ADR-0093).
struct RetainedSink<'a> {
    lowering: &'a mut SceneLowering,
    now_ms: f64,
}

impl AnchorSink for RetainedSink<'_> {
    type Cursor = RetainedCursor;

    fn enter_node(&mut self) {
        self.lowering.walk_count += 1;
    }

    fn begin(&mut self, ctx: &mut WalkCtx, cursor: RetainedCursor, id: ElementId) -> Option<NodeId> {
        let tree = ctx.tree;
        let anchor_id = ensure_anchor(tree, ctx.sg, self.lowering, id, cursor.parent_anchor);
        let children = tree.ordered_children(id);
        clear_lowered_content(ctx.sg, anchor_id, &children, self.lowering);
        Some(anchor_id)
    }

    fn displayed(&mut self, id: ElementId, resolved: Visual) -> Visual {
        // Diff the after-change resolved visual against the previous frame's
        // displayed value at the resolve seam, interpolating changed continuous
        // properties (ADR-0093). The retained anchor holds the before-change value.
        self.lowering
            .anchors
            .get_mut(&id)
            .map(|entry| entry.resolve_displayed(&resolved, self.now_ms))
            .unwrap_or(resolved)
    }

    fn children(
        &self,
        tree: &ElementTree,
        cursor: RetainedCursor,
        id: ElementId,
        effective_parent: Option<NodeId>,
    ) -> Vec<(ElementId, RetainedCursor)> {
        visual_invalidation::children_for_reach(tree, id, cursor.reach)
            .into_iter()
            .map(|(child, reach)| {
                (
                    child,
                    RetainedCursor {
                        parent_anchor: effective_parent,
                        reach,
                    },
                )
            })
            .collect()
    }

    fn end_element(&mut self, ctx: &mut WalkCtx, effective_parent: Option<NodeId>, id: ElementId) {
        let tree = ctx.tree;
        reparent_child_anchors_under(
            ctx.sg,
            effective_parent,
            &tree.ordered_children(id),
            self.lowering,
        );
    }
}

/// Full ephemeral rebuild adapter (ADR-0079 golden-frame parity): fresh nodes
/// under the parent group, no anchors, no interpolation.
struct EphemeralSink;

impl AnchorSink for EphemeralSink {
    /// The parent group child nodes attach under.
    type Cursor = Option<NodeId>;

    fn enter_node(&mut self) {}

    fn begin(&mut self, _ctx: &mut WalkCtx, cursor: Option<NodeId>, _id: ElementId) -> Option<NodeId> {
        cursor
    }

    fn displayed(&mut self, _id: ElementId, resolved: Visual) -> Visual {
        // A full rebuild has no retained `last_displayed`, so it never interpolates
        // — it paints the resolved target directly (ADR-0093).
        resolved
    }

    fn children(
        &self,
        tree: &ElementTree,
        _cursor: Option<NodeId>,
        id: ElementId,
        effective_parent: Option<NodeId>,
    ) -> Vec<(ElementId, Option<NodeId>)> {
        tree.ordered_children(id)
            .into_iter()
            .map(|child| (child, effective_parent))
            .collect()
    }

    fn end_element(&mut self, _ctx: &mut WalkCtx, _effective_parent: Option<NodeId>, _id: ElementId) {}
}

/// Full ephemeral rebuild without retained anchors (parity reference / tests).
pub fn build_ephemeral(tree: &ElementTree) -> SceneGraph {
    let mut sg = SceneGraph::new();
    let interaction = tree.interaction_snapshot();
    if let Some(root) = tree.root() {
        let mut sink = EphemeralSink;
        let mut ctx = WalkCtx {
            tree,
            interaction: &interaction,
            sg: &mut sg,
            ox: 0.0,
            oy: 0.0,
            inherited: InheritedVisualContext::root(),
        };
        walk(&mut ctx, &mut sink, None, root);
    }
    // Selection chrome floats on top as document-level overlays (ADR-0097): the
    // drag handles first, then the toolbar above them.
    if let Some(handles) = tree.selection_handles() {
        emit_selection_handles(&mut sg, &handles);
    }
    if let Some(toolbar) = tree.selection_toolbar() {
        emit_selection_toolbar(&mut sg, tree, &toolbar);
    }
    sg
}

/// Incrementally update a scene graph using retained element anchors.
///
/// `now_ms` is the host clock driving in-flight transitions; the per-element
/// `resolve_effective` seam diffs the resolved visual against the stored
/// displayed value to start/advance interpolation (ADR-0093).
pub(crate) fn update(
    tree: &ElementTree,
    scene_cache: &mut SceneGraph,
    lowering: &mut SceneLowering,
    dirty: LoweringDirtySnapshot,
    now_ms: f64,
) {
    lowering.walk_count = 0;
    let interaction = tree.interaction_snapshot();

    if dirty.full_rebuild || !lowering.built {
        *scene_cache = SceneGraph::new();
        lowering.anchors.clear();
        if let Some(root) = tree.root() {
            let mut sink = RetainedSink {
                lowering: &mut *lowering,
                now_ms,
            };
            let mut ctx = WalkCtx {
                tree,
                interaction: &interaction,
                sg: &mut *scene_cache,
                ox: 0.0,
                oy: 0.0,
                inherited: InheritedVisualContext::root(),
            };
            walk(
                &mut ctx,
                &mut sink,
                RetainedCursor {
                    parent_anchor: None,
                    reach: VisualInvalidationReach::Subtree,
                },
                root,
            );
        }
        lowering.built = true;
        // The fresh graph dropped any prior overlay; re-emit from scratch.
        lowering.toolbar_root = None;
        lowering.handles_root = None;
        refresh_selection_chrome(tree, scene_cache, lowering);
        return;
    }

    if dirty.elements.is_empty() {
        // Even with no element repaints, the selection (hence its chrome) may
        // have moved or cleared, so the overlays are always refreshed.
        refresh_selection_chrome(tree, scene_cache, lowering);
        return;
    }

    for &parent_id in &dirty.z_index_reorder_parents {
        reorder_children_for_z_index(tree, scene_cache, lowering, parent_id);
    }

    let patch_roots = visual_invalidation::minimal_patch_roots(tree, &dirty.elements);
    {
        let mut sink = RetainedSink {
            lowering: &mut *lowering,
            now_ms,
        };
        for patch_root in patch_roots {
            let reach = dirty
                .elements
                .get(&patch_root)
                .copied()
                .unwrap_or(VisualInvalidationReach::Subtree);
            let parent_anchor = tree
                .elements
                .get(&patch_root)
                .and_then(|el| el.parent)
                .and_then(|parent| sink.lowering.anchors.get(&parent).map(|entry| entry.anchor_id));
            let (ox, oy) = tree
                .elements
                .get(&patch_root)
                .and_then(|el| el.parent)
                .and_then(|parent| tree.layout.geometry(parent))
                .map(|(x, y, _, _)| (x, y))
                .unwrap_or((0.0, 0.0));
            let inherited = effective_visual::inherited_context_at(&tree.elements, patch_root);
            let mut ctx = WalkCtx {
                tree,
                interaction: &interaction,
                sg: &mut *scene_cache,
                ox,
                oy,
                inherited,
            };
            walk(
                &mut ctx,
                &mut sink,
                RetainedCursor {
                    parent_anchor,
                    reach,
                },
                patch_root,
            );
        }
    }
    refresh_selection_chrome(tree, scene_cache, lowering);
}

/// Re-emit the core-drawn selection overlays (ADR-0097): the drag handles first,
/// then the floating toolbar on top, so the toolbar is inserted last and paints
/// above the handles.
fn refresh_selection_chrome(
    tree: &ElementTree,
    sg: &mut SceneGraph,
    lowering: &mut SceneLowering,
) {
    refresh_selection_handles(tree, sg, lowering);
    refresh_selection_toolbar(tree, sg, lowering);
}

/// Re-emit the selection drag-handles overlay (ADR-0097, #273). Removes the
/// previous overlay subtree, then draws fresh knobs when a non-collapsed
/// selection is active. Idempotent: a no-op (beyond removal) when no handles.
fn refresh_selection_handles(
    tree: &ElementTree,
    sg: &mut SceneGraph,
    lowering: &mut SceneLowering,
) {
    if let Some(prev) = lowering.handles_root.take() {
        sg.remove_subtree(prev);
    }
    let Some(handles) = tree.selection_handles() else {
        return;
    };
    lowering.handles_root = Some(emit_selection_handles(sg, &handles));
}

/// Lower the selection drag handles into a top-level overlay subtree: a `Group`
/// holding one filled circular knob per end (a square rect with a corner radius
/// of half its side), colored by the chrome style. Returns the group id.
fn emit_selection_handles(
    sg: &mut SceneGraph,
    handles: &crate::element::selection_chrome::SelectionHandles,
) -> NodeId {
    let color = handles.style.handle_color();
    let group = sg.insert(Node {
        kind: NodeKind::Group {
            transform: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        },
        children: Vec::new(),
    });
    for handle in [&handles.start, &handles.end] {
        let d = handle.radius * 2.0;
        sg.insert_child(
            group,
            Node {
                kind: NodeKind::Rect {
                    x: handle.knob_x - handle.radius,
                    y: handle.knob_y - handle.radius,
                    width: d,
                    height: d,
                    color,
                    corner_radius: handle.radius,
                },
                children: Vec::new(),
            },
        );
    }
    group
}

/// Re-emit the floating selection toolbar overlay (ADR-0097, #272). Removes the
/// previous overlay subtree, then draws a fresh one on top when a selection is
/// active. Idempotent: a no-op (beyond removal) when nothing is selected.
fn refresh_selection_toolbar(
    tree: &ElementTree,
    sg: &mut SceneGraph,
    lowering: &mut SceneLowering,
) {
    if let Some(prev) = lowering.toolbar_root.take() {
        sg.remove_subtree(prev);
    }
    let Some(toolbar) = tree.selection_toolbar() else {
        return;
    };
    lowering.toolbar_root = Some(emit_selection_toolbar(sg, tree, &toolbar));
}

/// Lower a [`SelectionToolbar`] into a top-level overlay subtree: a `Group`
/// holding a rounded background panel with the per-button label text runs on
/// top, inserted last so it paints above the document. Returns the group id.
fn emit_selection_toolbar(
    sg: &mut SceneGraph,
    tree: &ElementTree,
    toolbar: &crate::element::selection_chrome::SelectionToolbar,
) -> NodeId {
    let ct = tree.chrome_tuning();
    // The overlay root is a Group; its children are inserted via `insert_child`
    // so they are not also registered as top-level roots (which would double-
    // paint them, once as a root and once via the group walk).
    let group = sg.insert(Node {
        kind: NodeKind::Group {
            transform: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        },
        children: Vec::new(),
    });
    sg.insert_child(
        group,
        Node {
            kind: NodeKind::Rect {
                x: toolbar.bounds.x,
                y: toolbar.bounds.y,
                width: toolbar.bounds.width,
                height: toolbar.bounds.height,
                // The panel/label *colours* are theme-owned (Material vs
                // Cupertino, ADR-0097) and switch with `toolbar.style`, so they
                // stay style-derived — only the non-themed `corner_radius` is a
                // tuning knob.
                color: toolbar.style.toolbar_background(),
                corner_radius: ct.toolbar_corner_radius,
            },
            children: Vec::new(),
        },
    );
    let label_color = toolbar.style.toolbar_label();
    for button in &toolbar.buttons {
        let Some(label) = tree.toolbar_label_layout(button.action) else {
            continue;
        };
        // Center the label within its button cell.
        let lx = button.bounds.x + (button.bounds.width - label.layout.width()) / 2.0;
        let ly = button.bounds.y + (button.bounds.height - label.layout.height()) / 2.0;
        for run in &label.runs {
            sg.insert_child(
                group,
                Node {
                    kind: NodeKind::TextRun {
                        x: lx,
                        y: ly,
                        color: label_color,
                        data: run.clone(),
                    },
                    children: Vec::new(),
                },
            );
        }
    }
    group
}

fn reorder_children_for_z_index(
    tree: &ElementTree,
    sg: &mut SceneGraph,
    lowering: &SceneLowering,
    parent_id: ElementId,
) {
    let Some(parent_entry) = lowering.anchors.get(&parent_id) else {
        return;
    };
    let parent_anchor = parent_entry.anchor_id;
    let ordered = tree.ordered_children(parent_id);
    let child_anchors: Vec<NodeId> = ordered
        .iter()
        .filter_map(|child| lowering.anchors.get(child).map(|e| e.anchor_id))
        .collect();
    if let Some(parent) = sg.get_mut(parent_anchor) {
        parent.children = child_anchors;
    }
}


fn first_child_matching(
    sg: &SceneGraph,
    parent: NodeId,
    pred: impl Fn(&NodeKind) -> bool,
) -> Option<NodeId> {
    let parent_node = sg.get(parent)?;
    parent_node.children.iter().copied().find(|&child| {
        sg.get(child).is_some_and(|n| pred(&n.kind))
    })
}

/// Node under which child element anchors should attach — follows Clip/scroll Group
/// wrappers when the parent is a ScrollView (issue #199).
fn find_content_attachment_point(
    sg: &SceneGraph,
    anchor_id: NodeId,
    el: &crate::element::tree::Element,
) -> NodeId {
    let mut node = anchor_id;
    if el.transform.is_some() {
        node = first_child_matching(sg, node, |kind| matches!(kind, NodeKind::Group { .. }))
            .unwrap_or(node);
    }
    if el.kind == ElementKind::ScrollView {
        node = first_child_matching(sg, node, |kind| matches!(kind, NodeKind::Clip { .. }))
            .unwrap_or(node);
        let (sx, sy) = el.scroll_offset;
        if sx != 0.0 || sy != 0.0 {
            node = first_child_matching(sg, node, |kind| matches!(kind, NodeKind::Group { .. }))
                .unwrap_or(node);
        }
    }
    node
}

fn resolve_parent_attachment(
    tree: &ElementTree,
    sg: &SceneGraph,
    lowering: &SceneLowering,
    id: ElementId,
    parent_anchor: Option<NodeId>,
) -> Option<NodeId> {
    let parent_id = tree.elements.get(&id).and_then(|el| el.parent)?;
    let parent_entry = lowering.anchors.get(&parent_id)?;
    let parent_el = tree.elements.get(&parent_id)?;
    Some(find_content_attachment_point(
        sg,
        parent_entry.anchor_id,
        parent_el,
    ))
    .or(parent_anchor)
}

fn ensure_anchor(
    tree: &ElementTree,
    sg: &mut SceneGraph,
    lowering: &mut SceneLowering,
    id: ElementId,
    parent_anchor: Option<NodeId>,
) -> NodeId {
    let attach_parent = resolve_parent_attachment(tree, sg, lowering, id, parent_anchor);
    if let Some(entry) = lowering.anchors.get(&id) {
        let anchor_id = entry.anchor_id;
        if let Some(parent) = attach_parent {
            insert_anchor_ordered(tree, sg, lowering, id, parent, anchor_id);
        }
        return anchor_id;
    }

    let anchor_id = sg.insert(Node {
        kind: NodeKind::ElementAnchor { element_id: id },
        children: Vec::new(),
    });
    if let Some(parent) = attach_parent {
        insert_anchor_ordered(tree, sg, lowering, id, parent, anchor_id);
    }
    lowering.anchors.insert(id, AnchorEntry::new(anchor_id));
    anchor_id
}

/// Attach `child` (the anchor for element `id`) under `parent` at the scene-child
/// index that matches `id`'s position among its element siblings.
///
/// A partial patch re-walks only some of a parent's children (e.g. a hovered card,
/// or the grown/pushed siblings of an insert). Blindly appending a re-walked anchor
/// to the end of `parent.children` scrambles paint order, so the interacted element
/// paints over the wrong sibling — the "hover/click corrupts a *different* element"
/// symptom. Positioning relative to the preceding sibling *anchors actually present
/// under `parent`* keeps the retained child order in lockstep with element order and
/// is robust to Clip/Group content-attachment wrappers (all siblings share one
/// attachment point).
fn insert_anchor_ordered(
    tree: &ElementTree,
    sg: &mut SceneGraph,
    lowering: &SceneLowering,
    id: ElementId,
    parent: NodeId,
    child: NodeId,
) {
    sg.retain_roots(|root| root != child);
    if let Some(old_parent) = sg.parent_of(child) {
        if let Some(p) = sg.get_mut(old_parent) {
            p.children.retain(|&c| c != child);
        }
    }
    // Anchors of siblings that follow `id` in element order. Insert `child` just
    // before the first one present under `parent`; if none are present yet, append.
    // Inserting *before following siblings* (rather than *after preceding ones*)
    // keeps the parent's own content nodes — fill/border emitted before any child
    // anchor — ahead of every child, so the box still paints under its children.
    let following: HashSet<NodeId> = tree
        .elements
        .get(&id)
        .and_then(|el| el.parent)
        .map(|p| tree.ordered_children(p))
        .unwrap_or_default()
        .into_iter()
        .skip_while(|&sib| sib != id)
        .skip(1)
        .filter_map(|sib| lowering.anchors.get(&sib).map(|e| e.anchor_id))
        .collect();
    if let Some(p) = sg.get_mut(parent) {
        let index = p
            .children
            .iter()
            .position(|c| following.contains(c))
            .unwrap_or(p.children.len());
        p.children.insert(index, child);
    }
}

fn attach_under(sg: &mut SceneGraph, parent: NodeId, child: NodeId) {
    sg.retain_roots(|root| root != child);
    if let Some(old_parent) = sg.parent_of(child) {
        if let Some(p) = sg.get_mut(old_parent) {
            p.children.retain(|&id| id != child);
        }
    }
    if let Some(p) = sg.get_mut(parent) {
        if !p.children.contains(&child) {
            p.children.push(child);
        }
    }
}

/// Re-stack a re-walked element's child anchors after its own content, in element
/// order. `emit_element` emits the box's own content (fill/border/text) by
/// appending, but `clear_lowered_content` preserves child anchors at the front of
/// the list — so without this pass the box's own fill paints *over* its children
/// (and stale sibling order survives). Re-attaching every child in element order
/// after content emission restores `[content..., child0, child1, ...]`.
///
/// Also handles the Clip/scroll-Group wrapper case it was written for: when
/// `effective_parent` is a wrapper inside the anchor, children slide under the
/// wrapper so clipping still applies.
fn reparent_child_anchors_under(
    sg: &mut SceneGraph,
    effective_parent: Option<NodeId>,
    children: &[ElementId],
    lowering: &SceneLowering,
) {
    let Some(parent) = effective_parent else {
        return;
    };
    for &child_id in children {
        let Some(child_anchor) = lowering.anchors.get(&child_id).map(|e| e.anchor_id) else {
            continue;
        };
        attach_under(sg, parent, child_anchor);
    }
}

/// The one scene walk shared by both anchor strategies (issue #322). The
/// strategy-specific anchoring is delegated to the [`AnchorSink`]; the emission
/// body lives in [`emit_element`]. A skipped (non-visited) element still gets a
/// `begin`/recurse pass so retained re-attaches its anchor.
fn walk<S: AnchorSink>(ctx: &mut WalkCtx, sink: &mut S, cursor: S::Cursor, id: ElementId) {
    sink.enter_node();

    let tree = ctx.tree;
    let (taffy_node, el) = match tree.layout.projection.traversal_step(&tree.elements, id) {
        Some(TraversalStep::Visit(taffy_node, el)) => (taffy_node, el),
        Some(TraversalStep::Skip(_)) => {
            let base = sink.begin(ctx, cursor, id);
            for (child, child_cursor) in sink.children(tree, cursor, id, base) {
                let mut child_ctx = ctx.child(ctx.ox, ctx.oy, ctx.inherited.clone());
                walk(&mut child_ctx, sink, child_cursor, child);
            }
            return;
        }
        None => return,
    };

    let base = sink.begin(ctx, cursor, id);
    emit_element(ctx, sink, cursor, id, el, taffy_node, base);
}

fn emit_element<S: AnchorSink>(
    ctx: &mut WalkCtx,
    sink: &mut S,
    cursor: S::Cursor,
    id: ElementId,
    el: &crate::element::tree::Element,
    taffy_node: taffy::NodeId,
    base: Option<NodeId>,
) {
    let tree = ctx.tree;
    let inherited_base = effective_visual::apply_text_inheritance(&ctx.inherited, &el.visual);
    let child_inherited = child_inherited_context(
        &ctx.inherited,
        el.kind,
        &inherited_base,
        &el.visual,
    );
    let resolved = effective_visual::resolve_effective(
        &ctx.inherited,
        &el.visual,
        &el.viewport_variants,
        tree.viewport(),
        &el.pseudo_styles,
        ctx.interaction,
        id,
    );
    let visual = sink.displayed(id, resolved);
    let layout = match tree.layout.projection.taffy.layout(taffy_node) {
        Ok(l) => l,
        Err(_) => return,
    };
    let x = ctx.ox + layout.location.x;
    let y = ctx.oy + layout.location.y;
    let w = layout.size.width;
    let h = layout.size.height;

    let confirmed_color = visual.text_color.unwrap_or(Color::BLACK);
    let confirmed_font_size = visual.font_size.unwrap_or(16.0);

    let mut effective_parent = base;
    if let Some(transform) = el.transform {
        let group_id = emit(
            ctx.sg,
            effective_parent,
            Node {
                kind: NodeKind::Group { transform },
                children: Vec::new(),
            },
        );
        effective_parent = Some(group_id);
    }

    // Parent for the native focus ring: above the element's own overflow clip so
    // the ring isn't cropped to the box (Chromium paints outlines outside the
    // element's clip), but still inside any transform group (#335).
    let ring_parent = effective_parent;

    let effective_parent = if el.kind == ElementKind::ScrollView {
        let clip_id = emit(
            ctx.sg,
            effective_parent,
            Node {
                kind: NodeKind::Clip {
                    x,
                    y,
                    width: w,
                    height: h,
                    corner_radii: [0.0; 4],
                },
                children: Vec::new(),
            },
        );
        let (sx, sy) = el.scroll_offset;
        if sx != 0.0 || sy != 0.0 {
            let scroll_group = emit(
                ctx.sg,
                Some(clip_id),
                Node {
                    kind: NodeKind::Group {
                        transform: [1.0, 0.0, 0.0, 1.0, -sx as f64, -sy as f64],
                    },
                    children: Vec::new(),
                },
            );
            Some(scroll_group)
        } else {
            Some(clip_id)
        }
    } else if visual.overflow == OverflowValue::Hidden {
        let clip_id = emit(
            ctx.sg,
            effective_parent,
            Node {
                kind: NodeKind::Clip {
                    x,
                    y,
                    width: w,
                    height: h,
                    corner_radii: [visual.border_radius; 4],
                },
                children: Vec::new(),
            },
        );
        Some(clip_id)
    } else {
        effective_parent
    };

    if !visual.box_shadow.is_empty() {
        emit_box_shadows(
            ctx.sg,
            effective_parent,
            x,
            y,
            w,
            h,
            visual.border_radius,
            &visual.box_shadow,
            visual.opacity,
            false,
        );
    }

    emit_visual_box(
        ctx.sg,
        effective_parent,
        x,
        y,
        w,
        h,
        visual.border_radius,
        visual.border_width,
        visual.background_color,
        visual.border_color,
        visual.border_style,
        visual.opacity,
    );

    if !visual.box_shadow.is_empty() {
        emit_box_shadows(
            ctx.sg,
            effective_parent,
            x,
            y,
            w,
            h,
            visual.border_radius,
            &visual.box_shadow,
            visual.opacity,
            true,
        );
    }

    if el.kind == ElementKind::Image {
        if let Some(img) = el.src_image.clone() {
            emit(
                ctx.sg,
                effective_parent,
                Node {
                    kind: NodeKind::Image {
                        x,
                        y,
                        width: w,
                        height: h,
                        data: img,
                    },
                    children: Vec::new(),
                },
            );
        }
    } else if el.kind == ElementKind::TextInput {
        let content_x = x + layout.border.left + layout.padding.left;
        let content_y = y + layout.border.top + layout.padding.top;
        let color = confirmed_color
            .with_opacity(visual.opacity)
            .to_array_f32();
        // Selection highlight paints behind the text (ADR-0097, #271), but only
        // for the focused text-input (ADR-0104): an unfocused field hides its
        // highlight even when the range is still remembered in EditState, so
        // Mouse/Pen blur reads as "hidden + remembered" and at most one
        // (= the focused) selection ever lights up across the document.
        if let Some(cl) = el.content_layout.as_ref() {
            let active_range = (tree.focused_element() == Some(id))
                .then(|| el.edit.as_ref().and_then(|e| e.selection_range()))
                .flatten();
            emit_edit_selection_highlight(
                &cl.layout,
                active_range,
                content_x,
                content_y,
                tree.chrome_tuning().selection_highlight_color,
                ctx.sg,
                effective_parent,
            );
        }
        // An empty input shows its placeholder: layout_pass leaves
        // `content_layout` empty and stacks the placeholder in `text_layout`
        // (ADR-0058). Chromium paints `::placeholder` muted rather than in the
        // body `color`; Canvas's visual reference is the Chromium DOM, so the
        // placeholder run is painted muted, not `confirmed_color` (ADR-0102,
        // #334).
        let (runs, run_color) = if let Some(cl) = el.content_layout.as_ref() {
            (Some(cl.runs.as_slice()), color)
        } else {
            let muted = placeholder_muted_color(confirmed_color, tree.chrome_tuning().placeholder_alpha)
                .with_opacity(visual.opacity)
                .to_array_f32();
            (el.text_layout.as_ref().map(|tl| tl.runs.as_slice()), muted)
        };
        if let Some(runs) = runs {
            for run in runs {
                emit(
                    ctx.sg,
                    effective_parent,
                    Node {
                        kind: NodeKind::TextRun {
                            x: content_x,
                            y: content_y,
                            color: run_color,
                            data: run.clone(),
                        },
                        children: Vec::new(),
                    },
                );
            }
        }
        // IME composition underlines: one per clause, drawn under the preedit
        // glyphs. Chromium underlines the active (being-converted) clause thick
        // and the determined ones thin (ADR-0102, #336).
        if let Some(cl) = el.content_layout.as_ref() {
            if let Some(edit) = el.edit.as_ref() {
                emit_composition_underlines(
                    &cl.layout,
                    &edit.composition_underlines(),
                    content_x,
                    content_y,
                    color,
                    tree.chrome_tuning().composition_underline_thin,
                    tree.chrome_tuning().composition_underline_thick,
                    ctx.sg,
                    effective_parent,
                );
            }
        }
        if el.cursor_visible {
            if let Some(cl) = el.content_layout.as_ref() {
                let cursor_index = el
                    .edit
                    .as_ref()
                    .map(|edit| edit.cursor_byte_index)
                    .unwrap_or(0);
                let cursor = parley::Cursor::from_byte_index(
                    &cl.layout,
                    cursor_index,
                    parley::Affinity::Upstream,
                );
                let bbox = cursor.geometry(&cl.layout, 1.5_f32);
                emit(
                    ctx.sg,
                    effective_parent,
                    Node {
                        kind: NodeKind::Rect {
                            x: content_x + bbox.x0 as f32,
                            y: content_y + bbox.y0 as f32,
                            width: ((bbox.x1 - bbox.x0) as f32).max(1.5),
                            height: (bbox.y1 - bbox.y0) as f32,
                            color,
                            corner_radius: 0.0,
                        },
                        children: Vec::new(),
                    },
                );
            } else {
                emit(
                    ctx.sg,
                    effective_parent,
                    Node {
                        kind: NodeKind::Rect {
                            x: content_x,
                            y: content_y,
                            width: 1.5,
                            height: confirmed_font_size * 1.2,
                            color: confirmed_color
                                .with_opacity(visual.opacity)
                                .to_array_f32(),
                            corner_radius: 0.0,
                        },
                        children: Vec::new(),
                    },
                );
            }
        }
    } else if let Some(tl) = el.text_layout.as_ref() {
        let color = confirmed_color
            .with_opacity(visual.opacity)
            .to_array_f32();
        emit_selection_highlight(tree, id, &tl.layout, x, y, ctx.sg, effective_parent);
        for run in &tl.runs {
            emit(
                ctx.sg,
                effective_parent,
                Node {
                    kind: NodeKind::TextRun {
                        x,
                        y,
                        color,
                        data: run.clone(),
                    },
                    children: Vec::new(),
                },
            );
        }
    }

    // Native focus ring (`:focus-visible`, #335). Painted on top of the box's own
    // content and outside its border, following the box corner radius. The
    // application's `:focus` background/border switch is resolved separately via
    // pseudo styles above and is unaffected.
    if tree.focus_visible_element() == Some(id) {
        emit_focus_ring(ctx.sg, ring_parent, x, y, w, h, visual.border_radius);
    }

    // Scrollbar overlay (ADR-0110, #407): drawn over the content on each
    // overflowing axis, under `ring_parent` so it sits above the content Clip and
    // scroll Group (like the focus ring) and is *not* itself scroll-translated —
    // the thumb is fixed to the box edge while its position along the track tracks
    // the Scroll Offset. For a nested scroll-view, `ring_parent` already lives
    // under the outer Clip/scroll Group, so the inner thumb follows the outer box
    // and cannot leak outside it (issue #199/#200 coordinate system).
    if el.kind == ElementKind::ScrollView {
        emit_scrollbar_overlay(tree, id, ctx.sg, ring_parent, x, y, w, h);
    }

    for (child, child_cursor) in sink.children(tree, cursor, id, effective_parent) {
        let mut child_ctx = ctx.child(x, y, child_inherited.clone());
        walk(&mut child_ctx, sink, child_cursor, child);
    }
    sink.end_element(ctx, effective_parent, id);
}

/// Emit a `RoundedRing` wrapping the box `(x, y, width, height)` from the outside
/// — the native focus ring (#335). The outer rect is grown by the offset plus the
/// ring width on every side; the ring's inner edge then lands `FOCUS_RING_OFFSET`
/// outside the border box, matching Chromium's `outline-offset`.
fn emit_focus_ring(
    sg: &mut SceneGraph,
    parent_group: Option<NodeId>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    border_radius: f32,
) {
    let grow = FOCUS_RING_OFFSET + FOCUS_RING_WIDTH;
    emit(
        sg,
        parent_group,
        Node {
            kind: NodeKind::RoundedRing {
                x: x - grow,
                y: y - grow,
                width: width + 2.0 * grow,
                height: height + 2.0 * grow,
                outer_radius: border_radius.max(0.0) + grow,
                border_width: FOCUS_RING_WIDTH,
                color: FOCUS_RING_COLOR.to_array_f32(),
            },
            children: Vec::new(),
        },
    );
}

/// Thumb extent `(start, length)` along one scroll axis, in the box-local track
/// space whose origin is the box edge. `viewport` is the box length on the axis,
/// `content` the scrollable content length, `offset` the current Scroll Offset and
/// `max` its maximum. The length scales with the viewport/content ratio, floored
/// at [`SCROLLBAR_MIN_THUMB_LENGTH`]; the start slides the thumb down the track by
/// the offset as a fraction of the scrollable range.
fn scrollbar_thumb_extent(
    viewport: f32,
    content: f32,
    offset: f32,
    max: f32,
    track_margin: f32,
    min_thumb_length: f32,
) -> (f32, f32) {
    let track_len = (viewport - 2.0 * track_margin).max(0.0);
    let thumb_len = (track_len * viewport / content)
        .max(min_thumb_length)
        .min(track_len);
    let progress = if max > 0.0 {
        (offset / max).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let start = track_margin + (track_len - thumb_len) * progress;
    (start, thumb_len)
}

/// Scrollbar geometry per overflowing axis for the box `(x, y, w, h)` already
/// resolved by the caller. The shared core of [`scrollbar_axes`] (which supplies
/// the box from layout) and [`emit_scrollbar_overlay`] (which supplies the box
/// from its scene walk), so paint and hit-test compute one identical geometry.
fn scrollbar_axes_in_box(
    tree: &ElementTree,
    id: ElementId,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) -> Vec<ScrollbarAxisGeometry> {
    let ct = tree.chrome_tuning();
    let (content_w, content_h) = tree.element_content_size(id);
    let (max_x, max_y) = tree.element_scroll_max_offset(id);
    let (offset_x, offset_y) = tree.element_get_scroll_offset(id);
    let mut axes = Vec::new();

    // Vertical bar at the right edge — only when content overflows the box height.
    if content_h > h {
        let (start, thumb_len) = scrollbar_thumb_extent(
            h,
            content_h,
            offset_y,
            max_y,
            ct.scrollbar_track_margin,
            ct.scrollbar_min_thumb_length,
        );
        let track_len = (h - 2.0 * ct.scrollbar_track_margin).max(0.0);
        let bar_x = x + w - ct.scrollbar_track_margin - ct.scrollbar_thickness;
        axes.push(ScrollbarAxisGeometry {
            axis: ScrollAxis::Vertical,
            thumb: (bar_x, y + start, ct.scrollbar_thickness, thumb_len),
            track: (
                bar_x,
                y + ct.scrollbar_track_margin,
                ct.scrollbar_thickness,
                track_len,
            ),
            max_offset: max_y,
            thumb_travel: (track_len - thumb_len).max(0.0),
        });
    }

    // Horizontal bar at the bottom edge — only when content overflows the width.
    if content_w > w {
        let (start, thumb_len) = scrollbar_thumb_extent(
            w,
            content_w,
            offset_x,
            max_x,
            ct.scrollbar_track_margin,
            ct.scrollbar_min_thumb_length,
        );
        let track_len = (w - 2.0 * ct.scrollbar_track_margin).max(0.0);
        let bar_y = y + h - ct.scrollbar_track_margin - ct.scrollbar_thickness;
        axes.push(ScrollbarAxisGeometry {
            axis: ScrollAxis::Horizontal,
            thumb: (x + start, bar_y, thumb_len, ct.scrollbar_thickness),
            track: (
                x + ct.scrollbar_track_margin,
                bar_y,
                track_len,
                ct.scrollbar_thickness,
            ),
            max_offset: max_x,
            thumb_travel: (track_len - thumb_len).max(0.0),
        });
    }

    axes
}

/// Lower a `ScrollView`'s scrollbar overlay (ADR-0110, #407): one rounded thumb
/// per overflowing axis, drawn under `parent` (above the content clip). The
/// vertical bar sits at the right edge, the horizontal bar at the bottom edge; an
/// axis whose content fits draws nothing.
#[allow(clippy::too_many_arguments)]
fn emit_scrollbar_overlay(
    tree: &ElementTree,
    id: ElementId,
    sg: &mut SceneGraph,
    parent: Option<NodeId>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) {
    // Pointer-Modality branch (ADR-0110, SCR-04, #410), reusing the same last
    // pointer kind that gates selection chrome (ADR-0104) — Mouse/Pen get the
    // operable thumb, Touch gets the transient indicator.
    match tree.last_pointer_kind() {
        PointerKind::Touch => emit_touch_scroll_indicator(tree, id, sg, parent, x, y, w, h),
        PointerKind::Mouse | PointerKind::Pen => {
            let ct = tree.chrome_tuning();
            let thumb_rgba = ct
                .scrollbar_thumb_color
                .with_opacity(ct.scrollbar_thumb_opacity)
                .to_array_f32();
            let radius = ct.scrollbar_thickness / 2.0;
            for axis in scrollbar_axes_in_box(tree, id, x, y, w, h) {
                let (tx, ty, tw, th) = axis.thumb;
                emit_fill_rect(sg, parent, tx, ty, tw, th, thumb_rgba, radius);
            }
        }
    }
}

/// The Touch indicator's visibility factor `[0, 1]` for an indicator last
/// refreshed `elapsed` ms ago (ADR-0110, SCR-04, #410): full through the hold
/// window, then a linear ramp to zero across the fade window, and zero beyond it.
/// The single source the render-time advance uses to recompute each live
/// indicator's `fade`.
pub fn touch_scroll_indicator_fade(elapsed: f64) -> f32 {
    if elapsed <= SCROLLBAR_INDICATOR_HOLD_MS {
        1.0
    } else if elapsed >= SCROLLBAR_INDICATOR_HOLD_MS + SCROLLBAR_INDICATOR_FADE_MS {
        0.0
    } else {
        (1.0 - (elapsed - SCROLLBAR_INDICATOR_HOLD_MS) / SCROLLBAR_INDICATOR_FADE_MS) as f32
    }
}

/// Lower a `ScrollView`'s Touch transient indicator (ADR-0110, SCR-04, #410): a
/// non-operable bar that appears while the content scrolls and fades after it
/// stops, with no thumb/track hit region (content flick scrolls, not a drag).
/// Drawn only inside the show→fade window — a resting Touch surface paints no
/// scrollbar at all (mobile has no always-on bar).
#[allow(clippy::too_many_arguments)]
fn emit_touch_scroll_indicator(
    tree: &ElementTree,
    id: ElementId,
    sg: &mut SceneGraph,
    parent: Option<NodeId>,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) {
    let fade = tree.touch_scroll_indicator_opacity(id);
    if fade <= 0.0 {
        return;
    }
    let ct = tree.chrome_tuning();
    let rgba = ct
        .scrollbar_indicator_color
        .with_opacity(ct.scrollbar_indicator_opacity * fade)
        .to_array_f32();
    let radius = ct.scrollbar_indicator_thickness / 2.0;
    for axis in scrollbar_axes_in_box(tree, id, x, y, w, h) {
        // The indicator rides the same thumb geometry (its position still tracks
        // the Scroll Offset) but is thinner and pinned to the box edge — right
        // edge for the vertical bar, bottom edge for the horizontal one.
        let (tx, ty, tw, th) = axis.thumb;
        let (ix, iy, iw, ih) = match axis.axis {
            ScrollAxis::Vertical => (
                tx + tw - ct.scrollbar_indicator_thickness,
                ty,
                ct.scrollbar_indicator_thickness,
                th,
            ),
            ScrollAxis::Horizontal => (
                tx,
                ty + th - ct.scrollbar_indicator_thickness,
                tw,
                ct.scrollbar_indicator_thickness,
            ),
        };
        emit_fill_rect(sg, parent, ix, iy, iw, ih, rgba, radius);
    }
}

/// Chromium UA `::placeholder` muted colour, used in place of the body `color`
/// when a TextInput shows its placeholder (ADR-0102: Canvas's visual reference
/// is the Chromium DOM; #334). Chromium paints the placeholder at ~54% of black
/// (light colour-scheme) or white (dark), composited over the input background —
/// it is not derived from, nor authorable alongside, the body `color`. The
/// colour-scheme is inferred from the body colour's luminance: dark body text
/// ⇒ light scheme ⇒ muted black; light body text ⇒ dark scheme ⇒ muted white.
/// The 0.54 factor follows ADR-0102's principle (~54% black/white); its exact
/// value is still pending calibration against real Chromium rendering.
///
/// The ~54% muting factor (its exact value pending Chromium calibration).
pub(crate) const PLACEHOLDER_ALPHA: f64 = 0.54;

fn placeholder_muted_color(body: Color, alpha: f64) -> Color {
    let luma = 0.299 * body.r + 0.587 * body.g + 0.114 * body.b;
    let base = if luma < 0.5 { Color::BLACK } else { Color::WHITE };
    Color::new(base.r, base.g, base.b, alpha)
}

fn emit(sg: &mut SceneGraph, parent_group: Option<NodeId>, node: Node) -> NodeId {
    match parent_group {
        None => sg.insert(node),
        Some(p) => sg.insert_child(p, node),
    }
}

/// Material-flavored selection tint (ADR-0097: a single core-drawn chrome whose
/// style is theme-switchable; the value lives here as the initial theme).
pub(crate) const SELECTION_HIGHLIGHT_COLOR: [f32; 4] = [0.20, 0.45, 0.95, 0.35];

/// Lower the active selection's highlight for IFC root `id`, as one filled rect
/// per covered line, positioned in the element's content space (offset by the
/// text run origin `ox`, `oy`). No-op unless the document selection lies in `id`.
fn emit_selection_highlight(
    tree: &ElementTree,
    id: ElementId,
    layout: &parley::Layout<crate::element::text::TextBrush>,
    ox: f32,
    oy: f32,
    sg: &mut SceneGraph,
    parent: Option<NodeId>,
) {
    let Some((start, end)) = tree.selection_range_in_block(id) else {
        return;
    };
    let highlight_color = tree.chrome_tuning().selection_highlight_color;
    for (rx, ry, rw, rh) in selection_highlight_rects(layout, start, end) {
        emit(
            sg,
            parent,
            Node {
                kind: NodeKind::Rect {
                    x: ox + rx,
                    y: oy + ry,
                    width: rw,
                    height: rh,
                    color: highlight_color,
                    corner_radius: 0.0,
                },
                children: Vec::new(),
            },
        );
    }
}

/// Lower a text-input's edit-selection highlight (ADR-0097, #271) from its
/// `EditState` byte `range` over the `content_layout`, in the element's content
/// space (offset by `content_x`, `content_y`). Painted behind the text. No-op
/// when the range is collapsed/absent.
fn emit_edit_selection_highlight(
    layout: &parley::Layout<crate::element::text::TextBrush>,
    range: Option<(usize, usize)>,
    content_x: f32,
    content_y: f32,
    highlight_color: [f32; 4],
    sg: &mut SceneGraph,
    parent: Option<NodeId>,
) {
    let Some((start, end)) = range else {
        return;
    };
    for (rx, ry, rw, rh) in selection_highlight_rects(layout, start, end) {
        emit(
            sg,
            parent,
            Node {
                kind: NodeKind::Rect {
                    x: content_x + rx,
                    y: content_y + ry,
                    width: rw,
                    height: rh,
                    color: highlight_color,
                    corner_radius: 0.0,
                },
                children: Vec::new(),
            },
        );
    }
}

/// IME composition underline thickness (ADR-0102, #336). Chromium draws the
/// determined clauses with a thin underline and the active (being-converted)
/// clause with a thick one; the exact pixel weights are pending calibration
/// against real Chromium rasterisation, like the other Canvas chrome values.
pub(crate) const COMPOSITION_UNDERLINE_THIN: f32 = 1.0;
pub(crate) const COMPOSITION_UNDERLINE_THICK: f32 = 2.0;

/// Lower a text-input's IME composition underlines (ADR-0102, #336): one filled
/// rect per clause, sat at the bottom of each covered line in the element's
/// content space (offset by `content_x`, `content_y`), painted in the text
/// `color`. `underlines` are display-text byte ranges with their weight; no-op
/// when no composition is active.
#[allow(clippy::too_many_arguments)]
fn emit_composition_underlines(
    layout: &parley::Layout<crate::element::text::TextBrush>,
    underlines: &[(usize, usize, crate::element::edit_state::CompositionUnderline)],
    content_x: f32,
    content_y: f32,
    color: [f32; 4],
    thin: f32,
    thick: f32,
    sg: &mut SceneGraph,
    parent: Option<NodeId>,
) {
    use crate::element::edit_state::CompositionUnderline;
    for &(start, end, weight) in underlines {
        let thickness = match weight {
            CompositionUnderline::Thin => thin,
            CompositionUnderline::Thick => thick,
        };
        for (rx, ry, rw, rh) in selection_highlight_rects(layout, start, end) {
            emit(
                sg,
                parent,
                Node {
                    kind: NodeKind::Rect {
                        x: content_x + rx,
                        y: content_y + ry + rh - thickness,
                        width: rw,
                        height: thickness,
                        color,
                        corner_radius: 0.0,
                    },
                    children: Vec::new(),
                },
            );
        }
    }
}

/// Per-line highlight rectangles (in layout-local coordinates) covering the byte
/// range `start..end` of a Parley layout. Each line contributes the span from
/// the caret at its clamped range start to the caret at its clamped range end.
pub(crate) fn selection_highlight_rects(
    layout: &parley::Layout<crate::element::text::TextBrush>,
    start: usize,
    end: usize,
) -> Vec<(f32, f32, f32, f32)> {
    use parley::{Affinity, Cursor};
    let mut rects = Vec::new();
    if start >= end {
        return rects;
    }
    for line in layout.lines() {
        let line_range = line.text_range();
        let s = start.max(line_range.start);
        let e = end.min(line_range.end);
        if s >= e {
            continue;
        }
        let m = line.metrics();
        let y0 = m.block_min_coord;
        let height = m.block_max_coord - m.block_min_coord;
        let x_start = Cursor::from_byte_index(layout, s, Affinity::Downstream)
            .geometry(layout, 0.0)
            .x0 as f32;
        let x_end = Cursor::from_byte_index(layout, e, Affinity::Upstream)
            .geometry(layout, 0.0)
            .x0 as f32;
        let left = x_start.min(x_end);
        let width = (x_end - x_start).abs();
        if width > 0.0 && height > 0.0 {
            rects.push((left, y0, width, height));
        }
    }
    rects
}

fn emit_visual_box(
    sg: &mut SceneGraph,
    parent_group: Option<NodeId>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    border_radius: f32,
    border_width: f32,
    background_color: Option<Color>,
    border_color: Option<Color>,
    border_style: BorderStyleValue,
    opacity: f32,
) {
    let radius = border_radius.max(0.0);
    let border_w = border_width.max(0.0);
    let background = background_color.map(|c| c.with_opacity(opacity).to_array_f32());
    let border = border_color.map(|c| c.with_opacity(opacity).to_array_f32());

    // A border is drawn only when it has both a positive width and an explicit
    // style (CSS-like: `border-style` defaults to `none`, issue #204).
    let draw_border = border_w > 0.0 && border_style != BorderStyleValue::None;

    if draw_border {
        let Some(border_rgba) = border else {
            if let Some(bg) = background {
                emit_fill_rect(sg, parent_group, x, y, width, height, bg, radius);
            }
            return;
        };

        if border_style == BorderStyleValue::Dashed {
            // Background fills the full box; dashes stroke the perimeter on top.
            if let Some(bg) = background {
                emit_fill_rect(sg, parent_group, x, y, width, height, bg, radius);
            }
            emit(
                sg,
                parent_group,
                Node {
                    kind: NodeKind::DashedBorder {
                        x,
                        y,
                        width,
                        height,
                        outer_radius: radius,
                        border_width: border_w,
                        color: border_rgba,
                    },
                    children: Vec::new(),
                },
            );
            return;
        }

        if let Some(bg) = background {
            emit_fill_rect(
                sg,
                parent_group,
                x,
                y,
                width,
                height,
                border_rgba,
                radius,
            );
            let inner_w = (width - 2.0 * border_w).max(0.0);
            let inner_h = (height - 2.0 * border_w).max(0.0);
            if inner_w > 0.0 && inner_h > 0.0 {
                let inner_radius = (radius - border_w).max(0.0);
                emit_fill_rect(
                    sg,
                    parent_group,
                    x + border_w,
                    y + border_w,
                    inner_w,
                    inner_h,
                    bg,
                    inner_radius,
                );
            }
            return;
        }

        if radius > 0.0 {
            emit(
                sg,
                parent_group,
                Node {
                    kind: NodeKind::RoundedRing {
                        x,
                        y,
                        width,
                        height,
                        outer_radius: radius,
                        border_width: border_w,
                        color: border_rgba,
                    },
                    children: Vec::new(),
                },
            );
            return;
        }

        for (bx, by, bw2, bh2) in [
            (x, y, width, border_w),
            (x, y + height - border_w, width, border_w),
            (x, y + border_w, border_w, (height - 2.0 * border_w).max(0.0)),
            (
                x + width - border_w,
                y + border_w,
                border_w,
                (height - 2.0 * border_w).max(0.0),
            ),
        ] {
            emit_fill_rect(sg, parent_group, bx, by, bw2, bh2, border_rgba, 0.0);
        }
        return;
    }

    if let Some(bg) = background {
        emit_fill_rect(sg, parent_group, x, y, width, height, bg, radius);
    }
}

fn emit_fill_rect(
    sg: &mut SceneGraph,
    parent_group: Option<NodeId>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: [f32; 4],
    corner_radius: f32,
) {
    emit(
        sg,
        parent_group,
        Node {
            kind: NodeKind::Rect {
                x,
                y,
                width,
                height,
                color,
                corner_radius,
            },
            children: Vec::new(),
        },
    );
}

/// Number of translucent layers used to approximate a shadow's gaussian blur
/// (ADR-0095: "blur は許容範囲のガウス近似でよい"). Box-shadow is lowered to plain
/// rounded-rect fills so the Vello and tiny-skia painters render it identically
/// (semantic DOM/Canvas parity); blur ≈ overlapping translucent rounded rects.
const SHADOW_BLUR_LAYERS: usize = 6;

/// Emit the `inset == want_inset` subset of an element's box-shadow layers.
///
/// CSS paints the first-listed shadow on top, so we emit in reverse order (the
/// last-listed shadow first / bottom-most). Outset shadows are emitted behind
/// the box; inset shadows on top of the background, clipped to the border box.
#[allow(clippy::too_many_arguments)]
fn emit_box_shadows(
    sg: &mut SceneGraph,
    parent_group: Option<NodeId>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    border_radius: f32,
    shadows: &[Shadow],
    opacity: f32,
    want_inset: bool,
) {
    let radius = border_radius.max(0.0);
    for shadow in shadows.iter().rev() {
        if shadow.inset != want_inset {
            continue;
        }
        let color = shadow.color.with_opacity(opacity);
        if color.a <= 0.0 {
            continue;
        }
        if want_inset {
            emit_inset_shadow(sg, parent_group, x, y, width, height, radius, shadow, color);
        } else {
            emit_drop_shadow(sg, parent_group, x, y, width, height, radius, shadow, color);
        }
    }
}

/// Outset (drop) shadow: a rounded rect grown by `spread`, shifted by the
/// offset, and blurred by overlapping translucent rounded rects.
#[allow(clippy::too_many_arguments)]
fn emit_drop_shadow(
    sg: &mut SceneGraph,
    parent_group: Option<NodeId>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radius: f32,
    shadow: &Shadow,
    color: Color,
) {
    let sx = x + shadow.offset_x - shadow.spread;
    let sy = y + shadow.offset_y - shadow.spread;
    let sw = (width + 2.0 * shadow.spread).max(0.0);
    let sh = (height + 2.0 * shadow.spread).max(0.0);
    let sr = (radius + shadow.spread).max(0.0);
    if sw <= 0.0 || sh <= 0.0 {
        return;
    }

    let blur = shadow.blur.max(0.0);
    if blur <= 0.5 {
        emit_fill_rect(sg, parent_group, sx, sy, sw, sh, color.to_array_f32(), sr);
        return;
    }

    // Distribute the colour alpha across overlapping layers so the dense centre
    // sums to ≈ the shadow's alpha while the outer edge fades to a soft halo.
    let n = SHADOW_BLUR_LAYERS;
    let layer = Color {
        a: color.a / (n as f64 + 1.0),
        ..color
    };
    let layer_rgba = layer.to_array_f32();
    for i in (0..=n).rev() {
        let grow = blur * (i as f32) / (n as f32);
        emit_fill_rect(
            sg,
            parent_group,
            sx - grow,
            sy - grow,
            sw + 2.0 * grow,
            sh + 2.0 * grow,
            layer_rgba,
            (sr + grow).max(0.0),
        );
    }
}

/// Inset shadow: a darkened inner-edge band, layered from the border-box edge
/// inward (spread + blur thick) and clipped to the border box.
#[allow(clippy::too_many_arguments)]
fn emit_inset_shadow(
    sg: &mut SceneGraph,
    parent_group: Option<NodeId>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    radius: f32,
    shadow: &Shadow,
    color: Color,
) {
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    let clip_id = emit(
        sg,
        parent_group,
        Node {
            kind: NodeKind::Clip {
                x,
                y,
                width,
                height,
                corner_radii: [radius; 4],
            },
            children: Vec::new(),
        },
    );

    let band = (shadow.spread + shadow.blur).max(0.5);
    let max_band = width.min(height) * 0.5;
    let n = SHADOW_BLUR_LAYERS;
    let layer = Color {
        a: color.a / n as f64,
        ..color
    };
    let layer_rgba = layer.to_array_f32();
    // Additive translucent edge bands, clipped to the (rounded) border box.
    // Overlapping layers darken the inner perimeter and fade toward the centre,
    // approximating an inset shadow without clearing the background (unlike a
    // ring fill). The offset only nudges the band rectangle.
    let bx = x + shadow.offset_x;
    let by = y + shadow.offset_y;
    for i in 1..=n {
        let bw = (band * (i as f32) / (n as f32)).min(max_band);
        if bw <= 0.0 {
            continue;
        }
        // top, bottom, left, right bands
        for (rx, ry, rw, rh) in [
            (bx, by, width, bw),
            (bx, by + height - bw, width, bw),
            (bx, by + bw, bw, (height - 2.0 * bw).max(0.0)),
            (bx + width - bw, by + bw, bw, (height - 2.0 * bw).max(0.0)),
        ] {
            if rw <= 0.0 || rh <= 0.0 {
                continue;
            }
            emit_fill_rect(sg, Some(clip_id), rx, ry, rw, rh, layer_rgba, 0.0);
        }
    }
}
