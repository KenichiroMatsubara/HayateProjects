use hayate_core::{event_document_kind, ElementId, ElementTree, Event};

/// Deliver `event` through the document runtime when `tree` is present.
///
/// Without a tree, events accumulate in `raw_events` (unit-test / null-tree only;
/// both Canvas and HTML renderers always pass `Some(tree)` in production).
pub(crate) fn emit_event(
    tree: &mut Option<&mut ElementTree>,
    raw_fallback: &mut Vec<Event>,
    event: Event,
) {
    if let Some(t) = tree.as_mut() {
        if let Some(kind) = event_document_kind(&event) {
            t.dispatch_event(kind, event);
        }
    } else {
        raw_fallback.push(event);
    }
}

/// Shared input-handling state for both renderer backends.
///
/// Tracks hover/active/focus pointer state. Canvas and HTML renderers pass
/// `Some(tree)` so interaction events route through the document runtime and
/// surface as poll deliveries via `ElementTree::poll_deliveries()`.
pub(crate) struct RendererEventState {
    pub hovered_element: Option<ElementId>,
    pub active_element: Option<ElementId>,
    /// Focused element for event routing. Canvas mode mirrors this into
    /// `ElementTree` for cursor-blink rendering.
    pub focused_element: Option<ElementId>,
    pub last_pointer_pos: Option<(f32, f32)>,
    raw_events: Vec<Event>,
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
impl RendererEventState {
    pub fn new() -> Self {
        Self {
            hovered_element: None,
            active_element: None,
            focused_element: None,
            last_pointer_pos: None,
            raw_events: Vec::new(),
        }
    }

    #[cfg(test)]
    pub fn drain_raw(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.raw_events)
    }

    pub fn focus(&mut self, tree: Option<&mut ElementTree>, id: ElementId) {
        if self.focused_element == Some(id) {
            return;
        }
        let mut tree = tree;
        if let Some(prev) = self.focused_element {
            emit_event(
                &mut tree,
                &mut self.raw_events,
                Event::Blur {
                    target_id: prev,
                },
            );
        }
        self.focused_element = Some(id);
        emit_event(
            &mut tree,
            &mut self.raw_events,
            Event::Focus {
                target_id: id,
            },
        );
    }

    pub fn blur(&mut self, tree: Option<&mut ElementTree>, id: ElementId) {
        if self.focused_element != Some(id) {
            return;
        }
        let mut tree = tree;
        self.focused_element = None;
        emit_event(
            &mut tree,
            &mut self.raw_events,
            Event::Blur {
                target_id: id,
            },
        );
    }

    pub fn pointer_down(
        &mut self,
        tree: Option<&mut ElementTree>,
        target: Option<ElementId>,
        x: f32,
        y: f32,
    ) {
        let mut tree = tree;
        if let Some(t) = target {
            emit_event(
                &mut tree,
                &mut self.raw_events,
                Event::Click {
                    target_id: t,
                    x,
                    y,
                },
            );
            emit_event(
                &mut tree,
                &mut self.raw_events,
                Event::ActiveStart { target_id: t },
            );
            self.active_element = Some(t);
            self.focus(tree, t);
        } else if let Some(prev) = self.focused_element.take() {
            emit_event(
                &mut tree,
                &mut self.raw_events,
                Event::Blur {
                    target_id: prev,
                },
            );
        }
    }

    pub fn pointer_up(&mut self, tree: Option<&mut ElementTree>, explicit_fallback: Option<ElementId>) {
        let mut tree = tree;
        let target = self.active_element.take().or(explicit_fallback);
        if let Some(t) = target {
            emit_event(
                &mut tree,
                &mut self.raw_events,
                Event::ActiveEnd { target_id: t },
            );
        }
    }

    pub fn pointer_move_to(
        &mut self,
        tree: Option<&mut ElementTree>,
        new_hover: Option<ElementId>,
        x: f32,
        y: f32,
    ) -> bool {
        if let Some((lx, ly)) = self.last_pointer_pos {
            if (x - lx).abs() < 1.0 && (y - ly).abs() < 1.0 {
                return false;
            }
        }
        self.last_pointer_pos = Some((x, y));
        let mut tree = tree;
        emit_event(
            &mut tree,
            &mut self.raw_events,
            Event::PointerMove { x, y },
        );
        self.apply_hover(tree, new_hover);
        true
    }

    pub fn hover_enter(&mut self, tree: Option<&mut ElementTree>, target: ElementId) {
        let mut tree = tree;
        if self.hovered_element != Some(target) {
            if let Some(prev) = self.hovered_element {
                emit_event(
                    &mut tree,
                    &mut self.raw_events,
                    Event::HoverLeave { target_id: prev },
                );
            }
            self.hovered_element = Some(target);
            emit_event(
                &mut tree,
                &mut self.raw_events,
                Event::HoverEnter { target_id: target },
            );
        }
    }

    pub fn hover_leave(&mut self, tree: Option<&mut ElementTree>, target: ElementId) {
        let mut tree = tree;
        if self.hovered_element == Some(target) {
            self.hovered_element = None;
            emit_event(
                &mut tree,
                &mut self.raw_events,
                Event::HoverLeave { target_id: target },
            );
        }
    }

    pub fn wheel(
        &mut self,
        tree: Option<&mut ElementTree>,
        target: ElementId,
        delta_x: f32,
        delta_y: f32,
    ) {
        let mut tree = tree;
        emit_event(
            &mut tree,
            &mut self.raw_events,
            Event::Scroll {
                target_id: target,
                delta_x,
                delta_y,
            },
        );
    }

    pub fn resize(&mut self, tree: Option<&mut ElementTree>, width: f32, height: f32) {
        let mut tree = tree;
        emit_event(
            &mut tree,
            &mut self.raw_events,
            Event::Resize { width, height },
        );
    }

    pub fn key_down(&mut self, tree: Option<&mut ElementTree>, key: &str, modifiers: u32) {
        let mut tree = tree;
        if let Some(focused) = self.focused_element {
            emit_event(
                &mut tree,
                &mut self.raw_events,
                Event::KeyDown {
                    target_id: focused,
                    key: key.to_string(),
                    modifiers,
                },
            );
        }
    }

    pub fn text_input(&mut self, tree: Option<&mut ElementTree>, target: ElementId, text: &str) {
        let mut tree = tree;
        emit_event(
            &mut tree,
            &mut self.raw_events,
            Event::TextInput {
                target_id: target,
                text: text.to_string(),
            },
        );
    }

    pub fn composition_start(&mut self, tree: Option<&mut ElementTree>, target: ElementId, text: &str) {
        let mut tree = tree;
        emit_event(
            &mut tree,
            &mut self.raw_events,
            Event::CompositionStart {
                target_id: target,
                text: text.to_string(),
            },
        );
    }

    pub fn composition_update(
        &mut self,
        tree: Option<&mut ElementTree>,
        target: ElementId,
        text: &str,
    ) {
        let mut tree = tree;
        emit_event(
            &mut tree,
            &mut self.raw_events,
            Event::CompositionUpdate {
                target_id: target,
                text: text.to_string(),
            },
        );
    }

    pub fn composition_end(&mut self, tree: Option<&mut ElementTree>, target: ElementId, text: &str) {
        let mut tree = tree;
        emit_event(
            &mut tree,
            &mut self.raw_events,
            Event::CompositionEnd {
                target_id: target,
                text: text.to_string(),
            },
        );
    }

    pub fn on_subtree_remove<F: Fn(ElementId) -> bool>(&mut self, in_subtree: F) {
        if let Some(h) = self.hovered_element {
            if in_subtree(h) {
                self.hovered_element = None;
            }
        }
        if let Some(a) = self.active_element {
            if in_subtree(a) {
                self.active_element = None;
            }
        }
        if let Some(f) = self.focused_element {
            if in_subtree(f) {
                self.focused_element = None;
            }
        }
    }

    fn apply_hover(&mut self, tree: Option<&mut ElementTree>, new_hover: Option<ElementId>) {
        let mut tree = tree;
        if new_hover != self.hovered_element {
            if let Some(prev) = self.hovered_element {
                emit_event(
                    &mut tree,
                    &mut self.raw_events,
                    Event::HoverLeave { target_id: prev },
                );
            }
            if let Some(cur) = new_hover {
                emit_event(
                    &mut tree,
                    &mut self.raw_events,
                    Event::HoverEnter { target_id: cur },
                );
            }
            self.hovered_element = new_hover;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hayate_core::{DocumentEventKind, ElementKind};

    #[test]
    fn with_tree_routes_click_to_deliveries_not_raw() {
        let mut tree = ElementTree::new();
        let btn = tree.element_create(1, ElementKind::Button);
        tree.set_root(btn);
        let listener = tree.register_listener(btn, DocumentEventKind::Click);

        let mut state = RendererEventState::new();
        state.pointer_down(Some(&mut tree), Some(btn), 10.0, 20.0);

        assert!(state.drain_raw().is_empty());
        let deliveries = tree.poll_deliveries();
        assert_eq!(deliveries.len(), 1);
        assert_eq!(deliveries[0].listener_id, listener);
        assert!(matches!(
            &deliveries[0].event,
            Event::Click { target_id, x, y }
                if *target_id == btn && (*x - 10.0).abs() < f32::EPSILON && (*y - 20.0).abs() < f32::EPSILON
        ));
    }

    #[test]
    fn pointer_move_skips_duplicate_coordinates() {
        let mut state = RendererEventState::new();
        assert!(state.pointer_move_to(None, None, 1.0, 2.0));
        assert!(!state.pointer_move_to(None, None, 1.0, 2.0));
        assert!(state.pointer_move_to(None, None, 2.0, 2.0));
    }

    #[test]
    fn without_tree_accumulates_raw_events_for_tests() {
        let mut state = RendererEventState::new();
        state.pointer_down(None, Some(ElementId::from_u64(1)), 0.0, 0.0);

        let raw = state.drain_raw();
        assert_eq!(raw.len(), 3); // Click + ActiveStart + Focus
        assert!(matches!(&raw[0], Event::Click { .. }));
        assert!(matches!(&raw[1], Event::ActiveStart { .. }));
        assert!(matches!(&raw[2], Event::Focus { .. }));
    }

    #[test]
    fn hover_with_tree_delivers_to_target_listener() {
        let mut tree = ElementTree::new();
        let root = tree.element_create(10, ElementKind::View);
        let child = tree.element_create(11, ElementKind::Button);
        tree.set_root(root);
        tree.element_append_child(root, child);
        let listener = tree.register_listener(child, DocumentEventKind::HoverEnter);

        let mut state = RendererEventState::new();
        state.hover_enter(Some(&mut tree), child);

        assert!(state.drain_raw().is_empty());
        let deliveries = tree.poll_deliveries();
        assert_eq!(deliveries.len(), 1);
        assert_eq!(deliveries[0].listener_id, listener);
        assert!(matches!(
            &deliveries[0].event,
            Event::HoverEnter { target_id } if *target_id == child
        ));
    }
}
