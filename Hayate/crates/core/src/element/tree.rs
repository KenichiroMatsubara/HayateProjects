use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use fontique::FontInfoOverride;
use linebender_resource_handle::Blob;
use crate::color::Color;
use crate::element::document_runtime::{self, DocumentRuntime, EventDelivery, ListenerId};
use crate::element::edit_state::EditState;
use crate::element::engine::ElementEngine;
use crate::element::effective_visual::{self, child_inherited_context};
use crate::element::ime_bridge::CharacterBounds;
use crate::element::event_spec::DocumentEventKind;

pub use crate::element::event_spec::Event;
use crate::element::id::ElementId;
use crate::element::kind::ElementKind;
use crate::element::inline_text::{self, ifc_root, is_ifc_root};
use crate::element::layout_pass::LayoutPass;
use crate::element::taffy_projection::{TaffyProjection, TraversalStep};
use crate::element::pseudo_state::{
    self, diff_hover_sets, hover_set_for_hit, InteractionSnapshot, PseudoState, PseudoStyles,
};
use crate::element::scene_build;
use crate::element::scene_lowering::{collect_lowering_dirty, SceneLowering};
use crate::element::style::{
    BorderStyleValue, CursorValue, FontStyleValue, OverflowValue, Shadow, StyleProp, StylePropKind,
    TextDecorationValue, TextOverflowValue, TransitionTimingValue, ViewportCondition,
};
use crate::element::taffy_bridge;
use crate::element::text;
use crate::element::visual_invalidation::{
    self, DirtyKind, ElementContext, VisualInvalidationReach,
};
use crate::node::SceneGraph;
use crate::render::RenderImage;

#[derive(Clone, Debug)]
pub struct Visual {
    pub background_color: Option<Color>,
    pub opacity: f32,
    pub border_radius: f32,
    pub border_width: f32,
    pub border_color: Option<Color>,
    pub border_style: BorderStyleValue,
    /// Ordered box-shadow layers (ADR-0095); empty means no shadow. Top layer
    /// first, matching CSS paint order.
    pub box_shadow: Vec<Shadow>,
    /// Child-overflow handling (issue #206). `Hidden` clips children to the
    /// element's (optionally rounded) border box; `Visible` is the default.
    pub overflow: OverflowValue,
    /// Max number of text lines before truncation (issue #207). `None` = unbounded.
    /// The sole trigger for text truncation; `text_overflow` is inert without it.
    pub max_lines: Option<u32>,
    /// How the last visible line is truncated when `max_lines` is exceeded.
    pub text_overflow: TextOverflowValue,
    pub text_color: Option<Color>,
    pub font_size: Option<f32>,
    pub font_weight: Option<f32>,
    pub font_style: Option<FontStyleValue>,
    pub text_decoration: Option<TextDecorationValue>,
    /// Pointer cursor appearance (ADR-0088). `None` resolves to `Default`.
    pub cursor: Option<CursorValue>,
    pub z_index: i32,
    /// Custom font-family name registered via `register_font`.
    pub font_family: Option<String>,
    /// Ambient default text style (block-penetrating, ADR-0065 ch2).
    pub default_color: Option<Color>,
    pub default_font_size: Option<f32>,
    pub default_font_weight: Option<f32>,
    pub default_font_family: Option<String>,
    /// Pseudo-state transition duration in milliseconds (ADR-0089, issue #209).
    /// `0.0` (the default) means pseudo-state switches apply instantly.
    pub transition_duration: f32,
    /// Easing curve used while interpolating a pseudo-state transition.
    pub transition_timing: TransitionTimingValue,
}

impl Default for Visual {
    fn default() -> Self {
        Self {
            background_color: None,
            opacity: 1.0,
            border_radius: 0.0,
            border_width: 0.0,
            border_color: None,
            border_style: BorderStyleValue::None,
            box_shadow: Vec::new(),
            overflow: OverflowValue::Visible,
            max_lines: None,
            text_overflow: TextOverflowValue::Clip,
            text_color: None,
            font_size: None,
            font_weight: None,
            font_style: None,
            text_decoration: None,
            cursor: None,
            z_index: 0,
            font_family: None,
            default_color: None,
            default_font_size: None,
            default_font_weight: None,
            default_font_family: None,
            transition_duration: 0.0,
            transition_timing: TransitionTimingValue::Ease,
        }
    }
}

pub(crate) struct Element {
    pub kind: ElementKind,
    pub parent: Option<ElementId>,
    pub children: Vec<ElementId>,
    pub layout_style: taffy::Style,
    pub visual: Visual,
    pub text: Option<String>,
    pub src: Option<String>,
    pub text_layout: Option<crate::element::text::TextLayout>,
    /// Optional affine transform applied on top of layout (kurbo coefficients [a,b,c,d,e,f]).
    pub transform: Option<[f64; 6]>,
    /// Scroll offset for ScrollView elements (x, y in pixels).
    pub scroll_offset: (f32, f32),
    /// Loaded image data for Image elements (populated by the adapter after async fetch).
    pub src_image: Option<Arc<RenderImage>>,
    /// Text-input edit model (TextInput only). ADR-0069.
    pub edit: Option<EditState>,
    /// Whether the cursor should be drawn (true when the element is focused).
    pub cursor_visible: bool,
    /// Pre-built Parley layout of text_content + preedit, rebuilt each render pass.
    pub content_layout: Option<crate::element::text::TextLayout>,
    /// ARIA label for screen readers.
    pub aria_label: Option<String>,
    /// ARIA role (e.g. "button", "listitem"). None uses the implicit role.
    pub role: Option<String>,
    /// Hayate CSS pseudo-class overrides (`:hover` / `:active` / `:focus`).
    pub pseudo_styles: PseudoStyles,
    /// When true, suppresses hit-testing and interaction (ADR-0071).
    pub disabled: bool,
    /// When true, this element establishes a Selection Region: text under it can
    /// be selected by pointer drag, bounded by the nearest selectable ancestor
    /// (ADR-0097 / ADR-0071 closed typed property, same shape as `disabled`).
    pub selectable: bool,
    /// Viewport-conditional style overrides, one variant per property (ADR-0081).
    pub viewport_variants: Vec<(ViewportCondition, StyleProp)>,
}

/// Events emitted by input wiring and drained by `poll_events`.
/// Fully-resolved per-element state after layout, keyed by stable ElementId.
/// Used by HTML Mode to update DOM elements without going through SceneGraph.
#[derive(Clone, Debug)]
pub struct ResolvedElement {
    pub kind: ElementKind,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub background_color: Option<Color>,
    pub opacity: f32,
    pub border_radius: f32,
    pub border_width: f32,
    pub border_color: Option<Color>,
    pub text_color: Option<Color>,
    pub font_size: Option<f32>,
    pub font_weight: Option<f32>,
    pub z_index: i32,
    pub text: Option<String>,
    pub src: Option<String>,
    /// Current value for TextInput elements (text_content + active preedit, combined for display).
    pub text_content: Option<String>,
    pub font_family: Option<String>,
    pub aria_label: Option<String>,
    pub role: Option<String>,
}

pub struct ElementTree {
    pub(crate) elements: HashMap<ElementId, Element>,
    pub(crate) root: Option<ElementId>,
    /// Layout-computation and text-shaping state. Grouped here so callers
    /// cross one seam (`commit_frame()`) instead of touching Taffy, Parley,
    /// font dirty state, and cursor timing directly.
    pub(crate) layout: LayoutPass,
    /// Dirty-tracking sets and frame-resolution logic (ADR-0075).
    pub(crate) engine: ElementEngine,
    pub(crate) viewport: (f32, f32),
    pub(crate) scene_cache: SceneGraph,
    pub(crate) scene_lowering: SceneLowering,
    pub(crate) event_queue: Vec<Event>,
    /// Element that owns the text-input cursor blink. Tracked here (not in the
    /// adapter) so `render(timestamp_ms)` can advance the blink itself per ADR-0032.
    pub(crate) focused_element: Option<ElementId>,
    /// Elements matching CSS `:hover` (self or descendant under pointer).
    pub(crate) hovered_elements: HashSet<ElementId>,
    pub(crate) active_element: Option<ElementId>,
    /// The single document-wide text selection, if any (ADR-0097). At most one
    /// is active across the whole document.
    pub(crate) selection: Option<crate::element::selection::Selection>,
    /// True while a pointer-down inside a Selection Region is driving a drag
    /// selection (the active-session capture extended to selection, ADR-0097).
    pub(crate) selection_drag: bool,
    /// Multi-click tracking for word/paragraph gestures (#267): the last
    /// pointer-down position and how many presses have landed near it. The
    /// adapter's OS-level double-click timing is re-derived here by proximity:
    /// consecutive presses at the same spot cycle caret → word → paragraph.
    pub(crate) last_click_pos: Option<(f32, f32)>,
    pub(crate) click_count: u32,
    /// Last pointer position for sub-pixel move dedup (ADR-0066).
    pub(crate) last_pointer_pos: Option<(f32, f32)>,
    /// Cursor last resolved under the pointer, reported on coalesced moves (ADR-0088).
    pub(crate) last_cursor: CursorValue,
    pub(crate) runtime: DocumentRuntime,
}

impl ElementTree {
    pub fn new() -> Self {
        Self {
            elements: HashMap::new(),
            root: None,
            layout: LayoutPass::new(),
            engine: ElementEngine::new(),
            viewport: (800.0, 600.0),
            scene_cache: SceneGraph::new(),
            scene_lowering: SceneLowering::default(),
            event_queue: Vec::new(),
            focused_element: None,
            hovered_elements: HashSet::new(),
            active_element: None,
            selection: None,
            selection_drag: false,
            last_click_pos: None,
            click_count: 0,
            last_pointer_pos: None,
            last_cursor: CursorValue::Default,
            runtime: DocumentRuntime::new(),
        }
    }

    pub fn interaction_snapshot(&self) -> InteractionSnapshot {
        InteractionSnapshot {
            hovered: self.hovered_elements.clone(),
            active: self.active_element,
            focused: self.focused_element,
        }
    }

    pub fn set_viewport(&mut self, width: f32, height: f32) {
        let new_viewport = (width, height);
        if new_viewport == self.viewport {
            return;
        }
        let old_viewport = self.viewport;
        self.viewport = new_viewport;
        self.mark_viewport_dirty_on_resize(old_viewport, new_viewport);
    }

    #[cfg(test)]
    pub fn test_viewport_dirty_len(&self) -> usize {
        self.engine.viewport_dirty.len()
    }

    pub fn viewport(&self) -> (f32, f32) {
        self.viewport
    }

    pub fn root(&self) -> Option<ElementId> {
        self.root
    }

    pub fn set_root(&mut self, id: ElementId) {
        debug_assert!(self.elements.contains_key(&id), "set_root: unknown id");
        self.root = Some(id);
    }

    pub fn element_create(&mut self, id: u64, kind: ElementKind) -> ElementId {
        let id = ElementId::from_u64(id);
        let layout_style = taffy::Style::default();

        let element = Element {
            kind,
            parent: None,
            children: Vec::new(),
            layout_style,
            visual: Visual::default(),
            text: None,
            src: None,
            text_layout: None,
            transform: None,
            scroll_offset: (0.0, 0.0),
            src_image: None,
            edit: if kind == ElementKind::TextInput {
                Some(EditState::default())
            } else {
                None
            },
            cursor_visible: false,
            content_layout: None,
            aria_label: None,
            role: None,
            pseudo_styles: PseudoStyles::default(),
            disabled: false,
            selectable: false,
            viewport_variants: Vec::new(),
        };
        self.elements.insert(id, element);

        if self.root.is_none() {
            self.root = Some(id);
        }
        id
    }

    pub fn element_set_text(&mut self, id: ElementId, text: &str) {
        let el = match self.elements.get_mut(&id) {
            Some(e) => e,
            None => return,
        };
        // ADR-0058: text は text-like 要素にのみ宿る — `Text`（内容）/ `TextInput`
        // （placeholder）。`view` / `button` / `image` / `scroll-view` はテキストを
        // 子 `text` 要素で持ち、親へ集約しない。非 text 要素への set は無視する
        // （wire 駆動の外部入力なので panic せず防御的に no-op）。
        if !matches!(el.kind, ElementKind::Text | ElementKind::TextInput) {
            return;
        }
        el.text = Some(text.to_string());
        el.text_layout = None;
        self.mark_text_content_dirty(id, VisualInvalidationReach::Subtree);
    }

    pub fn element_set_src(&mut self, id: ElementId, url: &str) {
        if let Some(el) = self.elements.get_mut(&id) {
            el.src = if url.is_empty() {
                None
            } else {
                Some(url.to_string())
            };
            el.src_image = None;
        }
    }

    pub fn element_set_disabled(&mut self, id: ElementId, disabled: bool) {
        if let Some(el) = self.elements.get_mut(&id) {
            el.disabled = disabled;
        }
    }

    /// Mark (or unmark) an element as a Selection Region boundary (ADR-0097).
    pub fn element_set_selectable(&mut self, id: ElementId, selectable: bool) {
        if let Some(el) = self.elements.get_mut(&id) {
            el.selectable = selectable;
        }
    }

    /// Store decoded image data for an Image element (called by the adapter after async load).
    pub fn element_set_image(&mut self, id: ElementId, image: Arc<RenderImage>) {
        if let Some(el) = self.elements.get_mut(&id) {
            el.src_image = Some(image);
            self.engine
                .mark_visual_dirty(id, VisualInvalidationReach::SelfOnly);
        }
    }

    /// Replace the editable text content of a TextInput element.
    pub fn element_set_text_content(&mut self, id: ElementId, text: &str) {
        if let Some(edit) = self
            .elements
            .get_mut(&id)
            .and_then(|el| el.edit.as_mut())
        {
            edit.set(text);
        }
    }

    /// Append text to a TextInput's committed content.
    pub fn element_append_text_content(&mut self, id: ElementId, text: &str) {
        if let Some(edit) = self
            .elements
            .get_mut(&id)
            .and_then(|el| el.edit.as_mut())
        {
            edit.append(text);
        }
    }

    /// Remove the last Unicode scalar value from a TextInput's committed content.
    pub fn element_backspace(&mut self, id: ElementId) {
        if let Some(edit) = self
            .elements
            .get_mut(&id)
            .and_then(|el| el.edit.as_mut())
        {
            edit.backspace();
        }
    }

    /// Show or hide the insertion cursor for a TextInput element.
    pub fn element_set_cursor_visible(&mut self, id: ElementId, visible: bool) {
        if let Some(el) = self.elements.get_mut(&id) {
            el.cursor_visible = visible;
            self.engine
                .mark_visual_dirty(id, VisualInvalidationReach::SelfOnly);
        }
    }

    /// Mark `id` as the focused element. Used by `render(timestamp_ms)` to
    /// drive cursor blink internally (ADR-0032). Also shows the cursor for
    /// TextInput targets so the first frame after focus draws it solid.
    pub fn element_focus(&mut self, id: ElementId) {
        if self.focused_element == Some(id) {
            return;
        }
        if let Some(prev) = self.focused_element {
            if let Some(el) = self.elements.get_mut(&prev) {
                el.cursor_visible = false;
            }
            self.engine
                .mark_visual_dirty(prev, VisualInvalidationReach::SelfOnly);
            self.mark_pseudo_activation_dirty(prev, PseudoState::Focus);
        }
        if let Some(el) = self.elements.get_mut(&id) {
            el.cursor_visible = true;
        }
        self.engine
            .mark_visual_dirty(id, VisualInvalidationReach::SelfOnly);
        self.mark_pseudo_activation_dirty(id, PseudoState::Focus);
        self.focused_element = Some(id);
        self.layout.last_cursor_toggle_ms = None;
    }

    /// Clear focus from `id` (no-op if `id` is not currently focused).
    pub fn element_blur(&mut self, id: ElementId) {
        if self.focused_element != Some(id) {
            return;
        }
        if let Some(el) = self.elements.get_mut(&id) {
            el.cursor_visible = false;
        }
        self.engine
            .mark_visual_dirty(id, VisualInvalidationReach::SelfOnly);
        self.mark_pseudo_activation_dirty(id, PseudoState::Focus);
        self.focused_element = None;
        self.layout.last_cursor_toggle_ms = None;
    }

    /// Currently-focused element, if any.
    pub fn focused_element(&self) -> Option<ElementId> {
        self.focused_element
    }

    /// Set the font family (by name) for an element. The family must first be registered via
    /// `register_font`, or be a system font available in the default FontContext.
    pub fn element_set_font_family(&mut self, id: ElementId, family: &str) {
        if let Some(el) = self.elements.get_mut(&id) {
            el.visual.font_family = if family.is_empty() {
                None
            } else {
                Some(family.to_string())
            };
            el.text_layout = None;
            el.content_layout = None;
            self.layout.projection.mark_dirty(id);
        }
    }

    /// Set the ARIA label for screen-reader accessibility.
    pub fn element_set_aria_label(&mut self, id: ElementId, label: &str) {
        if let Some(el) = self.elements.get_mut(&id) {
            el.aria_label = if label.is_empty() {
                None
            } else {
                Some(label.to_string())
            };
        }
    }

    /// Set the ARIA role (e.g. "button", "listitem", "img"). Pass an empty string to clear.
    pub fn element_set_role(&mut self, id: ElementId, role: &str) {
        if let Some(el) = self.elements.get_mut(&id) {
            el.role = if role.is_empty() {
                None
            } else {
                Some(role.to_string())
            };
        }
    }

    /// Register a custom font from raw bytes with a given family name.
    /// After registration, the name can be used in `element_set_font_family`.
    pub fn register_font(&mut self, family_name: &str, bytes: Vec<u8>) {
        let data = Arc::new(bytes);

        // 要求名で登録（element_set_font_family による明示的な指定に対応）
        let blob = Blob::new(data.clone());
        let override_info = FontInfoOverride {
            family_name: Some(family_name),
            ..Default::default()
        };
        self.layout
            .font_cx
            .collection
            .register_fonts(blob, Some(override_info));

        // デフォルトファミリ ("Noto Sans") にも登録する。
        // build_text_layout のデフォルトスタックは常に DEFAULT_FONT_FAMILY を参照するため、
        // 追加フォントを element_set_font_family なしで全要素から自動的に使えるようにする。
        // 同名での二重登録は fontique が内部でマージするためグリフ競合は発生しない。
        if family_name != text::DEFAULT_FONT_FAMILY {
            let fallback_blob = Blob::new(data);
            let fallback_override = FontInfoOverride {
                family_name: Some(text::DEFAULT_FONT_FAMILY),
                ..Default::default()
            };
            self.layout
                .font_cx
                .collection
                .register_fonts(fallback_blob, Some(fallback_override));
        }

        self.layout.pending_font_fetches.remove(family_name);
        self.engine.mark_fonts_dirty();
    }

    /// Register a font from raw bytes using the family name(s) embedded in the
    /// font file itself. Backs the WIT `element-load-font` export.
    pub fn register_font_bytes(&mut self, bytes: Vec<u8>) {
        let blob = Blob::new(Arc::new(bytes));
        self.layout.font_cx.collection.register_fonts(blob, None);
    }

    /// Set the IME preedit for a TextInput (in-progress, not yet committed).
    pub fn element_set_preedit(&mut self, id: ElementId, preedit: &str) {
        if let Some(edit) = self
            .elements
            .get_mut(&id)
            .and_then(|el| el.edit.as_mut())
        {
            edit.set_preedit(preedit);
        }
    }

    /// Commit the current preedit text into text_content and clear the preedit.
    pub fn element_commit_preedit(&mut self, id: ElementId) {
        if let Some(edit) = self
            .elements
            .get_mut(&id)
            .and_then(|el| el.edit.as_mut())
        {
            edit.commit_preedit();
        }
    }

    /// Deliver pasted text to a TextInput: commits any active preedit, appends the
    /// pasted text, then queues a TextInput event. No-op for non-TextInput elements.
    pub fn element_paste(&mut self, id: ElementId, text: &str) {
        let pasted = text.to_string();
        let el = match self.elements.get_mut(&id) {
            Some(e) if e.kind == ElementKind::TextInput => e,
            _ => return,
        };
        let Some(edit) = el.edit.as_mut() else {
            return;
        };
        if !edit.paste(&pasted) {
            return;
        }
        self.dispatch_event(
            DocumentEventKind::TextInput,
            Event::TextInput {
                target_id: id,
                text: pasted,
            },
        );
    }

    /// Return the combined display text (text_content + any active preedit) for a TextInput.
    pub fn element_get_text_content(&self, id: ElementId) -> String {
        self.elements
            .get(&id)
            .and_then(|el| el.edit.as_ref())
            .map(|edit| edit.display_text())
            .unwrap_or_default()
    }

    /// Set a 2D affine transform on the element (6 kurbo coefficients [a,b,c,d,e,f]).
    /// Pass an empty/None to clear. The transform is applied on top of layout coordinates.
    pub fn element_set_transform(&mut self, id: ElementId, matrix: Option<[f64; 6]>) {
        if let Some(el) = self.elements.get_mut(&id) {
            el.transform = matrix;
            self.engine
                .mark_visual_dirty(id, VisualInvalidationReach::SelfOnly);
        }
    }

    /// Programmatically set the scroll offset of a ScrollView element.
    pub fn element_set_scroll_offset(&mut self, id: ElementId, x: f32, y: f32) {
        if let Some(el) = self.elements.get_mut(&id) {
            el.scroll_offset = (x, y);
            self.engine
                .mark_visual_dirty(id, VisualInvalidationReach::SelfOnly);
        }
    }

    /// Read the current scroll offset of an element.
    pub fn element_get_scroll_offset(&self, id: ElementId) -> (f32, f32) {
        self.elements
            .get(&id)
            .map_or((0.0, 0.0), |e| e.scroll_offset)
    }

    /// Return the absolute layout rect (x, y, w, h) from the last render pass.
    pub fn element_layout_rect(&self, id: ElementId) -> Option<(f32, f32, f32, f32)> {
        self.layout.layout_cache.get(&id).copied()
    }

    /// Return the bounding dimensions of all descendants (content size) for a ScrollView.
    /// Values are relative to the element's own top-left corner.
    pub fn element_content_size(&self, id: ElementId) -> (f32, f32) {
        let &(ex, ey, _, _) = match self.layout.layout_cache.get(&id) {
            Some(r) => r,
            None => return (0.0, 0.0),
        };
        let mut max_x: f32 = 0.0;
        let mut max_y: f32 = 0.0;
        self.accumulate_content_bounds(id, ex, ey, &mut max_x, &mut max_y);
        (max_x, max_y)
    }

    fn accumulate_content_bounds(
        &self,
        id: ElementId,
        origin_x: f32,
        origin_y: f32,
        max_x: &mut f32,
        max_y: &mut f32,
    ) {
        let el = match self.elements.get(&id) {
            Some(e) => e,
            None => return,
        };
        for &child in &el.children {
            if let Some(&(cx, cy, cw, ch)) = self.layout.layout_cache.get(&child) {
                *max_x = max_x.max(cx - origin_x + cw);
                *max_y = max_y.max(cy - origin_y + ch);
                self.accumulate_content_bounds(child, origin_x, origin_y, max_x, max_y);
            }
        }
    }

    pub fn element_set_style(&mut self, id: ElementId, props: &[StyleProp]) {
        let el = match self.elements.get_mut(&id) {
            Some(e) => e,
            None => return,
        };
        let mut layout_changed = false;
        let mut text_dirty = false;
        for prop in props {
            if prop.is_layout() {
                taffy_bridge::apply_to_style(&mut el.layout_style, prop);
                layout_changed = true;
            } else {
                apply_visual(&mut el.visual, prop, &mut text_dirty);
            }
        }
        if text_dirty {
            el.text_layout = None;
        }
        if layout_changed {
            let style = el.layout_style.clone();
            self.layout.projection.set_style(id, style);
            return;
        }
        let change = self.classify_style_props(id, props);
        self.apply_style_change(id, change);
    }

    /// Merge the invalidation of every non-layout prop in a style change against
    /// the element's context (the *what*). Empty/all-layout lists fall back to a
    /// scene-only self repaint.
    fn classify_style_props(
        &self,
        id: ElementId,
        props: &[StyleProp],
    ) -> visual_invalidation::Change {
        let ctx = self.element_context(id);
        props
            .iter()
            .filter(|p| !p.is_layout())
            .map(|p| visual_invalidation::classify(p, ctx))
            .reduce(visual_invalidation::Change::merge)
            .unwrap_or_else(visual_invalidation::Change::visual_self_only)
    }

    /// Route a classified style change to its dirty set. The *what* (dirty kind
    /// + reach) is decided by `classify`; this only resolves the *which* element
    /// (e.g. the enclosing IFC root for a shape change).
    fn apply_style_change(&mut self, id: ElementId, change: visual_invalidation::Change) {
        match change.dirty_kind {
            DirtyKind::Shape => self.mark_text_content_dirty(id, change.reach),
            DirtyKind::Visual | DirtyKind::Structure => {
                self.engine.mark_visual_dirty(id, change.reach)
            }
        }
    }

    /// Append a viewport-conditional style override (ADR-0081).
    ///
    /// Multiple variants for the same property are kept in declaration order;
    /// `element_effective_visual` applies every matching variant and later
    /// entries win (CSS `@media` cascade).
    pub fn element_set_style_variant(
        &mut self,
        id: ElementId,
        condition: ViewportCondition,
        prop: StyleProp,
    ) {
        let el = match self.elements.get_mut(&id) {
            Some(e) => e,
            None => return,
        };
        el.viewport_variants.push((condition, prop));
    }

    /// Unset one or more inheritable style properties, reverting them to "inherit from parent".
    pub fn element_unset_style(&mut self, id: ElementId, kinds: &[StylePropKind]) {
        let el = match self.elements.get_mut(&id) {
            Some(e) => e,
            None => return,
        };
        let mut text_dirty = false;
        for kind in kinds {
            match kind {
                StylePropKind::Color => {
                    el.visual.text_color = None;
                }
                StylePropKind::FontSize => {
                    el.visual.font_size = None;
                    el.text_layout = None;
                    text_dirty = true;
                }
                StylePropKind::FontFamily => {
                    el.visual.font_family = None;
                    el.text_layout = None;
                    text_dirty = true;
                }
                StylePropKind::FontWeight => {
                    el.visual.font_weight = None;
                    el.text_layout = None;
                    text_dirty = true;
                }
            }
        }
        if text_dirty {
            self.mark_text_content_dirty(id, VisualInvalidationReach::Subtree);
        }
    }

    pub fn element_append_child(&mut self, parent: ElementId, child: ElementId) {
        if !self.elements.contains_key(&parent) || !self.elements.contains_key(&child) {
            return;
        }
        self.detach_from_current_parent(child);
        self.elements.get_mut(&parent).unwrap().children.push(child);
        self.elements.get_mut(&child).unwrap().parent = Some(parent);
        self.mark_child_attachment_dirty(parent, child);
    }

    pub fn element_insert_before(
        &mut self,
        parent: ElementId,
        child: ElementId,
        before: ElementId,
    ) {
        if !self.elements.contains_key(&parent)
            || !self.elements.contains_key(&child)
            || !self.elements.contains_key(&before)
        {
            return;
        }
        self.detach_from_current_parent(child);
        let index = match self.elements[&parent]
            .children
            .iter()
            .position(|&c| c == before)
        {
            Some(i) => i,
            None => {
                // `before` is not a child of `parent`; append as a fallback.
                self.element_append_child(parent, child);
                return;
            }
        };
        self.elements
            .get_mut(&parent)
            .unwrap()
            .children
            .insert(index, child);
        self.elements.get_mut(&child).unwrap().parent = Some(parent);
        self.mark_child_attachment_dirty(parent, child);
    }

    pub fn element_remove(&mut self, id: ElementId) {
        if !self.elements.contains_key(&id) {
            return;
        }
        self.detach_from_current_parent(id);
        // Recursively remove the subtree.
        let mut stack = vec![id];
        let mut to_remove = Vec::new();
        while let Some(node) = stack.pop() {
            to_remove.push(node);
            if let Some(el) = self.elements.get(&node) {
                stack.extend(el.children.iter().copied());
            }
        }
        if let Some(root) = self.root {
            self.engine.mark_structure_dirty(root);
        }
        for node in to_remove.into_iter().rev() {
            self.elements.remove(&node);
            self.runtime.remove_element_listeners(node);
            if self.focused_element == Some(node) {
                self.focused_element = None;
                self.layout.last_cursor_toggle_ms = None;
            }
            self.hovered_elements.remove(&node);
            if self.active_element == Some(node) {
                self.active_element = None;
            }
        }
        if self.root == Some(id) {
            self.root = None;
        }
    }

    /// Update CSS `:hover` set from the deepest hit under the pointer.
    /// Returns `(entered, left)` for event dispatch.
    pub fn update_pointer_hover(&mut self, deepest_hit: Option<ElementId>) -> (Vec<ElementId>, Vec<ElementId>) {
        let next = match deepest_hit {
            Some(hit) => hover_set_for_hit(&self.elements, hit),
            None => HashSet::new(),
        };
        let (entered, left) = diff_hover_sets(&self.hovered_elements, &next);
        for id in &entered {
            self.mark_pseudo_activation_dirty(*id, PseudoState::Hover);
        }
        for id in &left {
            self.mark_pseudo_activation_dirty(*id, PseudoState::Hover);
        }
        self.hovered_elements = next;
        (entered, left)
    }

    /// HTML `mouseenter` path: mark a single element hovered (parent retains hover over children).
    pub fn hover_enter_element(&mut self, id: ElementId) -> bool {
        self.hovered_elements.insert(id)
    }

    /// HTML `mouseleave` path: clear hover on the element that was left.
    pub fn hover_leave_element(&mut self, id: ElementId) -> bool {
        self.hovered_elements.remove(&id)
    }

    pub fn element_set_pseudo_style(&mut self, id: ElementId, state: PseudoState, props: &[StyleProp]) {
        let el = match self.elements.get_mut(&id) {
            Some(e) => e,
            None => return,
        };
        let slot = el.pseudo_styles.props_mut(state);
        for prop in props {
            if prop.is_layout() {
                continue;
            }
            pseudo_state::upsert_style_prop(slot, prop);
        }
        let reach = self.classify_style_props(id, props).reach;
        self.engine.mark_visual_dirty(id, reach);
    }

    pub fn element_unset_pseudo_style(
        &mut self,
        id: ElementId,
        state: PseudoState,
        kinds: &[StylePropKind],
    ) {
        let el = match self.elements.get_mut(&id) {
            Some(e) => e,
            None => return,
        };
        for kind in kinds {
            pseudo_state::unset_pseudo_prop(&mut el.pseudo_styles, state, *kind);
        }
        self.engine
            .mark_visual_dirty(id, VisualInvalidationReach::Subtree);
    }

    pub fn element_get_text(&self, id: ElementId) -> String {
        self.elements
            .get(&id)
            .and_then(|e| e.text.clone())
            .unwrap_or_default()
    }

    pub fn element_kind(&self, id: ElementId) -> Option<ElementKind> {
        self.elements.get(&id).map(|e| e.kind)
    }

    pub fn element_parent(&self, id: ElementId) -> Option<ElementId> {
        self.elements.get(&id).and_then(|e| e.parent)
    }

    /// Whether `id` was projected to a Taffy node on the last layout pass.
    #[doc(hidden)]
    pub fn element_has_taffy_node(&self, id: ElementId) -> bool {
        self.layout.projection.has_node(id)
    }

    /// Element ids in `root` and its descendants (pre-order). Empty when unknown.
    pub fn subtree_element_ids(&self, root: ElementId) -> Vec<ElementId> {
        if !self.elements.contains_key(&root) {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            out.push(node);
            if let Some(el) = self.elements.get(&node) {
                stack.extend(el.children.iter().copied());
            }
        }
        out
    }

    /// Run layout, lower the element tree into the scene graph, and return it.
    ///
    /// `timestamp_ms` is a monotonic host clock (e.g. `performance.now()`); it
    /// drives the focused TextInput's cursor blink without exposing a cursor-tick
    /// function to the host (ADR-0032).
    pub fn render(&mut self, timestamp_ms: f64) -> &SceneGraph {
        if self.root.is_some() {
            if let Some(id) = self.layout.advance_cursor_blink(
                &mut self.elements,
                self.focused_element,
                timestamp_ms,
            ) {
                self.engine
                    .mark_visual_dirty(id, VisualInvalidationReach::SelfOnly);
            }
        }
        let mut dirty = collect_lowering_dirty(
            self,
            &self.engine.structure_dirty,
            &self.engine.shape_dirty,
            &self.engine.shape_lowering_reach,
            &self.engine.viewport_dirty,
            &self.engine.visual_dirty,
            self.engine.fonts_dirty,
        );
        self.commit_frame();
        // `commit_frame` re-ran layout; fold any element whose box geometry
        // changed into this frame's lowering set so reflowed-but-otherwise-clean
        // boxes (grown ancestors, pushed siblings) re-lower instead of painting
        // stale geometry. Done after commit because the diff is only known once
        // the new layout cache exists, and before `scene_build::update` consumes
        // `dirty`.
        let geometry_dirty = self.engine.drain_layout_geometry_dirty();
        let _ = self.engine.drain_visual_dirty();
        let _ = self.engine.drain_shape_lowering_reach();
        for id in geometry_dirty {
            visual_invalidation::apply_visual_invalidation(
                self,
                id,
                VisualInvalidationReach::SelfOnly,
                &mut dirty.elements,
                &mut dirty.z_index_reorder_parents,
            );
        }
        let mut scene_cache = std::mem::take(&mut self.scene_cache);
        let mut scene_lowering = std::mem::take(&mut self.scene_lowering);
        scene_build::update(self, &mut scene_cache, &mut scene_lowering, dirty, timestamp_ms);
        // Transitions advance at the lowering seam; keep any still-interpolating
        // element visual-dirty so the next frame re-lowers and advances it. When
        // the last track settles this frame the element is not re-marked and the
        // frame loop goes quiet (ADR-0086/0093).
        for id in scene_lowering.active_transition_ids() {
            self.engine
                .mark_visual_dirty(id, VisualInvalidationReach::SelfOnly);
        }
        self.scene_cache = scene_cache;
        self.scene_lowering = scene_lowering;
        &self.scene_cache
    }

    /// Resolve dirty state and settle layout (`LayoutPass::run()` equivalent, ADR-0075
    /// scope A): Taffy projection reconcile, Parley text shaping, and layout-cache refresh.
    /// Does not lower the scene graph or advance the cursor blink.
    pub fn commit_frame(&mut self) {
        if let Some(root) = self.root {
            self.engine.resolve(
                &mut self.layout,
                &mut self.elements,
                root,
                self.viewport,
                &mut self.event_queue,
            );
        }
    }

    pub fn scene_graph(&self) -> &SceneGraph {
        &self.scene_cache
    }

    pub fn poll_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.event_queue)
    }

    pub fn register_listener(
        &mut self,
        element_id: ElementId,
        kind: DocumentEventKind,
    ) -> ListenerId {
        self.runtime.register_listener(element_id, kind)
    }

    pub fn dispatch_event(&mut self, kind: DocumentEventKind, event: Event) {
        let mut path = Vec::new();
        let mut node = document_runtime::event_target(&event);
        while let Some(id) = node {
            path.push(id);
            if !kind.bubbles() {
                break;
            }
            node = self.element_parent(id);
        }
        self.runtime.dispatch_to_path(&path, kind, event);
    }

    pub fn poll_deliveries(&mut self) -> Vec<EventDelivery> {
        self.runtime.poll_deliveries()
    }

    /// Apply wheel delta to ancestor ScrollViews of `hit` with browser-style scroll chaining.
    ///
    /// Starting at the nearest ScrollView, each axis consumes delta up to content bounds;
    /// unconsumed remainder propagates to the next ancestor ScrollView until the root.
    pub fn apply_wheel_delta(
        &mut self,
        hit: ElementId,
        delta_x: f32,
        delta_y: f32,
    ) -> Option<ElementId> {
        let first_sv = nearest_scroll_view(self, hit)?;
        let mut current_sv = first_sv;
        let mut remaining_x = delta_x;
        let mut remaining_y = delta_y;
        let mut any_applied = false;

        loop {
            if remaining_x.abs() < 1e-6 && remaining_y.abs() < 1e-6 {
                break;
            }

            let (ox, oy) = self.element_get_scroll_offset(current_sv);
            let (content_w, content_h) = self.element_content_size(current_sv);
            let sv_rect = self
                .element_layout_rect(current_sv)
                .unwrap_or((0.0, 0.0, 0.0, 0.0));
            let max_x = (content_w - sv_rect.2).max(0.0);
            let max_y = (content_h - sv_rect.3).max(0.0);
            let new_x = (ox + remaining_x).clamp(0.0, max_x);
            let new_y = (oy + remaining_y).clamp(0.0, max_y);
            let consumed_x = new_x - ox;
            let consumed_y = new_y - oy;

            if consumed_x.abs() > 1e-6 || consumed_y.abs() > 1e-6 {
                self.element_set_scroll_offset(current_sv, new_x, new_y);
                any_applied = true;
            }

            remaining_x -= consumed_x;
            remaining_y -= consumed_y;

            match next_ancestor_scroll_view(self, current_sv) {
                Some(next) => current_sv = next,
                None => break,
            }
        }

        if any_applied {
            Some(first_sv)
        } else {
            None
        }
    }

    /// Append an event to the outgoing queue.
    pub fn push_event(&mut self, event: Event) {
        self.event_queue.push(event);
    }

    /// Returns true if at least one layout pass has completed (layout_cache is populated).
    pub fn has_layout(&self) -> bool {
        !self.layout.layout_cache.is_empty()
    }

    /// Z-Order の単一正本。`id` の子兄弟を **paint order**（z 昇順・同 z は
    /// document 順で安定 = 後勝ち）で返す。
    ///
    /// 描画（`scene_build`）はこの順で前方反復し、hit-test は `.rev()` で最前面から
    /// 走る。「hit-test = paint の逆順」を構造的に保証するため、Z-Order の順序解決は
    /// この 1 メソッドに集約する。`resolved_elements` / HTML 経路は意図的にこの seam を
    /// 通さず document order を保つ（CSS が stacking、将来の a11y 読み上げ順は document
    /// order）。ADR-0021 / ADR-0060。
    pub fn ordered_children(&self, id: ElementId) -> Vec<ElementId> {
        let mut children = match self.elements.get(&id) {
            Some(el) => el.children.clone(),
            None => return Vec::new(),
        };
        // 安定ソート: 同 z は元の document 順を保持する。
        children.sort_by_key(|cid| self.elements.get(cid).map_or(0, |c| c.visual.z_index));
        children
    }

    /// Returns the deepest element whose bounding rect contains (x, y),
    /// or None if no element is hit. Uses the layout from the last render pass.
    /// Character bounds for IME candidate window (ADR-0069). Requires prior layout.
    pub fn element_character_bounds(&self, id: ElementId) -> Option<CharacterBounds> {
        let el = self.elements.get(&id)?;
        let edit = el.edit.as_ref()?;
        let cl = el.content_layout.as_ref()?;
        let &(ex, ey, _, _) = self.layout.layout_cache.get(&id)?;
        let taffy_node = self.layout.projection.node_id(id)?;
        let box_layout = self.layout.projection.taffy.layout(taffy_node).ok()?;
        let content_x = ex + box_layout.border.left + box_layout.padding.left;
        let content_y = ey + box_layout.border.top + box_layout.padding.top;
        use parley::{Affinity, Cursor};
        let cursor = Cursor::from_byte_index(
            &cl.layout,
            edit.cursor_byte_index,
            Affinity::Upstream,
        );
        let bbox = cursor.geometry(&cl.layout, 1.5_f32);
        Some(CharacterBounds {
            x: content_x + bbox.x0 as f32,
            y: content_y + bbox.y0 as f32,
            width: ((bbox.x1 - bbox.x0) as f32).max(1.5),
            height: (bbox.y1 - bbox.y0) as f32,
        })
    }

    /// Resolved effective visual for `id` (inheritance + viewport variant + pseudo). ADR-0067, ADR-0081.
    pub fn element_effective_visual(&self, id: ElementId) -> Option<Visual> {
        let el = self.elements.get(&id)?;
        let ctx = effective_visual::inherited_context_at(&self.elements, id);
        let interaction = self.interaction_snapshot();
        let own = effective_visual::own_with_viewport_variants(
            &el.visual,
            &el.viewport_variants,
            self.viewport,
        );
        Some(effective_visual::resolve_effective(
            &ctx,
            &own,
            &el.pseudo_styles,
            &interaction,
            id,
        ))
    }

    /// Returns the deepest element whose bounding rect contains (x, y),
    /// or None if no element is hit. Uses the layout from the last render pass.
    pub fn hit_test(&self, x: f32, y: f32) -> Option<ElementId> {
        let root = self.root?;
        let box_hit = hit_test_walk(self, root, x, y)?;
        inline_text::resolve_ifc_inline_hit(self, box_hit, x, y)
    }

    /// Run layout and return every element with its absolute position and visual state.
    /// Keyed by stable ElementId — safe to use as a DOM node mapping key across frames.
    pub fn resolved_elements(&mut self) -> Vec<(ElementId, ResolvedElement)> {
        self.commit_frame();
        let interaction = self.interaction_snapshot();
        let mut out = Vec::new();
        if let Some(root) = self.root {
            walk_resolved(
                &self.elements,
                &self.layout.projection,
                root,
                0.0,
                0.0,
                effective_visual::InheritedVisualContext::root(),
                &interaction,
                self.viewport,
                &mut out,
            );
        }
        out
    }

    // ── internals ────────────────────────────────────────────────────────

    fn detach_from_current_parent(&mut self, child: ElementId) {
        let parent = match self.elements.get(&child).and_then(|c| c.parent) {
            Some(p) => p,
            None => return,
        };
        self.elements
            .get_mut(&parent)
            .unwrap()
            .children
            .retain(|&c| c != child);
        self.elements.get_mut(&child).unwrap().parent = None;
        self.mark_child_detachment_dirty(parent, child);
    }

    pub(crate) fn mark_pseudo_activation_dirty(&mut self, id: ElementId, state: PseudoState) {
        let props = match self.elements.get(&id) {
            Some(el) => el.pseudo_styles.props(state),
            None => return,
        };
        if props.is_empty() {
            return;
        }
        let reach = self.classify_style_props(id, props).reach;
        self.engine.mark_visual_dirty(id, reach);
        if pseudo_state::pseudo_affects_text_shaping(props) {
            self.mark_text_content_dirty(id, reach);
        }
        // The transition trigger lives at the `resolve_effective` lowering seam
        // (ADR-0093), not here: marking the element visual-dirty is enough to
        // re-lower it, where the per-property diff starts any interpolation.
    }

    fn mark_text_content_dirty(&mut self, id: ElementId, reach: VisualInvalidationReach) {
        if let Some(root) = ifc_root(&self.elements, id) {
            self.engine.mark_shape_dirty(root, reach);
            self.layout.projection.mark_dirty(root);
        } else if self.layout.projection.has_node(id) {
            self.engine.mark_shape_dirty(id, reach);
            self.layout.projection.mark_dirty(id);
        }
    }

    fn mark_child_attachment_dirty(&mut self, parent: ElementId, child: ElementId) {
        let parent_ctx = self.element_context(parent);
        let child_ctx = self.element_context(child);
        match visual_invalidation::classify_attachment(parent_ctx, child_ctx).dirty_kind {
            DirtyKind::Shape => {
                self.engine
                    .mark_shape_dirty(parent, VisualInvalidationReach::Subtree);
                self.layout.projection.mark_dirty(parent);
            }
            DirtyKind::Visual | DirtyKind::Structure => {
                self.engine.mark_structure_dirty(parent);
                self.engine.mark_structure_dirty(child);
            }
        }
    }

    /// Build the topological context of an element for the invalidation
    /// classifier. Reads the live tree; the classifier itself stays pure.
    pub(crate) fn element_context(&self, id: ElementId) -> ElementContext {
        let el = self.elements.get(&id);
        let kind = el.map_or(ElementKind::View, |e| e.kind);
        let has_text_parent = el
            .and_then(|e| e.parent)
            .and_then(|p| self.elements.get(&p))
            .is_some_and(|p| p.kind == ElementKind::Text);
        ElementContext {
            kind,
            is_ifc_root: is_ifc_root(&self.elements, id),
            has_text_parent,
        }
    }

    fn mark_child_detachment_dirty(&mut self, parent: ElementId, child: ElementId) {
        self.mark_child_attachment_dirty(parent, child);
    }

    fn mark_viewport_dirty_on_resize(
        &mut self,
        old_viewport: (f32, f32),
        new_viewport: (f32, f32),
    ) {
        for (&id, el) in &self.elements {
            if el.viewport_variants.is_empty() {
                continue;
            }
            if effective_visual::viewport_variant_resolution_changed(
                &el.visual,
                &el.viewport_variants,
                old_viewport,
                new_viewport,
            ) {
                self.engine.mark_viewport_dirty(id);
            }
        }
    }
}

impl Default for ElementTree {
    fn default() -> Self {
        Self::new()
    }
}

fn walk_resolved(
    elements: &HashMap<ElementId, Element>,
    projection: &TaffyProjection,
    id: ElementId,
    ox: f32,
    oy: f32,
    inherited: effective_visual::InheritedVisualContext,
    interaction: &InteractionSnapshot,
    viewport: (f32, f32),
    out: &mut Vec<(ElementId, ResolvedElement)>,
) {
    let (taffy_node, el) = match projection.traversal_step(elements, id) {
        Some(TraversalStep::Visit(taffy_node, el)) => (Some(taffy_node), el),
        Some(TraversalStep::Skip(el)) => (None, el),
        None => return,
    };
    let inherited_base = effective_visual::apply_text_inheritance(&inherited, &el.visual);
    let child_inherited = child_inherited_context(
        &inherited,
        el.kind,
        &inherited_base,
        &el.visual,
    );
    let taffy_node = match taffy_node {
        Some(n) => n,
        None => {
            for &child in &el.children {
                walk_resolved(
                    elements,
                    projection,
                    child,
                    ox,
                    oy,
                    child_inherited.clone(),
                    interaction,
                    viewport,
                    out,
                );
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
    let own = effective_visual::own_with_viewport_variants(
        &el.visual,
        &el.viewport_variants,
        viewport,
    );
    let visual = effective_visual::resolve_effective(
        &inherited,
        &own,
        &el.pseudo_styles,
        interaction,
        id,
    );

    let display_text_content = if el.kind == ElementKind::TextInput {
        el.edit.as_ref().map(|edit| edit.display_text())
    } else {
        None
    };

    out.push((
        id,
        ResolvedElement {
            kind: el.kind,
            x,
            y,
            width: layout.size.width,
            height: layout.size.height,
            background_color: visual.background_color,
            opacity: visual.opacity,
            border_radius: visual.border_radius,
            border_width: visual.border_width,
            border_color: visual.border_color,
            text_color: visual.text_color,
            font_size: visual.font_size,
            font_weight: visual.font_weight,
            z_index: visual.z_index,
            text: el.text.clone(),
            src: el.src.clone(),
            text_content: display_text_content,
            font_family: visual.font_family.clone(),
            aria_label: el.aria_label.clone(),
            role: el.role.clone(),
        },
    ));

    for &child in &el.children {
        walk_resolved(
            elements,
            projection,
            child,
            x,
            y,
            child_inherited.clone(),
            interaction,
            viewport,
            out,
        );
    }
}

fn hit_test_walk(tree: &ElementTree, id: ElementId, x: f32, y: f32) -> Option<ElementId> {
    let &(ex, ey, ew, eh) = tree.layout.layout_cache.get(&id)?;
    if x < ex || y < ey || x >= ex + ew || y >= ey + eh {
        return None;
    }
    tree.elements.get(&id)?;
    // Visit children in reverse paint order (`.rev()`) so the topmost element wins.
    // Sharing `ordered_children` keeps hit-test as the exact reverse of paint order.
    for child in tree.ordered_children(id).into_iter().rev() {
        if let Some(hit) = hit_test_walk(tree, child, x, y) {
            return Some(hit);
        }
    }
    if tree.elements.get(&id).is_some_and(|e| e.disabled) {
        return None;
    }
    Some(id)
}

pub(crate) fn apply_visual(visual: &mut Visual, prop: &StyleProp, text_dirty: &mut bool) {
    match prop {
        StyleProp::BackgroundColor(c) => visual.background_color = Some(*c),
        StyleProp::Opacity(v) => visual.opacity = v.clamp(0.0, 1.0),
        StyleProp::BorderRadius(v) => visual.border_radius = v.max(0.0),
        StyleProp::BorderWidth(v) => visual.border_width = v.max(0.0),
        StyleProp::BorderColor(c) => visual.border_color = Some(*c),
        StyleProp::BorderStyle(v) => visual.border_style = *v,
        StyleProp::BoxShadow(shadows) => visual.box_shadow = shadows.clone(),
        StyleProp::Overflow(v) => visual.overflow = *v,
        StyleProp::MaxLines(v) => {
            visual.max_lines = if *v == 0 { None } else { Some(*v) };
            *text_dirty = true;
        }
        StyleProp::TextOverflow(v) => {
            visual.text_overflow = *v;
            *text_dirty = true;
        }
        StyleProp::FontSize(v) => {
            visual.font_size = Some(v.max(0.0));
            *text_dirty = true;
        }
        StyleProp::FontFamily(f) => {
            visual.font_family = if f.is_empty() { None } else { Some(f.clone()) };
            *text_dirty = true;
        }
        StyleProp::FontWeight(v) => {
            visual.font_weight = Some(v.clamp(1.0, 1000.0));
            *text_dirty = true;
        }
        StyleProp::Color(c) => {
            visual.text_color = Some(*c);
            *text_dirty = true;
        }
        StyleProp::FontStyle(v) => {
            visual.font_style = Some(*v);
            *text_dirty = true;
        }
        StyleProp::TextDecoration(v) => {
            visual.text_decoration = Some(*v);
            *text_dirty = true;
        }
        StyleProp::Cursor(v) => visual.cursor = Some(*v),
        StyleProp::DefaultColor(c) => visual.default_color = Some(*c),
        StyleProp::DefaultFontSize(v) => visual.default_font_size = Some(v.max(0.0)),
        StyleProp::DefaultFontWeight(v) => {
            visual.default_font_weight = Some(v.clamp(1.0, 1000.0));
        }
        StyleProp::DefaultFontFamily(f) => {
            visual.default_font_family = if f.is_empty() {
                None
            } else {
                Some(f.clone())
            };
        }
        StyleProp::ZIndex(z) => visual.z_index = *z,
        StyleProp::TransitionDuration(v) => visual.transition_duration = v.max(0.0),
        StyleProp::TransitionTiming(v) => visual.transition_timing = *v,
        _ => {}
    }
}

fn nearest_scroll_view(tree: &ElementTree, mut id: ElementId) -> Option<ElementId> {
    loop {
        if tree.element_kind(id) == Some(ElementKind::ScrollView) {
            return Some(id);
        }
        id = tree.element_parent(id)?;
    }
}

fn next_ancestor_scroll_view(tree: &ElementTree, after: ElementId) -> Option<ElementId> {
    let mut id = tree.element_parent(after)?;
    loop {
        if tree.element_kind(id) == Some(ElementKind::ScrollView) {
            return Some(id);
        }
        id = tree.element_parent(id)?;
    }
}

#[doc(hidden)]
impl ElementTree {
    pub fn test_scene_lowering_built(&self) -> bool {
        self.scene_lowering.built
    }

    pub fn test_scene_lowering_walk_count(&self) -> usize {
        self.scene_lowering.walk_count
    }

    pub fn test_visual_dirty_contains(&self, id: ElementId) -> bool {
        self.engine.visual_dirty.contains_key(&id)
    }

    pub fn test_shape_dirty_contains(&self, id: ElementId) -> bool {
        self.engine.shape_dirty.contains(&id)
    }

    /// Whether a continuous-property transition is currently in flight for `id`
    /// (issue #227). State lives in the retained lowering, so this reflects the
    /// last `render()` pass.
    pub fn test_transition_active(&self, id: ElementId) -> bool {
        self.scene_lowering
            .anchors
            .get(&id)
            .is_some_and(|entry| entry.transitions.is_active())
    }

    /// Number of laid-out lines in an element's shaped text (issue #207 tests).
    pub fn test_text_line_count(&self, id: ElementId) -> Option<usize> {
        self.elements
            .get(&id)
            .and_then(|el| el.text_layout.as_ref())
            .map(|tl| tl.layout.lines().count())
    }

    /// The shaped text of an element's IFC layout, after any truncation (issue #207 tests).
    pub fn test_shaped_text(&self, id: ElementId) -> Option<String> {
        self.elements
            .get(&id)
            .and_then(|el| el.text_layout.as_ref())
            .map(|tl| tl.text.to_string())
    }

    /// Mirror of `render()` cursor-blink tick without draining dirty sets (issue #183).
    pub fn test_tick_cursor_blink(&mut self, timestamp_ms: f64) -> bool {
        let Some(id) = self.layout.advance_cursor_blink(
            &mut self.elements,
            self.focused_element,
            timestamp_ms,
        ) else {
            return false;
        };
        self.engine
            .mark_visual_dirty(id, VisualInvalidationReach::SelfOnly);
        true
    }

    pub fn test_element_anchor_id(&self, id: ElementId) -> crate::node::NodeId {
        self.scene_lowering
            .anchors
            .get(&id)
            .expect("element anchor")
            .anchor_id
    }

    pub fn test_scene_full_rebuild_draw_ops(&self) -> Vec<crate::render::DrawOp> {
        use crate::render::{render_scene_graph, RecordingPainter};
        let sg = scene_build::build_ephemeral(self);
        let mut painter = RecordingPainter::new();
        render_scene_graph(&sg, &mut painter);
        painter.into_ops()
    }
}

#[cfg(test)]
mod viewport_resize_tests {
    use super::*;
    use crate::element::kind::ElementKind;

    #[test]
    fn set_viewport_marks_viewport_dirty_when_breakpoint_crossed() {
        let mut tree = ElementTree::new();
        let id = tree.element_create(13210, ElementKind::View);
        tree.set_root(id);
        tree.set_viewport(500.0, 800.0);
        tree.element_set_style_variant(
            id,
            ViewportCondition {
                min_width: Some(768.0),
                ..Default::default()
            },
            StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
        );

        tree.set_viewport(900.0, 800.0);
        assert_eq!(tree.test_viewport_dirty_len(), 1);

        tree.commit_frame();
        assert_eq!(tree.test_viewport_dirty_len(), 0);
    }

    #[test]
    fn set_viewport_skips_viewport_dirty_without_variants() {
        let mut tree = ElementTree::new();
        let id = tree.element_create(13211, ElementKind::View);
        tree.set_root(id);
        tree.set_viewport(400.0, 300.0);

        tree.set_viewport(900.0, 800.0);
        assert_eq!(tree.test_viewport_dirty_len(), 0);
        let _ = id;
    }

    #[test]
    fn set_viewport_skips_viewport_dirty_within_same_breakpoint() {
        let mut tree = ElementTree::new();
        let id = tree.element_create(13212, ElementKind::View);
        tree.set_root(id);
        tree.set_viewport(900.0, 800.0);
        tree.element_set_style_variant(
            id,
            ViewportCondition {
                min_width: Some(768.0),
                ..Default::default()
            },
            StyleProp::BackgroundColor(Color::new(0.0, 0.0, 1.0, 1.0)),
        );

        tree.set_viewport(950.0, 850.0);
        assert_eq!(tree.test_viewport_dirty_len(), 0);
    }
}
