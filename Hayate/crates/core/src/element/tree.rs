use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use fontique::FontInfoOverride;
use linebender_resource_handle::Blob;
use crate::color::Color;
use crate::element::document_runtime::{self, DocumentRuntime, EventDelivery, ListenerId};
use crate::element::edit_state::EditState;
use crate::element::effective_visual::{self, child_inherited_context};
use crate::element::ime_bridge::CharacterBounds;
use crate::element::event_spec::DocumentEventKind;

pub use crate::element::event_spec::Event;
use crate::element::id::ElementId;
use crate::element::kind::ElementKind;
use crate::element::inline_text::{self, ifc_root, is_ifc_root};
use crate::element::layout_pass::LayoutPass;
use crate::element::taffy_projection::TaffyProjection;
use crate::element::pseudo_state::{
    self, diff_hover_sets, hover_set_for_hit, InteractionSnapshot, PseudoState, PseudoStyles,
};
use crate::element::scene_build;
use crate::element::style::{FontStyleValue, StyleProp, StylePropKind, TextDecorationValue};
use crate::element::taffy_bridge;
use crate::element::text;
use crate::node::SceneGraph;
use crate::render::RenderImage;

#[derive(Clone, Debug)]
pub struct Visual {
    pub background_color: Option<Color>,
    pub opacity: f32,
    pub border_radius: f32,
    pub border_width: f32,
    pub border_color: Option<Color>,
    pub text_color: Option<Color>,
    pub font_size: Option<f32>,
    pub font_weight: Option<f32>,
    pub font_style: Option<FontStyleValue>,
    pub text_decoration: Option<TextDecorationValue>,
    pub z_index: i32,
    /// Custom font-family name registered via `register_font`.
    pub font_family: Option<String>,
    /// Ambient default text style (block-penetrating, ADR-0065 ch2).
    pub default_color: Option<Color>,
    pub default_font_size: Option<f32>,
    pub default_font_weight: Option<f32>,
    pub default_font_family: Option<String>,
}

impl Default for Visual {
    fn default() -> Self {
        Self {
            background_color: None,
            opacity: 1.0,
            border_radius: 0.0,
            border_width: 0.0,
            border_color: None,
            text_color: None,
            font_size: None,
            font_weight: None,
            font_style: None,
            text_decoration: None,
            z_index: 0,
            font_family: None,
            default_color: None,
            default_font_size: None,
            default_font_weight: None,
            default_font_family: None,
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
    /// cross one seam (`layout.run(...)`) instead of touching Taffy, Parley,
    /// font dirty state, and cursor timing directly.
    pub(crate) layout: LayoutPass,
    pub(crate) viewport: (f32, f32),
    pub(crate) scene_cache: SceneGraph,
    pub(crate) event_queue: Vec<Event>,
    /// Element that owns the text-input cursor blink. Tracked here (not in the
    /// adapter) so `render(timestamp_ms)` can advance the blink itself per ADR-0032.
    pub(crate) focused_element: Option<ElementId>,
    /// Elements matching CSS `:hover` (self or descendant under pointer).
    pub(crate) hovered_elements: HashSet<ElementId>,
    pub(crate) active_element: Option<ElementId>,
    /// Last pointer position for sub-pixel move dedup (ADR-0066).
    pub(crate) last_pointer_pos: Option<(f32, f32)>,
    pub(crate) runtime: DocumentRuntime,
}

impl ElementTree {
    pub fn new() -> Self {
        Self {
            elements: HashMap::new(),
            root: None,
            layout: LayoutPass::new(),
            viewport: (800.0, 600.0),
            scene_cache: SceneGraph::new(),
            event_queue: Vec::new(),
            focused_element: None,
            hovered_elements: HashSet::new(),
            active_element: None,
            last_pointer_pos: None,
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
        self.viewport = (width, height);
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
        self.mark_text_content_dirty(id);
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

    /// Store decoded image data for an Image element (called by the adapter after async load).
    pub fn element_set_image(&mut self, id: ElementId, image: Arc<RenderImage>) {
        if let Some(el) = self.elements.get_mut(&id) {
            el.src_image = Some(image);
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
        }
        if let Some(el) = self.elements.get_mut(&id) {
            el.cursor_visible = true;
        }
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
        self.layout.fonts_dirty = true;
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
        }
    }

    /// Programmatically set the scroll offset of a ScrollView element.
    pub fn element_set_scroll_offset(&mut self, id: ElementId, x: f32, y: f32) {
        if let Some(el) = self.elements.get_mut(&id) {
            el.scroll_offset = (x, y);
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
        } else if text_dirty {
            self.mark_text_content_dirty(id);
        }
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
            self.mark_text_content_dirty(id);
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
            self.layout.mark_structure_dirty(root);
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
        if let Some(root) = self.root {
            self.layout
                .advance_cursor_blink(&mut self.elements, self.focused_element, timestamp_ms);
            self.layout.run(
                &mut self.elements,
                root,
                self.viewport,
                &mut self.event_queue,
            );
        }
        self.scene_cache = scene_build::build(self);
        &self.scene_cache
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

    /// Apply wheel delta to the nearest ancestor ScrollView of `hit`, clamped to content bounds.
    pub fn apply_wheel_delta(
        &mut self,
        hit: ElementId,
        delta_x: f32,
        delta_y: f32,
    ) -> Option<ElementId> {
        let sv = nearest_scroll_view(self, hit)?;
        let (ox, oy) = self.element_get_scroll_offset(sv);
        let (content_w, content_h) = self.element_content_size(sv);
        let sv_rect = self
            .element_layout_rect(sv)
            .unwrap_or((0.0, 0.0, 0.0, 0.0));
        let max_x = (content_w - sv_rect.2).max(0.0);
        let max_y = (content_h - sv_rect.3).max(0.0);
        let new_x = (ox + delta_x).clamp(0.0, max_x);
        let new_y = (oy + delta_y).clamp(0.0, max_y);
        self.element_set_scroll_offset(sv, new_x, new_y);
        Some(sv)
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
        use parley::{Affinity, Cursor};
        let cursor = Cursor::from_byte_index(
            &cl.layout,
            edit.cursor_byte_index,
            Affinity::Upstream,
        );
        let bbox = cursor.geometry(&cl.layout, 1.5_f32);
        Some(CharacterBounds {
            x: ex + bbox.x0 as f32,
            y: ey + bbox.y0 as f32,
            width: ((bbox.x1 - bbox.x0) as f32).max(1.5),
            height: (bbox.y1 - bbox.y0) as f32,
        })
    }

    /// Resolved effective visual for `id` (inheritance + pseudo). ADR-0067.
    pub fn element_effective_visual(&self, id: ElementId) -> Option<Visual> {
        let el = self.elements.get(&id)?;
        let ctx = effective_visual::inherited_context_at(&self.elements, id);
        let interaction = self.interaction_snapshot();
        Some(effective_visual::resolve_effective(
            &ctx,
            &el.visual,
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
        resolve_ifc_inline_hit(self, box_hit, x, y)
    }

    /// Run layout and return every element with its absolute position and visual state.
    /// Keyed by stable ElementId — safe to use as a DOM node mapping key across frames.
    pub fn resolved_elements(&mut self) -> Vec<(ElementId, ResolvedElement)> {
        if let Some(root) = self.root {
            self.layout.run(
                &mut self.elements,
                root,
                self.viewport,
                &mut self.event_queue,
            );
        }
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

    fn mark_text_content_dirty(&mut self, id: ElementId) {
        if let Some(root) = ifc_root(&self.elements, id) {
            self.layout.mark_shape_dirty(root);
        } else if self.layout.projection.has_node(id) {
            self.layout.projection.mark_dirty(id);
        }
    }

    fn mark_child_attachment_dirty(&mut self, parent: ElementId, child: ElementId) {
        if is_ifc_root(&self.elements, parent)
            && self
                .elements
                .get(&child)
                .is_some_and(|e| e.kind == ElementKind::Text)
        {
            self.layout.mark_shape_dirty(parent);
        } else {
            self.layout.mark_structure_dirty(parent);
            self.layout.mark_structure_dirty(child);
        }
    }

    fn mark_child_detachment_dirty(&mut self, parent: ElementId, child: ElementId) {
        self.mark_child_attachment_dirty(parent, child);
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
    out: &mut Vec<(ElementId, ResolvedElement)>,
) {
    let el = match elements.get(&id) {
        Some(e) => e,
        None => return,
    };
    let inherited_base = effective_visual::apply_text_inheritance(&inherited, &el.visual);
    let child_inherited = child_inherited_context(
        &inherited,
        el.kind,
        &inherited_base,
        &el.visual,
    );
    let taffy_node = match projection.node_id(id) {
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
    let visual = effective_visual::resolve_effective(
        &inherited,
        &el.visual,
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
            out,
        );
    }
}

fn resolve_ifc_inline_hit(
    tree: &ElementTree,
    box_hit: ElementId,
    x: f32,
    y: f32,
) -> Option<ElementId> {
    if !is_ifc_root(&tree.elements, box_hit) {
        return Some(box_hit);
    }
    let el = tree.elements.get(&box_hit)?;
    let tl = el.text_layout.as_ref()?;
    let &(ex, ey, _, _) = tree.layout.layout_cache.get(&box_hit)?;
    let byte = inline_text::byte_index_at_point(tl, x - ex, y - ey);
    if let Some(map) = &tl.range_map {
        if let Some(inline_id) = map.lookup(byte) {
            return Some(inline_id);
        }
    }
    Some(box_hit)
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
