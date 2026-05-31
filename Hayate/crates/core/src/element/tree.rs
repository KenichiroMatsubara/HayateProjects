use std::collections::{HashMap, HashSet};

use parley::{FontContext, LayoutContext};
use taffy::{AvailableSpace, NodeId as TaffyId, Size as TaffySize, Style as TaffyStyle, TaffyTree};

use std::sync::Arc;

use vello::peniko::ImageData;

use crate::color::Color;
use crate::element::id::ElementId;
use crate::element::kind::ElementKind;
use crate::element::scene_build;
use crate::element::style::StyleProp;
use crate::element::taffy_bridge::{self, MeasureCtx};
use crate::element::text::{self, TextBrush, TextLayout};
use crate::node::SceneGraph;

#[derive(Clone, Debug)]
pub struct Visual {
    pub background_color: Option<Color>,
    pub opacity: f32,
    pub border_radius: f32,
    pub border_width: f32,
    pub border_color: Option<Color>,
    pub text_color: Color,
    pub font_size: f32,
    pub z_index: i32,
    /// Custom font-family name registered via `register_font`.
    pub font_family: Option<String>,
}

impl Default for Visual {
    fn default() -> Self {
        Self {
            background_color: None,
            opacity: 1.0,
            border_radius: 0.0,
            border_width: 0.0,
            border_color: None,
            text_color: Color::BLACK,
            font_size: 16.0,
            z_index: 0,
            font_family: None,
        }
    }
}

pub(crate) struct Element {
    pub kind: ElementKind,
    pub parent: Option<ElementId>,
    pub children: Vec<ElementId>,
    pub taffy_node: TaffyId,
    pub layout_style: TaffyStyle,
    pub visual: Visual,
    pub text: Option<String>,
    pub src: Option<String>,
    pub text_layout: Option<TextLayout>,
    /// Optional affine transform applied on top of layout (kurbo coefficients [a,b,c,d,e,f]).
    pub transform: Option<[f64; 6]>,
    /// Scroll offset for ScrollView elements (x, y in pixels).
    pub scroll_offset: (f32, f32),
    /// Loaded image data for Image elements (populated by the adapter after async fetch).
    pub src_image: Option<Arc<ImageData>>,
    /// Editable text value for TextInput elements.
    pub text_content: String,
    /// IME preedit (in-progress composition, not yet committed).
    pub preedit: Option<String>,
    /// Byte offset of the insertion cursor within text_content (TextInput only).
    pub cursor_byte_index: usize,
    /// Whether the cursor should be drawn (true when the element is focused).
    pub cursor_visible: bool,
    /// Pre-built Parley layout of text_content + preedit, rebuilt each render pass.
    pub content_layout: Option<TextLayout>,
    /// ARIA label for screen readers.
    pub aria_label: Option<String>,
    /// ARIA role (e.g. "button", "listitem"). None uses the implicit role.
    pub role: Option<String>,
}

/// Events emitted by input wiring and drained by `poll_events`.
///
/// Naming follows ADR-0031: semantic state transitions (`hover-enter`,
/// `active-start`, …) instead of physical Pointer Events names. The single
/// exception is `PointerMove`, which is a coordinate stream with no target.
#[derive(Clone, Debug)]
pub enum Event {
    Click { target: ElementId, x: f32, y: f32 },
    Focus(ElementId),
    Blur(ElementId),
    TextInput { target: ElementId, text: String },
    CompositionStart { target: ElementId, text: String },
    CompositionUpdate { target: ElementId, text: String },
    CompositionEnd { target: ElementId, text: String },
    Scroll { target: ElementId, delta_x: f32, delta_y: f32 },
    Resize { width: f32, height: f32 },
    HoverEnter { target: ElementId },
    HoverLeave { target: ElementId },
    ActiveStart { target: ElementId },
    ActiveEnd { target: ElementId },
    PointerMove { x: f32, y: f32 },
    KeyDown { target: ElementId, key: String, modifiers: u32 },
    /// A font family with .notdef glyphs was detected during shaping.
    /// The adapter should fetch the font and call `load_font_from_url`.
    FetchFont { family: String },
}

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
    pub text_color: Color,
    pub font_size: f32,
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
    pub(crate) taffy: TaffyTree<MeasureCtx>,
    pub(crate) font_cx: FontContext,
    pub(crate) layout_cx: LayoutContext<TextBrush>,
    pub(crate) viewport: (f32, f32),
    pub(crate) scene_cache: SceneGraph,
    pub(crate) event_queue: Vec<Event>,
    /// Absolute bounding rects (x, y, w, h) per element, refreshed after each layout pass.
    pub(crate) layout_cache: HashMap<ElementId, (f32, f32, f32, f32)>,
    /// Element that owns the text-input cursor blink. Tracked here (not in the
    /// adapter) so `render(timestamp_ms)` can advance the blink itself per ADR-0032.
    pub(crate) focused_element: Option<ElementId>,
    /// Wall-clock millis (host-provided) of the last cursor-visibility toggle.
    /// `None` until the first frame after focus; reset on focus change.
    pub(crate) last_cursor_toggle_ms: Option<f64>,
    /// Set by `register_font`; cleared at the start of the next `compute_layout`.
    /// Causes all text elements to be re-shaped with the newly registered font.
    pub(crate) fonts_dirty: bool,
    /// Family names already requested via `FetchFont` but not yet loaded.
    /// Prevents duplicate events for the same family across frames.
    pub(crate) pending_font_fetches: HashSet<String>,
}

fn init_bundled_fonts(font_cx: &mut FontContext) {
    use fontique::{FontInfoOverride, GenericFamily};
    use vello::peniko::Blob;

    static NOTO_SANS_BYTES: &[u8] =
        include_bytes!("../../assets/fonts/NotoSansJP.ttf");

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

impl ElementTree {
    pub fn new() -> Self {
        let mut font_cx = FontContext::new();
        init_bundled_fonts(&mut font_cx);
        Self {
            elements: HashMap::new(),
            root: None,
            taffy: TaffyTree::new(),
            font_cx,
            layout_cx: LayoutContext::new(),
            viewport: (800.0, 600.0),
            scene_cache: SceneGraph::new(),
            event_queue: Vec::new(),
            layout_cache: HashMap::new(),
            focused_element: None,
            last_cursor_toggle_ms: None,
            fonts_dirty: false,
            pending_font_fetches: HashSet::new(),
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
        let layout_style = TaffyStyle::default();
        let measure_ctx = if kind.is_text_like() {
            MeasureCtx::Text(id)
        } else {
            MeasureCtx::None
        };
        let taffy_node = self.taffy
            .new_leaf_with_context(layout_style.clone(), measure_ctx)
            .expect("taffy new_leaf_with_context");

        let element = Element {
            kind,
            parent: None,
            children: Vec::new(),
            taffy_node,
            layout_style,
            visual: Visual::default(),
            text: None,
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
        el.text = Some(text.to_string());
        el.text_layout = None;
        let taffy_node = el.taffy_node;
        let _ = self.taffy.mark_dirty(taffy_node);
    }

    pub fn element_set_src(&mut self, id: ElementId, url: &str) {
        if let Some(el) = self.elements.get_mut(&id) {
            el.src = Some(url.to_string());
            el.src_image = None; // invalidate any previously loaded image
        }
    }

    /// Store decoded image data for an Image element (called by the adapter after async load).
    pub fn element_set_image(&mut self, id: ElementId, image: Arc<ImageData>) {
        if let Some(el) = self.elements.get_mut(&id) {
            el.src_image = Some(image);
        }
    }

    /// Replace the editable text content of a TextInput element.
    pub fn element_set_text_content(&mut self, id: ElementId, text: &str) {
        if let Some(el) = self.elements.get_mut(&id) {
            el.text_content = text.to_string();
            el.preedit = None;
            el.cursor_byte_index = el.text_content.len();
        }
    }

    /// Append text to a TextInput's committed content.
    pub fn element_append_text_content(&mut self, id: ElementId, text: &str) {
        if let Some(el) = self.elements.get_mut(&id) {
            el.text_content.push_str(text);
            el.cursor_byte_index = el.text_content.len();
        }
    }

    /// Remove the last Unicode scalar value from a TextInput's committed content.
    pub fn element_backspace(&mut self, id: ElementId) {
        if let Some(el) = self.elements.get_mut(&id) {
            if el.kind == ElementKind::TextInput && !el.text_content.is_empty() {
                let last_start = el.text_content
                    .char_indices()
                    .next_back()
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                el.text_content.truncate(last_start);
                el.cursor_byte_index = el.text_content.len();
            }
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
        self.last_cursor_toggle_ms = None;
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
        self.last_cursor_toggle_ms = None;
    }

    /// Currently-focused element, if any.
    pub fn focused_element(&self) -> Option<ElementId> {
        self.focused_element
    }

    /// Set the font family (by name) for an element. The family must first be registered via
    /// `register_font`, or be a system font available in the default FontContext.
    pub fn element_set_font_family(&mut self, id: ElementId, family: &str) {
        if let Some(el) = self.elements.get_mut(&id) {
            el.visual.font_family = if family.is_empty() { None } else { Some(family.to_string()) };
            el.text_layout = None;
            el.content_layout = None;
            let taffy_node = el.taffy_node;
            let _ = self.taffy.mark_dirty(taffy_node);
        }
    }

    /// Set the ARIA label for screen-reader accessibility.
    pub fn element_set_aria_label(&mut self, id: ElementId, label: &str) {
        if let Some(el) = self.elements.get_mut(&id) {
            el.aria_label = if label.is_empty() { None } else { Some(label.to_string()) };
        }
    }

    /// Set the ARIA role (e.g. "button", "listitem", "img"). Pass an empty string to clear.
    pub fn element_set_role(&mut self, id: ElementId, role: &str) {
        if let Some(el) = self.elements.get_mut(&id) {
            el.role = if role.is_empty() { None } else { Some(role.to_string()) };
        }
    }

    /// Register a custom font from raw bytes with a given family name.
    /// After registration, the name can be used in `element_set_font_family`.
    pub fn register_font(&mut self, family_name: &str, bytes: Vec<u8>) {
        use fontique::FontInfoOverride;
        use std::sync::Arc;
        use vello::peniko::Blob;

        let data = Arc::new(bytes);

        // 要求名で登録（element_set_font_family による明示的な指定に対応）
        let blob = Blob::new(data.clone());
        let override_info = FontInfoOverride {
            family_name: Some(family_name),
            ..Default::default()
        };
        self.font_cx.collection.register_fonts(blob, Some(override_info));

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
            self.font_cx.collection.register_fonts(fallback_blob, Some(fallback_override));
        }

        self.pending_font_fetches.remove(family_name);
        self.fonts_dirty = true;
    }

    /// Register a font from raw bytes using the family name(s) embedded in the
    /// font file itself. Backs the WIT `element-load-font` export.
    pub fn register_font_bytes(&mut self, bytes: Vec<u8>) {
        use std::sync::Arc;
        use vello::peniko::Blob;
        let blob = Blob::new(Arc::new(bytes));
        self.font_cx.collection.register_fonts(blob, None);
    }

    /// Set the IME preedit for a TextInput (in-progress, not yet committed).
    pub fn element_set_preedit(&mut self, id: ElementId, preedit: &str) {
        if let Some(el) = self.elements.get_mut(&id) {
            el.preedit = if preedit.is_empty() { None } else { Some(preedit.to_string()) };
        }
    }

    /// Commit the current preedit text into text_content and clear the preedit.
    pub fn element_commit_preedit(&mut self, id: ElementId) {
        if let Some(el) = self.elements.get_mut(&id) {
            if let Some(preedit) = el.preedit.take() {
                el.text_content.push_str(&preedit);
            }
        }
    }

    /// Deliver pasted text to a TextInput: commits any active preedit, appends the
    /// pasted text, then queues a TextInput event. No-op for non-TextInput elements.
    pub fn element_paste(&mut self, id: ElementId, text: &str) {
        if text.is_empty() {
            return;
        }
        let el = match self.elements.get_mut(&id) {
            Some(e) if e.kind == ElementKind::TextInput => e,
            _ => return,
        };
        if let Some(preedit) = el.preedit.take() {
            el.text_content.push_str(&preedit);
        }
        el.text_content.push_str(text);
        self.event_queue.push(Event::TextInput { target: id, text: text.to_string() });
    }

    /// Return the combined display text (text_content + any active preedit) for a TextInput.
    pub fn element_get_text_content(&self, id: ElementId) -> String {
        let el = match self.elements.get(&id) {
            Some(e) => e,
            None => return String::new(),
        };
        match &el.preedit {
            Some(p) => format!("{}{}", el.text_content, p),
            None => el.text_content.clone(),
        }
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
        self.elements.get(&id).map_or((0.0, 0.0), |e| e.scroll_offset)
    }

    /// Return the absolute layout rect (x, y, w, h) from the last render pass.
    pub fn element_layout_rect(&self, id: ElementId) -> Option<(f32, f32, f32, f32)> {
        self.layout_cache.get(&id).copied()
    }

    /// Return the bounding dimensions of all descendants (content size) for a ScrollView.
    /// Values are relative to the element's own top-left corner.
    pub fn element_content_size(&self, id: ElementId) -> (f32, f32) {
        let &(ex, ey, _, _) = match self.layout_cache.get(&id) {
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
            if let Some(&(cx, cy, cw, ch)) = self.layout_cache.get(&child) {
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
            let node = el.taffy_node;
            let _ = self.taffy.set_style(node, style);
        } else if text_dirty {
            let node = el.taffy_node;
            let _ = self.taffy.mark_dirty(node);
        }
    }

    pub fn element_append_child(&mut self, parent: ElementId, child: ElementId) {
        if !self.elements.contains_key(&parent) || !self.elements.contains_key(&child) {
            return;
        }
        self.detach_from_current_parent(child);
        let (parent_taffy, child_taffy) = {
            let p = &self.elements[&parent];
            let c = &self.elements[&child];
            (p.taffy_node, c.taffy_node)
        };
        let _ = self.taffy.add_child(parent_taffy, child_taffy);
        self.elements.get_mut(&parent).unwrap().children.push(child);
        self.elements.get_mut(&child).unwrap().parent = Some(parent);
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
        let index = match self.elements[&parent].children.iter().position(|&c| c == before) {
            Some(i) => i,
            None => {
                // `before` is not a child of `parent`; append as a fallback.
                self.element_append_child(parent, child);
                return;
            }
        };
        let (parent_taffy, child_taffy) = {
            let p = &self.elements[&parent];
            let c = &self.elements[&child];
            (p.taffy_node, c.taffy_node)
        };
        let _ = self.taffy.insert_child_at_index(parent_taffy, index, child_taffy);
        self.elements.get_mut(&parent).unwrap().children.insert(index, child);
        self.elements.get_mut(&child).unwrap().parent = Some(parent);
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
        for node in to_remove.into_iter().rev() {
            if let Some(el) = self.elements.remove(&node) {
                let _ = self.taffy.remove(el.taffy_node);
            }
            if self.focused_element == Some(node) {
                self.focused_element = None;
                self.last_cursor_toggle_ms = None;
            }
        }
        if self.root == Some(id) {
            self.root = None;
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

    /// Run layout, lower the element tree into the scene graph, and return it.
    ///
    /// `timestamp_ms` is a monotonic host clock (e.g. `performance.now()`); it
    /// drives the focused TextInput's cursor blink without exposing
    /// `tick_cursor` to the host (ADR-0032).
    pub fn render(&mut self, timestamp_ms: f64) -> &SceneGraph {
        self.tick_cursor_blink(timestamp_ms);
        if let Some(root) = self.root {
            self.compute_layout(root);
            self.layout_cache.clear();
            cache_layout(&self.elements, &self.taffy, root, 0.0, 0.0, &mut self.layout_cache);
        }
        self.scene_cache = scene_build::build(self);
        &self.scene_cache
    }

    /// Toggle the focused element's cursor every 500 ms. Idempotent if called
    /// multiple times in the same frame, and a no-op when nothing is focused.
    fn tick_cursor_blink(&mut self, timestamp_ms: f64) {
        let focused = match self.focused_element {
            Some(id) => id,
            None => return,
        };
        match self.last_cursor_toggle_ms {
            None => {
                // First frame after focus: keep cursor visible, start the clock.
                self.last_cursor_toggle_ms = Some(timestamp_ms);
                if let Some(el) = self.elements.get_mut(&focused) {
                    el.cursor_visible = true;
                }
            }
            Some(prev) if timestamp_ms - prev >= 500.0 => {
                self.last_cursor_toggle_ms = Some(timestamp_ms);
                if let Some(el) = self.elements.get_mut(&focused) {
                    el.cursor_visible = !el.cursor_visible;
                }
            }
            _ => {}
        }
    }

    pub fn scene_graph(&self) -> &SceneGraph {
        &self.scene_cache
    }

    pub fn poll_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.event_queue)
    }

    /// Append an event to the outgoing queue.
    pub fn push_event(&mut self, event: Event) {
        self.event_queue.push(event);
    }

    /// Returns true if at least one layout pass has completed (layout_cache is populated).
    pub fn has_layout(&self) -> bool {
        !self.layout_cache.is_empty()
    }

    /// Returns the deepest element whose bounding rect contains (x, y),
    /// or None if no element is hit. Uses the layout from the last render pass.
    pub fn hit_test(&self, x: f32, y: f32) -> Option<ElementId> {
        let root = self.root?;
        hit_test_walk(&self.layout_cache, &self.elements, root, x, y)
    }

    /// Run layout and return every element with its absolute position and visual state.
    /// Keyed by stable ElementId — safe to use as a DOM node mapping key across frames.
    pub fn resolved_elements(&mut self) -> Vec<(ElementId, ResolvedElement)> {
        if let Some(root) = self.root {
            self.compute_layout(root);
            self.layout_cache.clear();
            cache_layout(&self.elements, &self.taffy, root, 0.0, 0.0, &mut self.layout_cache);
        }
        let mut out = Vec::new();
        if let Some(root) = self.root {
            walk_resolved(&self.elements, &self.taffy, root, 0.0, 0.0, &mut out);
        }
        out
    }

    // ── internals ────────────────────────────────────────────────────────

    fn detach_from_current_parent(&mut self, child: ElementId) {
        let parent = match self.elements.get(&child).and_then(|c| c.parent) {
            Some(p) => p,
            None => return,
        };
        let (parent_taffy, child_taffy) = {
            let p = &self.elements[&parent];
            let c = &self.elements[&child];
            (p.taffy_node, c.taffy_node)
        };
        let _ = self.taffy.remove_child(parent_taffy, child_taffy);
        self.elements.get_mut(&parent).unwrap().children.retain(|&c| c != child);
        self.elements.get_mut(&child).unwrap().parent = None;
    }

    fn compute_layout(&mut self, root: ElementId) {
        // When a new font was registered, invalidate all text layouts so they
        // are re-shaped with the new font data on this pass.
        if self.fonts_dirty {
            self.fonts_dirty = false;
            let text_ids: Vec<ElementId> = self.elements.iter()
                .filter_map(|(id, el)| {
                    if el.kind.is_text_like() { Some(*id) } else { None }
                })
                .collect();
            for id in text_ids {
                if let Some(el) = self.elements.get_mut(&id) {
                    el.text_layout = None;
                    el.content_layout = None;
                    let node = el.taffy_node;
                    let _ = self.taffy.mark_dirty(node);
                }
            }
        }

        let root_taffy = self.elements[&root].taffy_node;
        let available = TaffySize {
            width: AvailableSpace::Definite(self.viewport.0),
            height: AvailableSpace::Definite(self.viewport.1),
        };

        let Self {
            taffy,
            elements,
            font_cx,
            layout_cx,
            event_queue,
            pending_font_fetches,
            ..
        } = self;

        // Two-pass: stash text layouts produced inside the measure closure,
        // then drain them back onto the elements once compute_layout returns.
        let mut pending: HashMap<ElementId, TextLayout> = HashMap::new();
        let _ = taffy.compute_layout_with_measure(
            root_taffy,
            available,
            |known_dims, available_space, _node_id, ctx, _style| {
                let eid = match ctx {
                    Some(MeasureCtx::Text(eid)) => *eid,
                    _ => return TaffySize::ZERO,
                };
                let el = match elements.get(&eid) {
                    Some(e) => e,
                    None => return TaffySize::ZERO,
                };
                let text = match el.text.as_deref() {
                    Some(s) if !s.is_empty() => s,
                    _ => return TaffySize::ZERO,
                };

                let max_advance = match known_dims.width {
                    Some(w) => Some(w),
                    None => match available_space.width {
                        AvailableSpace::Definite(w) => Some(w),
                        AvailableSpace::MaxContent => None,
                        AvailableSpace::MinContent => Some(0.0),
                    },
                };
                let layout = text::build_text_layout(
                    font_cx,
                    layout_cx,
                    text,
                    el.visual.font_size,
                    max_advance,
                    el.visual.font_family.as_deref(),
                );
                let size = TaffySize {
                    width: layout.layout.width(),
                    height: layout.layout.height(),
                };
                pending.insert(eid, layout);
                size
            },
        );

        for (eid, mut layout) in pending {
            // Re-stamp the source text onto each lowered run so HTML mode can
            // place it back into a DOM text node.
            let src: std::sync::Arc<str> = layout.text.clone();
            for run in &mut layout.runs {
                if let Some(rd) = std::sync::Arc::get_mut(run) {
                    rd.text = src.clone();
                }
            }
            for &fam in &layout.missing_families {
                if !pending_font_fetches.contains(fam) {
                    pending_font_fetches.insert(fam.to_string());
                    event_queue.push(Event::FetchFont { family: fam.to_string() });
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
        }

        // Build content layouts for TextInput elements (used for Canvas-mode rendering + cursor).
        let textinput_ids: Vec<ElementId> = elements
            .iter()
            .filter_map(|(id, el)| {
                if el.kind == ElementKind::TextInput { Some(*id) } else { None }
            })
            .collect();

        for eid in textinput_ids {
            let (display_text, font_size) = {
                let el = match elements.get(&eid) {
                    Some(e) => e,
                    None => continue,
                };
                let text = match &el.preedit {
                    Some(p) => format!("{}{}", el.text_content, p),
                    None => el.text_content.clone(),
                };
                (text, el.visual.font_size)
            };

            if display_text.is_empty() {
                if let Some(el) = elements.get_mut(&eid) {
                    el.content_layout = None;
                }
                continue;
            }

            let (max_advance, font_family) = {
                let el = elements.get(&eid).map(|e| (
                    taffy.layout(e.taffy_node).ok().map(|l| l.size.width),
                    e.visual.font_family.clone(),
                ));
                el.map(|(a, f)| (a, f)).unwrap_or((None, None))
            };
            let content_layout = text::build_text_layout(
                font_cx,
                layout_cx,
                &display_text,
                font_size,
                max_advance,
                font_family.as_deref(),
            );

            for &fam in &content_layout.missing_families {
                if !pending_font_fetches.contains(fam) {
                    pending_font_fetches.insert(fam.to_string());
                    event_queue.push(Event::FetchFont { family: fam.to_string() });
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
                el.cursor_byte_index = el.text_content.len();
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
    taffy: &TaffyTree<MeasureCtx>,
    id: ElementId,
    ox: f32,
    oy: f32,
    out: &mut Vec<(ElementId, ResolvedElement)>,
) {
    let el = match elements.get(&id) {
        Some(e) => e,
        None => return,
    };
    let layout = match taffy.layout(el.taffy_node) {
        Ok(l) => l,
        Err(_) => return,
    };
    let x = ox + layout.location.x;
    let y = oy + layout.location.y;

    let display_text_content = if el.kind == ElementKind::TextInput {
        let combined = match &el.preedit {
            Some(p) => format!("{}{}", el.text_content, p),
            None => el.text_content.clone(),
        };
        Some(combined)
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
            background_color: el.visual.background_color,
            opacity: el.visual.opacity,
            border_radius: el.visual.border_radius,
            border_width: el.visual.border_width,
            border_color: el.visual.border_color,
            text_color: el.visual.text_color,
            font_size: el.visual.font_size,
            z_index: el.visual.z_index,
            text: el.text.clone(),
            src: el.src.clone(),
            text_content: display_text_content,
            font_family: el.visual.font_family.clone(),
            aria_label: el.aria_label.clone(),
            role: el.role.clone(),
        },
    ));

    for &child in &el.children {
        walk_resolved(elements, taffy, child, x, y, out);
    }
}

fn cache_layout(
    elements: &HashMap<ElementId, Element>,
    taffy: &TaffyTree<MeasureCtx>,
    id: ElementId,
    ox: f32,
    oy: f32,
    cache: &mut HashMap<ElementId, (f32, f32, f32, f32)>,
) {
    let el = match elements.get(&id) {
        Some(e) => e,
        None => return,
    };
    let layout = match taffy.layout(el.taffy_node) {
        Ok(l) => l,
        Err(_) => return,
    };
    let x = ox + layout.location.x;
    let y = oy + layout.location.y;
    cache.insert(id, (x, y, layout.size.width, layout.size.height));
    for &child in &el.children {
        cache_layout(elements, taffy, child, x, y, cache);
    }
}

fn hit_test_walk(
    cache: &HashMap<ElementId, (f32, f32, f32, f32)>,
    elements: &HashMap<ElementId, Element>,
    id: ElementId,
    x: f32,
    y: f32,
) -> Option<ElementId> {
    let &(ex, ey, ew, eh) = cache.get(&id)?;
    if x < ex || y < ey || x >= ex + ew || y >= ey + eh {
        return None;
    }
    let el = elements.get(&id)?;
    // Visit children in reverse paint order so the topmost element wins.
    // scene_build sorts children by ascending z-index (stable, so equal z's
    // keep document order); the reverse is descending z-index, ties in
    // reverse document order.
    let mut ordered: Vec<(usize, ElementId, i32)> = el
        .children
        .iter()
        .enumerate()
        .map(|(idx, &cid)| (idx, cid, elements.get(&cid).map_or(0, |c| c.visual.z_index)))
        .collect();
    ordered.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| b.0.cmp(&a.0)));
    for (_, child, _) in ordered {
        if let Some(hit) = hit_test_walk(cache, elements, child, x, y) {
            return Some(hit);
        }
    }
    Some(id)
}

fn apply_visual(visual: &mut Visual, prop: &StyleProp, text_dirty: &mut bool) {
    match prop {
        StyleProp::BackgroundColor(c) => visual.background_color = Some(*c),
        StyleProp::Opacity(v) => visual.opacity = v.clamp(0.0, 1.0),
        StyleProp::BorderRadius(v) => visual.border_radius = v.max(0.0),
        StyleProp::BorderWidth(v) => visual.border_width = v.max(0.0),
        StyleProp::BorderColor(c) => visual.border_color = Some(*c),
        StyleProp::FontSize(v) => {
            visual.font_size = v.max(0.0);
            *text_dirty = true;
        }
        StyleProp::FontFamily(f) => {
            visual.font_family = if f.is_empty() { None } else { Some(f.clone()) };
            *text_dirty = true;
        }
        StyleProp::Color(c) => {
            visual.text_color = *c;
            *text_dirty = true;
        }
        StyleProp::ZIndex(z) => visual.z_index = *z,
        _ => {}
    }
}
