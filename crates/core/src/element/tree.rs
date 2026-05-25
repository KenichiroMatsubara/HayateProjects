use std::collections::HashMap;

use parley::{FontContext, LayoutContext};
use slotmap::SlotMap;
use taffy::{AvailableSpace, NodeId as TaffyId, Size as TaffySize, Style as TaffyStyle, TaffyTree};

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
}

/// Events drained by `poll_events`. Placeholder until input wiring lands.
#[derive(Clone, Debug)]
pub enum Event {}

pub struct ElementTree {
    pub(crate) elements: SlotMap<ElementId, Element>,
    pub(crate) root: Option<ElementId>,
    pub(crate) taffy: TaffyTree<MeasureCtx>,
    pub(crate) font_cx: FontContext,
    pub(crate) layout_cx: LayoutContext<TextBrush>,
    pub(crate) viewport: (f32, f32),
    pub(crate) scene_cache: SceneGraph,
}

impl ElementTree {
    pub fn new() -> Self {
        Self {
            elements: SlotMap::with_key(),
            root: None,
            taffy: TaffyTree::new(),
            font_cx: FontContext::new(),
            layout_cx: LayoutContext::new(),
            viewport: (800.0, 600.0),
            scene_cache: SceneGraph::new(),
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
        debug_assert!(self.elements.contains_key(id), "set_root: unknown id");
        self.root = Some(id);
    }

    pub fn element_create(&mut self, kind: ElementKind) -> ElementId {
        let layout_style = TaffyStyle::default();
        // Text-like leaves get a measure context so Taffy invokes the closure.
        let taffy_node = if kind.is_text_like() {
            // Placeholder ElementId; rewritten below once we know the slotmap key.
            self.taffy
                .new_leaf_with_context(layout_style.clone(), MeasureCtx::None)
                .expect("taffy new_leaf_with_context")
        } else {
            self.taffy
                .new_leaf_with_context(layout_style.clone(), MeasureCtx::None)
                .expect("taffy new_leaf_with_context")
        };

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
        };
        let id = self.elements.insert(element);

        if kind.is_text_like() {
            self.taffy
                .set_node_context(taffy_node, Some(MeasureCtx::Text(id)))
                .expect("set_node_context");
        }

        if self.root.is_none() {
            self.root = Some(id);
        }
        id
    }

    pub fn element_set_text(&mut self, id: ElementId, text: &str) {
        let el = match self.elements.get_mut(id) {
            Some(e) => e,
            None => return,
        };
        el.text = Some(text.to_string());
        el.text_layout = None;
        let taffy_node = el.taffy_node;
        let _ = self.taffy.mark_dirty(taffy_node);
    }

    pub fn element_set_src(&mut self, id: ElementId, url: &str) {
        if let Some(el) = self.elements.get_mut(id) {
            el.src = Some(url.to_string());
        }
    }

    pub fn element_set_style(&mut self, id: ElementId, props: &[StyleProp]) {
        let el = match self.elements.get_mut(id) {
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
        if !self.elements.contains_key(parent) || !self.elements.contains_key(child) {
            return;
        }
        self.detach_from_current_parent(child);
        let (parent_taffy, child_taffy) = {
            let p = &self.elements[parent];
            let c = &self.elements[child];
            (p.taffy_node, c.taffy_node)
        };
        let _ = self.taffy.add_child(parent_taffy, child_taffy);
        self.elements[parent].children.push(child);
        self.elements[child].parent = Some(parent);
    }

    pub fn element_insert_before(
        &mut self,
        parent: ElementId,
        child: ElementId,
        before: ElementId,
    ) {
        if !self.elements.contains_key(parent)
            || !self.elements.contains_key(child)
            || !self.elements.contains_key(before)
        {
            return;
        }
        self.detach_from_current_parent(child);
        let index = match self.elements[parent].children.iter().position(|&c| c == before) {
            Some(i) => i,
            None => {
                // `before` is not a child of `parent`; append as a fallback.
                self.element_append_child(parent, child);
                return;
            }
        };
        let (parent_taffy, child_taffy) = {
            let p = &self.elements[parent];
            let c = &self.elements[child];
            (p.taffy_node, c.taffy_node)
        };
        let _ = self.taffy.insert_child_at_index(parent_taffy, index, child_taffy);
        self.elements[parent].children.insert(index, child);
        self.elements[child].parent = Some(parent);
    }

    pub fn element_remove(&mut self, id: ElementId) {
        if !self.elements.contains_key(id) {
            return;
        }
        self.detach_from_current_parent(id);
        // Recursively remove the subtree.
        let mut stack = vec![id];
        let mut to_remove = Vec::new();
        while let Some(node) = stack.pop() {
            to_remove.push(node);
            if let Some(el) = self.elements.get(node) {
                stack.extend(el.children.iter().copied());
            }
        }
        for node in to_remove.into_iter().rev() {
            if let Some(el) = self.elements.remove(node) {
                let _ = self.taffy.remove(el.taffy_node);
            }
        }
        if self.root == Some(id) {
            self.root = None;
        }
    }

    pub fn element_get_text(&self, id: ElementId) -> String {
        self.elements
            .get(id)
            .and_then(|e| e.text.clone())
            .unwrap_or_default()
    }

    pub fn element_kind(&self, id: ElementId) -> Option<ElementKind> {
        self.elements.get(id).map(|e| e.kind)
    }

    /// Run layout, lower the element tree into the scene graph, and return it.
    pub fn render(&mut self) -> &SceneGraph {
        if let Some(root) = self.root {
            self.compute_layout(root);
        }
        self.scene_cache = scene_build::build(self);
        &self.scene_cache
    }

    pub fn scene_graph(&self) -> &SceneGraph {
        &self.scene_cache
    }

    pub fn poll_events(&mut self) -> Vec<Event> {
        Vec::new()
    }

    // ── internals ────────────────────────────────────────────────────────

    fn detach_from_current_parent(&mut self, child: ElementId) {
        let parent = match self.elements.get(child).and_then(|c| c.parent) {
            Some(p) => p,
            None => return,
        };
        let (parent_taffy, child_taffy) = {
            let p = &self.elements[parent];
            let c = &self.elements[child];
            (p.taffy_node, c.taffy_node)
        };
        let _ = self.taffy.remove_child(parent_taffy, child_taffy);
        let p = &mut self.elements[parent];
        p.children.retain(|&c| c != child);
        self.elements[child].parent = None;
    }

    fn compute_layout(&mut self, root: ElementId) {
        let root_taffy = self.elements[root].taffy_node;
        let available = TaffySize {
            width: AvailableSpace::Definite(self.viewport.0),
            height: AvailableSpace::Definite(self.viewport.1),
        };

        let Self {
            taffy,
            elements,
            font_cx,
            layout_cx,
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
                let el = match elements.get(eid) {
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
            if let Some(el) = elements.get_mut(eid) {
                el.text_layout = Some(layout);
            }
        }
    }
}

impl Default for ElementTree {
    fn default() -> Self {
        Self::new()
    }
}

fn apply_visual(visual: &mut Visual, prop: &StyleProp, text_dirty: &mut bool) {
    match *prop {
        StyleProp::BackgroundColor(c) => visual.background_color = Some(c),
        StyleProp::Opacity(v) => visual.opacity = v.clamp(0.0, 1.0),
        StyleProp::BorderRadius(v) => visual.border_radius = v.max(0.0),
        StyleProp::BorderWidth(v) => visual.border_width = v.max(0.0),
        StyleProp::BorderColor(c) => visual.border_color = Some(c),
        StyleProp::FontSize(v) => {
            visual.font_size = v.max(0.0);
            *text_dirty = true;
        }
        StyleProp::Color(c) => {
            visual.text_color = c;
            *text_dirty = true;
        }
        _ => {}
    }
}
