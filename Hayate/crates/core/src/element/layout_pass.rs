use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use linebender_resource_handle::Blob;
use parley::{FontContext, LayoutContext};
use taffy::{AvailableSpace, Dimension as TaffyDim, Size as TaffySize};

use crate::element::id::ElementId;
use crate::element::kind::ElementKind;
use crate::element::taffy_bridge::MeasureCtx;
use crate::element::inline_text;
use crate::element::taffy_projection::TaffyProjection;
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
    /// Set by `register_font`; cleared at the start of the next `run`.
    /// Causes all text elements to be re-shaped with the newly registered font.
    pub(crate) fonts_dirty: bool,
    /// Family names already requested via `FetchFont` but not yet loaded.
    /// Prevents duplicate events for the same family across frames.
    pub(crate) pending_font_fetches: HashSet<String>,
    /// Wall-clock millis of the last cursor-visibility toggle (ADR-0032).
    pub(crate) last_cursor_toggle_ms: Option<f64>,
    /// Absolute bounding rects (x, y, w, h) per element, refreshed after each `run`.
    pub(crate) layout_cache: HashMap<ElementId, (f32, f32, f32, f32)>,
    /// IFC roots needing Parley re-compose before layout (ADR-0063).
    pub(crate) shape_dirty: HashSet<ElementId>,
}

impl LayoutPass {
    pub fn new() -> Self {
        let mut font_cx = FontContext::new();
        init_bundled_fonts(&mut font_cx);
        Self {
            projection: TaffyProjection::new(),
            font_cx,
            layout_cx: LayoutContext::new(),
            fonts_dirty: false,
            pending_font_fetches: HashSet::new(),
            last_cursor_toggle_ms: None,
            layout_cache: HashMap::new(),
            shape_dirty: HashSet::new(),
        }
    }

    /// Run a full layout pass: Taffy layout + Parley text shaping + layout-cache update.
    /// Emits `FetchFont` events into `event_queue` for any missing font families.
    pub(crate) fn run(
        &mut self,
        elements: &mut HashMap<ElementId, Element>,
        root: ElementId,
        viewport: (f32, f32),
        event_queue: &mut Vec<Event>,
    ) {
        self.projection.reconcile(&*elements, root);
        self.compute(elements, root, viewport, event_queue);
        self.layout_cache.clear();
        cache_layout(
            elements,
            &self.projection,
            root,
            0.0,
            0.0,
            &mut self.layout_cache,
        );
    }

    pub(crate) fn mark_structure_dirty(&mut self, id: ElementId) {
        self.projection.mark_structure_dirty(id);
    }

    pub(crate) fn mark_shape_dirty(&mut self, id: ElementId) {
        self.shape_dirty.insert(id);
        self.projection.mark_dirty(id);
    }

    /// Toggle the focused element's cursor every 500 ms (ADR-0032).
    /// No-op when nothing is focused or the interval hasn't elapsed.
    pub(crate) fn advance_cursor_blink(
        &mut self,
        elements: &mut HashMap<ElementId, Element>,
        focused_element: Option<ElementId>,
        timestamp_ms: f64,
    ) {
        let focused = match focused_element {
            Some(id) => id,
            None => return,
        };
        match self.last_cursor_toggle_ms {
            None => {
                // First frame after focus: show cursor and start the clock.
                self.last_cursor_toggle_ms = Some(timestamp_ms);
                if let Some(el) = elements.get_mut(&focused) {
                    el.cursor_visible = true;
                }
            }
            Some(prev) if timestamp_ms - prev >= 500.0 => {
                self.last_cursor_toggle_ms = Some(timestamp_ms);
                if let Some(el) = elements.get_mut(&focused) {
                    el.cursor_visible = !el.cursor_visible;
                }
            }
            _ => {}
        }
    }

    fn compute(
        &mut self,
        elements: &mut HashMap<ElementId, Element>,
        root: ElementId,
        viewport: (f32, f32),
        event_queue: &mut Vec<Event>,
    ) {
        // When a new font was registered, invalidate all text layouts so they
        // are re-shaped with the new font data on this pass.
        for &id in self.shape_dirty.iter() {
            if let Some(el) = elements.get_mut(&id) {
                el.text_layout = None;
            }
        }

        if self.fonts_dirty {
            self.fonts_dirty = false;
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
            pending_font_fetches,
            shape_dirty,
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
                    let layout =
                        inline_text::shape(elements, eid, max_advance, font_cx, layout_cx);
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
                if !pending_font_fetches.contains(fam) {
                    pending_font_fetches.insert(fam.to_string());
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
                        && !pending_font_fetches.contains(resolved)
                        && font_cx.collection.family_id(resolved).is_none()
                    {
                        let owned = resolved.to_string();
                        pending_font_fetches.insert(owned.clone());
                        event_queue.push(Event::FetchFont { family: owned });
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
            let (display_text, font_size, font_weight) = {
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
                )
            };

            if display_text.is_empty() {
                if let Some(el) = elements.get_mut(&eid) {
                    el.content_layout = None;
                }
                continue;
            }

            let (max_advance, font_family) = {
                let ambient = crate::element::ambient_defaults::ambient_at(elements, eid);
                let el = elements.get(&eid).map(|e| {
                    (
                        projection
                            .node_id(eid)
                            .and_then(|n| projection.taffy.layout(n).ok().map(|l| l.size.width)),
                        e.visual
                            .font_family
                            .clone()
                            .or(ambient.font_family.clone()),
                    )
                });
                el.map(|(a, f)| (a, f)).unwrap_or((None, None))
            };
            let content_layout = text::build_text_layout(
                font_cx,
                layout_cx,
                &display_text,
                font_size,
                max_advance,
                font_family.as_deref(),
                font_weight,
            );

            for &fam in &content_layout.missing_families {
                if !pending_font_fetches.contains(fam) {
                    pending_font_fetches.insert(fam.to_string());
                    event_queue.push(Event::FetchFont {
                        family: fam.to_string(),
                    });
                }
            }
            if let Some(ref fam) = font_family {
                let resolved = text::resolve_generic_family(fam);
                if resolved != text::DEFAULT_FONT_FAMILY
                    && !pending_font_fetches.contains(resolved)
                    && font_cx.collection.family_id(resolved).is_none()
                {
                    let owned = resolved.to_string();
                    pending_font_fetches.insert(owned.clone());
                    event_queue.push(Event::FetchFont { family: owned });
                }
            }
            if let Some(el) = elements.get_mut(&eid) {
                el.content_layout = Some(content_layout);
                if let Some(edit) = el.edit.as_mut() {
                    edit.cursor_byte_index = edit.text_content.len();
                }
            }
        }
    }
}

impl Default for LayoutPass {
    fn default() -> Self {
        Self::new()
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

pub(crate) fn cache_layout(
    elements: &HashMap<ElementId, Element>,
    projection: &TaffyProjection,
    id: ElementId,
    ox: f32,
    oy: f32,
    cache: &mut HashMap<ElementId, (f32, f32, f32, f32)>,
) {
    let el = match elements.get(&id) {
        Some(e) => e,
        None => return,
    };
    let taffy_node = match projection.node_id(id) {
        Some(n) => n,
        None => {
            // Inline text elements have no box geometry; still walk descendants.
            for &child in &el.children {
                cache_layout(elements, projection, child, ox, oy, cache);
            }
            return;
        }
    };
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
