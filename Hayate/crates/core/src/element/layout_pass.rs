use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use linebender_resource_handle::Blob;
use parley::{FontContext, LayoutContext};
use taffy::{AvailableSpace, Dimension as TaffyDim, Size as TaffySize};

use crate::element::font_fetch::FontFetchTracker;
use crate::element::id::ElementId;
use crate::element::kind::ElementKind;
use crate::element::style::StyleProp;
use crate::element::taffy_bridge::{self, MeasureCtx};
use crate::element::inline_text;
use crate::element::taffy_projection::{TaffyProjection, TraversalStep};
use crate::element::text::{self, TextBrush, TextLayout};
use crate::element::tree::{Element, Event};

/// Groups the layout-computation and text-shaping state that was formerly
/// embedded directly in `ElementTree`. Callers get depth: a single `run()`
/// call drives Taffy layout, Parley text shaping, font-dirty propagation,
/// FetchFont event emission, and layout-cache population.
pub struct LayoutPass {
    pub(crate) projection: TaffyProjection,
    pub(crate) font_cx: FontContext,
    pub(crate) layout_cx: LayoutContext<TextBrush>,
    /// On-demand font-fetch state (issue #343): suppresses duplicate `FetchFont`
    /// events, reopens families whose fetch failed, and gives up after a finite
    /// retry budget.
    pub(crate) font_fetches: FontFetchTracker,
    /// Wall-clock millis of the last cursor-visibility toggle (ADR-0032).
    pub(crate) last_cursor_toggle_ms: Option<f64>,
    /// Absolute bounding rects (x, y, w, h) per element, refreshed by each `settle`.
    /// Read only through `geometry` / `has_geometry` (issue #308 / §5).
    layout_cache: HashMap<ElementId, (f32, f32, f32, f32)>,
}

impl LayoutPass {
    pub fn new() -> Self {
        let mut font_cx = FontContext::new();
        init_bundled_fonts(&mut font_cx);
        Self {
            projection: TaffyProjection::new(),
            font_cx,
            layout_cx: LayoutContext::new(),
            font_fetches: FontFetchTracker::new(),
            last_cursor_toggle_ms: None,
            layout_cache: HashMap::new(),
        }
    }

    /// Set half of the reduced layout interface (issue #308 / §5): bridge-convert
    /// a layout `StyleProp` into `layout_style` (owned by the document tree) and
    /// push the result onto the derived Taffy node, which marks it layout-dirty.
    /// Returns `false` for non-layout props so the caller routes them to Visual.
    ///
    /// Folds the former `convert → set → mark` sequence behind one call; the
    /// document tree stays the owner (it owns `layout_style`), the Taffy node is
    /// re-derived from it.
    pub(crate) fn set_layout_prop(
        &mut self,
        id: ElementId,
        layout_style: &mut taffy::Style,
        prop: &StyleProp,
    ) -> bool {
        if !taffy_bridge::apply_to_style(layout_style, prop) {
            return false;
        }
        self.projection.set_style(id, layout_style.clone());
        true
    }

    /// Settle half of the reduced layout interface (issue #308 / §5): reconcile
    /// the derived Taffy projection against the (owner) element tree, run Taffy
    /// layout + Parley shaping, refresh the absolute-geometry cache, and return
    /// the set of elements whose box geometry changed (or appeared) this pass.
    ///
    /// Folds the former `reconcile → compute → cache → geometry diff` sequence
    /// behind one call. The returned diff is the bridge the scene lowering
    /// consumes so reflowed-but-otherwise-clean boxes re-lower instead of
    /// painting stale geometry. `structure_dirty` / `shape_dirty` / `fonts_dirty`
    /// are owned by `ElementEngine` (ADR-0075); this drains them.
    pub(crate) fn settle(
        &mut self,
        elements: &mut HashMap<ElementId, Element>,
        root: ElementId,
        viewport: (f32, f32),
        event_queue: &mut Vec<Event>,
        structure_dirty: &mut HashSet<ElementId>,
        shape_dirty: &mut HashSet<ElementId>,
        fonts_dirty: &mut bool,
    ) -> HashSet<ElementId> {
        self.projection.reconcile(&*elements, root, structure_dirty);
        self.compute(elements, root, viewport, event_queue, shape_dirty, fonts_dirty);
        // Snapshot the previous absolute geometry before rebuilding, then diff:
        // any element whose box `(x, y, w, h)` moved/resized (or newly appeared)
        // lands in the returned set. A flex reflow from an insert/select ripples
        // up to ancestors and sideways to siblings that are never structure/visual
        // dirty on their own; absolute coords mean every moved descendant lands in
        // the diff independently, so per-id re-lowering is sufficient.
        let previous = std::mem::take(&mut self.layout_cache);
        cache_layout(elements, &self.projection, root, 0.0, 0.0, &mut self.layout_cache);
        let mut geometry_dirty = HashSet::new();
        for (&id, geometry) in &self.layout_cache {
            if previous.get(&id) != Some(geometry) {
                geometry_dirty.insert(id);
            }
        }
        geometry_dirty
    }

    /// Test seam (ADR-0042): rebuild the font collection to mirror the WASM
    /// runtime — no system fonts, with `default_font` registered as the default
    /// family + sans-serif generic. Lets font-fetch tests drive the real
    /// `.notdef → FetchFont → register_font` path without depending on
    /// host-installed fonts (`system_fonts: false`).
    pub(crate) fn set_wasm_like_font_context(&mut self, default_font: Vec<u8>) {
        use fontique::{Collection, CollectionOptions, FontInfoOverride, GenericFamily};
        self.font_cx.collection = Collection::new(CollectionOptions {
            system_fonts: false,
            ..Default::default()
        });
        let blob = Blob::new(Arc::new(default_font));
        let override_info = FontInfoOverride {
            family_name: Some(text::DEFAULT_FONT_FAMILY),
            ..Default::default()
        };
        let registered = self.font_cx.collection.register_fonts(blob, Some(override_info));
        let ids: Vec<_> = registered.into_iter().map(|(id, _)| id).collect();
        if !ids.is_empty() {
            self.font_cx
                .collection
                .set_generic_families(GenericFamily::SansSerif, ids.into_iter());
        }
        self.font_fetches = FontFetchTracker::new();
    }

    /// Geometry-query side of the reduced layout interface (issue #308 / §5):
    /// the absolute box rect `(x, y, w, h)` from the latest `settle`, or `None`
    /// for elements without box geometry (e.g. inline text elements).
    pub(crate) fn geometry(&self, id: ElementId) -> Option<(f32, f32, f32, f32)> {
        self.layout_cache.get(&id).copied()
    }

    /// True once at least one `settle` has produced box geometry.
    pub(crate) fn has_geometry(&self) -> bool {
        !self.layout_cache.is_empty()
    }

    /// Toggle the focused element's cursor every 500 ms (ADR-0032).
    /// No-op when nothing is focused or the interval hasn't elapsed.
    pub(crate) fn advance_cursor_blink(
        &mut self,
        elements: &mut HashMap<ElementId, Element>,
        focused_element: Option<ElementId>,
        timestamp_ms: f64,
    ) -> Option<ElementId> {
        let focused = match focused_element {
            Some(id) => id,
            None => return None,
        };
        match self.last_cursor_toggle_ms {
            None => {
                // First frame after focus: show cursor and start the clock.
                self.last_cursor_toggle_ms = Some(timestamp_ms);
                if let Some(el) = elements.get_mut(&focused) {
                    el.cursor_visible = true;
                }
                Some(focused)
            }
            Some(prev) if timestamp_ms - prev >= 500.0 => {
                self.last_cursor_toggle_ms = Some(timestamp_ms);
                if let Some(el) = elements.get_mut(&focused) {
                    el.cursor_visible = !el.cursor_visible;
                }
                Some(focused)
            }
            _ => None,
        }
    }

    /// Resolve `shape_dirty`/`fonts_dirty` and run Taffy layout + Parley shaping.
    /// `shape_dirty`/`fonts_dirty` are owned by `ElementEngine` (ADR-0075).
    fn compute(
        &mut self,
        elements: &mut HashMap<ElementId, Element>,
        root: ElementId,
        viewport: (f32, f32),
        event_queue: &mut Vec<Event>,
        shape_dirty: &mut HashSet<ElementId>,
        fonts_dirty: &mut bool,
    ) {
        // When a new font was registered, invalidate all text layouts so they
        // are re-shaped with the new font data on this pass.
        for &id in shape_dirty.iter() {
            if let Some(el) = elements.get_mut(&id) {
                el.text_layout = None;
            }
        }

        if *fonts_dirty {
            *fonts_dirty = false;
            let text_ids: Vec<ElementId> = elements
                .iter()
                .filter_map(|(id, el)| {
                    if el.kind.is_text_like() {
                        Some(*id)
                    } else {
                        None
                    }
                })
                .collect();
            for id in text_ids {
                if let Some(el) = elements.get_mut(&id) {
                    el.text_layout = None;
                    el.content_layout = None;
                    self.projection.mark_dirty(id);
                }
            }
        }

        let root_taffy = match self.projection.node_id(root) {
            Some(n) => n,
            None => return,
        };
        let root_source_size = elements[&root].layout_style.size;

        // Pin root dimensions to viewport when the app asked for Auto/Percent.
        // The root has no containing block, so Percent would collapse to min-content
        // without this. Use layout_style (author intent), not the current Taffy style:
        // after the first pin the Taffy node holds a definite Length that must still
        // track viewport changes on resize.
        // Explicit px Length values set on the root are left untouched.
        if let Ok(mut style) = self.projection.taffy.style(root_taffy).cloned() {
            let mut changed = false;
            if !matches!(root_source_size.width, TaffyDim::Length(_)) {
                let pinned = TaffyDim::Length(viewport.0);
                if style.size.width != pinned {
                    style.size.width = pinned;
                    changed = true;
                }
            }
            if !matches!(root_source_size.height, TaffyDim::Length(_)) {
                let pinned = TaffyDim::Length(viewport.1);
                if style.size.height != pinned {
                    style.size.height = pinned;
                    changed = true;
                }
            }
            if changed {
                let _ = self.projection.taffy.set_style(root_taffy, style);
            }
        }

        let available = TaffySize {
            width: AvailableSpace::Definite(viewport.0),
            height: AvailableSpace::Definite(viewport.1),
        };

        let LayoutPass {
            projection,
            font_cx,
            layout_cx,
            font_fetches,
            ..
        } = self;

        // Two-pass: stash text layouts produced inside the measure closure,
        // then drain them back onto the elements once compute_layout returns.
        let mut pending: HashMap<ElementId, TextLayout> = HashMap::new();
        {
            let taffy = &mut projection.taffy;
            let _ = taffy.compute_layout_with_measure(
                root_taffy,
                available,
                |known_dims, available_space, _node_id, ctx, _style| {
                    let eid = match ctx {
                        Some(MeasureCtx::Text(eid)) => *eid,
                        _ => return TaffySize::ZERO,
                    };
                    if elements.get(&eid).is_none() {
                        return TaffySize::ZERO;
                    }
                    let max_advance = match known_dims.width {
                        Some(w) => Some(w),
                        None => match available_space.width {
                            AvailableSpace::Definite(w) => Some(w),
                            AvailableSpace::MaxContent => None,
                            AvailableSpace::MinContent => Some(0.0),
                        },
                    };
                    let layout = inline_text::shape(
                        elements,
                        eid,
                        max_advance,
                        font_cx,
                        layout_cx,
                        viewport,
                    );
                    if layout.text.is_empty() {
                        return TaffySize::ZERO;
                    }
                    let size = TaffySize {
                        width: layout.layout.width(),
                        height: layout.layout.height(),
                    };
                    pending.insert(eid, layout);
                    size
                },
            );
        }

        for (eid, mut layout) in pending {
            // Re-stamp the source text onto each lowered run so HTML mode can
            // place it back into a DOM text node.
            let src: Arc<str> = layout.text.clone();
            for run in &mut layout.runs {
                if let Some(rd) = Arc::get_mut(run) {
                    rd.text = src.clone();
                }
            }
            for &fam in &layout.missing_families {
                if font_fetches.should_request(fam) {
                    font_fetches.mark_requested(fam);
                    event_queue.push(Event::FetchFont {
                        family: fam.to_string(),
                    });
                }
            }
            // Proactively fetch named fonts: Latin fonts produce no .notdef glyphs
            // so script-based detection never fires for them. If the resolved family
            // is not yet in the fontique collection, request it now so the next
            // register_font() → fonts_dirty cycle will re-shape with the real font.
            if let Some(el) = elements.get(&eid) {
                if let Some(ref fam) = el.visual.font_family {
                    let resolved = text::resolve_generic_family(fam);
                    if resolved != text::DEFAULT_FONT_FAMILY
                        && font_fetches.should_request(resolved)
                        && font_cx.collection.family_id(resolved).is_none()
                    {
                        font_fetches.mark_requested(resolved);
                        event_queue.push(Event::FetchFont {
                            family: resolved.to_string(),
                        });
                    }
                }
            }
            if let Some(el) = elements.get_mut(&eid) {
                el.text_layout = Some(layout);
            }
            shape_dirty.remove(&eid);
        }

        // Build content layouts for TextInput elements (used for Canvas-mode rendering + cursor).
        let textinput_ids: Vec<ElementId> = elements
            .iter()
            .filter_map(|(id, el)| {
                if el.kind == ElementKind::TextInput {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();

        for eid in textinput_ids {
            let (display_text, font_size, font_weight, font_style) = {
                let el = match elements.get(&eid) {
                    Some(e) => e,
                    None => continue,
                };
                let ambient = crate::element::ambient_defaults::ambient_at(elements, eid);
                let text = el
                    .edit
                    .as_ref()
                    .map(|edit| edit.display_text())
                    .unwrap_or_default();
                (
                    text,
                    el.visual.font_size.unwrap_or(ambient.font_size),
                    el.visual.font_weight.or(ambient.font_weight),
                    el.visual.font_style,
                )
            };

            let (max_advance, font_family) = {
                let ambient = crate::element::ambient_defaults::ambient_at(elements, eid);
                let el = elements.get(&eid).map(|e| {
                    (
                        projection.node_id(eid).and_then(|n| {
                            projection
                                .taffy
                                .layout(n)
                                .ok()
                                .map(|l| l.content_box_width())
                        }),
                        e.visual
                            .font_family
                            .clone()
                            .or(ambient.font_family.clone()),
                    )
                });
                el.map(|(a, f)| (a, f)).unwrap_or((None, None))
            };

            let is_placeholder = display_text.is_empty();
            let text_to_layout: Option<String> = if is_placeholder {
                elements
                    .get(&eid)
                    .and_then(|el| el.text.clone())
                    .filter(|t| !t.is_empty())
            } else {
                Some(display_text)
            };

            if let Some(text) = text_to_layout {
                let layout = text::build_text_layout(
                    font_cx,
                    layout_cx,
                    &text,
                    font_size,
                    max_advance,
                    font_family.as_deref(),
                    font_weight,
                    font_style,
                );

                for &fam in &layout.missing_families {
                    if font_fetches.should_request(fam) {
                        font_fetches.mark_requested(fam);
                        event_queue.push(Event::FetchFont {
                            family: fam.to_string(),
                        });
                    }
                }
                if let Some(ref fam) = font_family {
                    let resolved = text::resolve_generic_family(fam);
                    if resolved != text::DEFAULT_FONT_FAMILY
                        && font_fetches.should_request(resolved)
                        && font_cx.collection.family_id(resolved).is_none()
                    {
                        font_fetches.mark_requested(resolved);
                        event_queue.push(Event::FetchFont {
                            family: resolved.to_string(),
                        });
                    }
                }
                if let Some(el) = elements.get_mut(&eid) {
                    if is_placeholder {
                        el.content_layout = None;
                        el.text_layout = Some(layout);
                    } else {
                        el.content_layout = Some(layout);
                        el.text_layout = None;
                        if let Some(edit) = el.edit.as_mut() {
                            edit.cursor_byte_index = edit.text_content.len();
                        }
                    }
                }
            } else if let Some(el) = elements.get_mut(&eid) {
                el.content_layout = None;
                el.text_layout = None;
            }
        }
    }
}

impl Default for LayoutPass {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::kind::ElementKind;
    use crate::element::style::{Dimension, StyleProp};
    use crate::element::tree::Visual;

    fn view(parent: Option<ElementId>, children: Vec<ElementId>) -> Element {
        Element {
            kind: ElementKind::View,
            parent,
            children,
            layout_style: taffy::Style::default(),
            visual: Visual::default(),
            text: None,
            src: None,
            text_layout: None,
            transform: None,
            scroll_offset: (0.0, 0.0),
            src_image: None,
            edit: None,
            cursor_visible: false,
            content_layout: None,
            aria_label: None,
            role: None,
            pseudo_styles: Default::default(),
            disabled: false,
            selectable: false,
            viewport_variants: Vec::new(),
        }
    }

    /// The reduced layout interface: a caller sets layout props and settles,
    /// then reads geometry — without touching bridge conversion, reconcile,
    /// compute, or the layout cache directly (issue #308 / §5).
    #[test]
    fn set_layout_prop_then_settle_then_geometry_lays_out_child() {
        let mut layout = LayoutPass::new();
        let root_id = ElementId::from_u64(1);
        let child_id = ElementId::from_u64(2);
        let mut elements = HashMap::new();
        elements.insert(root_id, view(None, vec![child_id]));
        elements.insert(child_id, view(Some(root_id), Vec::new()));

        {
            let child = elements.get_mut(&child_id).unwrap();
            assert!(layout.set_layout_prop(
                child_id,
                &mut child.layout_style,
                &StyleProp::Width(Dimension::px(80.0)),
            ));
            assert!(layout.set_layout_prop(
                child_id,
                &mut child.layout_style,
                &StyleProp::Height(Dimension::px(40.0)),
            ));
        }

        let mut structure_dirty = HashSet::new();
        let mut shape_dirty = HashSet::new();
        let mut fonts_dirty = false;
        let mut events = Vec::new();
        layout.settle(
            &mut elements,
            root_id,
            (300.0, 200.0),
            &mut events,
            &mut structure_dirty,
            &mut shape_dirty,
            &mut fonts_dirty,
        );

        let rect = layout.geometry(child_id).expect("child must have geometry");
        assert!((rect.2 - 80.0).abs() < 0.5, "width was {}", rect.2);
        assert!((rect.3 - 40.0).abs() < 0.5, "height was {}", rect.3);
    }

    /// `settle` returns the geometry diff (boxes that moved/resized/appeared)
    /// so the caller need not snapshot and compare the layout cache itself.
    #[test]
    fn settle_reports_geometry_diff_only_for_changed_boxes() {
        let mut layout = LayoutPass::new();
        let root_id = ElementId::from_u64(1);
        let child_id = ElementId::from_u64(2);
        let mut elements = HashMap::new();
        elements.insert(root_id, view(None, vec![child_id]));
        elements.insert(child_id, view(Some(root_id), Vec::new()));
        {
            let child = elements.get_mut(&child_id).unwrap();
            layout.set_layout_prop(
                child_id,
                &mut child.layout_style,
                &StyleProp::Width(Dimension::px(80.0)),
            );
            layout.set_layout_prop(
                child_id,
                &mut child.layout_style,
                &StyleProp::Height(Dimension::px(40.0)),
            );
        }

        let mut structure_dirty = HashSet::new();
        let mut shape_dirty = HashSet::new();
        let mut fonts_dirty = false;
        let mut events = Vec::new();
        let viewport = (300.0, 200.0);

        // First settle: every box newly appears in the diff.
        let appeared = layout.settle(
            &mut elements, root_id, viewport, &mut events,
            &mut structure_dirty, &mut shape_dirty, &mut fonts_dirty,
        );
        assert!(appeared.contains(&child_id));

        // Re-settle with no change: a stable layout reports an empty diff.
        let stable = layout.settle(
            &mut elements, root_id, viewport, &mut events,
            &mut structure_dirty, &mut shape_dirty, &mut fonts_dirty,
        );
        assert!(stable.is_empty(), "stable layout must report no geometry diff");

        // Resize through the reduced set interface, then settle.
        {
            let child = elements.get_mut(&child_id).unwrap();
            layout.set_layout_prop(
                child_id,
                &mut child.layout_style,
                &StyleProp::Height(Dimension::px(90.0)),
            );
        }
        let resized = layout.settle(
            &mut elements, root_id, viewport, &mut events,
            &mut structure_dirty, &mut shape_dirty, &mut fonts_dirty,
        );
        assert!(resized.contains(&child_id), "resized box must be in geometry diff");
        let rect = layout.geometry(child_id).expect("child geometry");
        assert!((rect.3 - 90.0).abs() < 0.5, "height was {}", rect.3);
    }

    /// The set interface bridge-converts only layout props; a visual prop is
    /// rejected (returns false) and leaves `layout_style` untouched, so the
    /// caller can route it to Visual instead.
    #[test]
    fn set_layout_prop_rejects_non_layout_prop() {
        let mut layout = LayoutPass::new();
        let id = ElementId::from_u64(1);
        let mut style = taffy::Style::default();
        let before = style.clone();

        let applied = layout.set_layout_prop(id, &mut style, &StyleProp::Opacity(0.5));

        assert!(!applied, "visual prop must not be accepted by the layout seam");
        assert_eq!(style, before, "non-layout prop must not mutate layout_style");
    }
}

fn init_bundled_fonts(font_cx: &mut FontContext) {
    use fontique::{FontInfoOverride, GenericFamily};

    static NOTO_SANS_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSansJP.ttf");

    let blob = Blob::new(Arc::new(NOTO_SANS_BYTES));
    let override_info = FontInfoOverride {
        family_name: Some(text::DEFAULT_FONT_FAMILY),
        ..Default::default()
    };
    let registered = font_cx.collection.register_fonts(blob, Some(override_info));
    let family_ids: Vec<_> = registered.into_iter().map(|(id, _)| id).collect();
    if !family_ids.is_empty() {
        font_cx
            .collection
            .set_generic_families(GenericFamily::SansSerif, family_ids.into_iter());
    }
}

fn cache_layout(
    elements: &HashMap<ElementId, Element>,
    projection: &TaffyProjection,
    id: ElementId,
    ox: f32,
    oy: f32,
    cache: &mut HashMap<ElementId, (f32, f32, f32, f32)>,
) {
    match projection.traversal_step(elements, id) {
        // Inline text elements have no box geometry; still walk descendants.
        Some(TraversalStep::Skip(el)) => {
            for &child in &el.children {
                cache_layout(elements, projection, child, ox, oy, cache);
            }
        }
        Some(TraversalStep::Visit(taffy_node, el)) => {
            let layout = match projection.taffy.layout(taffy_node) {
                Ok(l) => l,
                Err(_) => return,
            };
            let x = ox + layout.location.x;
            let y = oy + layout.location.y;
            cache.insert(id, (x, y, layout.size.width, layout.size.height));
            for &child in &el.children {
                cache_layout(elements, projection, child, x, y, cache);
            }
        }
        None => {}
    }
}
