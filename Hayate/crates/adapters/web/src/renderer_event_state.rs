use hayate_core::{ElementId, Event};

/// Shared input-handling state for both renderer backends.
///
/// Tracks hover/active/focus pointer state and accumulates `Event`s in a local
/// queue. Both `HayateElementRenderer` (Canvas) and
/// `HayateElementHtmlRenderer` (HTML) embed this so the state-transition
/// logic lives in exactly one place.
///
/// Canvas mode must also call `tree.element_focus` / `tree.element_blur`
/// alongside `focus` / `blur` here to keep the cursor-blink render state in sync.
pub(crate) struct RendererEventState {
    pub hovered_element: Option<ElementId>,
    pub active_element: Option<ElementId>,
    /// Focused element for event routing. Canvas mode mirrors this into
    /// `ElementTree` for cursor-blink rendering.
    pub focused_element: Option<ElementId>,
    pub last_pointer_pos: Option<(f32, f32)>,
    events: Vec<Event>,
}

impl RendererEventState {
    pub fn new() -> Self {
        Self {
            hovered_element: None,
            active_element: None,
            focused_element: None,
            last_pointer_pos: None,
            events: Vec::new(),
        }
    }

    /// Push an arbitrary event into the queue.
    pub fn push(&mut self, event: Event) {
        self.events.push(event);
    }

    /// Drain all queued events, leaving the queue empty.
    pub fn drain(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// Set `id` as the focused element.
    /// Pushes `Blur(prev)` then `Focus(id)` if focus changed; no-op when already focused.
    pub fn focus(&mut self, id: ElementId) {
        if self.focused_element == Some(id) {
            return;
        }
        if let Some(prev) = self.focused_element {
            self.events.push(Event::Blur(prev));
        }
        self.focused_element = Some(id);
        self.events.push(Event::Focus(id));
    }

    /// Clear focus from `id`.
    /// Pushes `Blur(id)` if `id` is currently focused; no-op otherwise.
    pub fn blur(&mut self, id: ElementId) {
        if self.focused_element != Some(id) {
            return;
        }
        self.focused_element = None;
        self.events.push(Event::Blur(id));
    }

    /// Handle pointer-down on `target` (None = pointer missed all elements).
    /// Pushes Click + ActiveStart events and manages focus transitions via
    /// `focus()` / `blur()`.
    pub fn pointer_down(&mut self, target: Option<ElementId>, x: f32, y: f32) {
        if let Some(t) = target {
            self.events.push(Event::Click { target: t, x, y });
            self.events.push(Event::ActiveStart { target: t });
            self.active_element = Some(t);
            self.focus(t);
        } else if let Some(prev) = self.focused_element.take() {
            self.events.push(Event::Blur(prev));
        }
    }

    /// Handle pointer-up.
    /// Uses `active_element` if tracked; falls back to `explicit_fallback`
    /// (pass `None` when no position-based fallback is available).
    pub fn pointer_up(&mut self, explicit_fallback: Option<ElementId>) {
        let target = self.active_element.take().or(explicit_fallback);
        if let Some(t) = target {
            self.events.push(Event::ActiveEnd { target: t });
        }
    }

    /// Update hover state and push a PointerMove event for Canvas mode
    /// (where hover is derived from a hit-test at the new pointer position).
    ///
    /// Applies the 1 px throttle from ADR-0019; returns `false` when the move
    /// was below the threshold and no events were pushed.
    pub fn pointer_move_to(&mut self, new_hover: Option<ElementId>, x: f32, y: f32) -> bool {
        if let Some((lx, ly)) = self.last_pointer_pos {
            if (x - lx).abs() < 1.0 && (y - ly).abs() < 1.0 {
                return false;
            }
        }
        self.last_pointer_pos = Some((x, y));
        self.events.push(Event::PointerMove { x, y });
        self.apply_hover(new_hover);
        true
    }

    /// Handle DOM `mouseenter` (HTML mode): transition hover to `target`.
    pub fn hover_enter(&mut self, target: ElementId) {
        if self.hovered_element != Some(target) {
            if let Some(prev) = self.hovered_element {
                self.events.push(Event::HoverLeave { target: prev });
            }
            self.hovered_element = Some(target);
            self.events.push(Event::HoverEnter { target });
        }
    }

    /// Handle DOM `mouseleave` (HTML mode): clear hover from `target`.
    pub fn hover_leave(&mut self, target: ElementId) {
        if self.hovered_element == Some(target) {
            self.hovered_element = None;
            self.events.push(Event::HoverLeave { target });
        }
    }

    /// Push a Scroll event.
    pub fn wheel(&mut self, target: ElementId, delta_x: f32, delta_y: f32) {
        self.events.push(Event::Scroll {
            target,
            delta_x,
            delta_y,
        });
    }

    /// Push a Resize event.
    pub fn resize(&mut self, width: f32, height: f32) {
        self.events.push(Event::Resize { width, height });
    }

    /// Push a KeyDown event for the currently-focused element.
    /// No-op when nothing is focused.
    pub fn key_down(&mut self, key: &str, modifiers: u32) {
        if let Some(focused) = self.focused_element {
            self.events.push(Event::KeyDown {
                target: focused,
                key: key.to_string(),
                modifiers,
            });
        }
    }

    /// Push a TextInput event.
    pub fn text_input(&mut self, target: ElementId, text: &str) {
        self.events.push(Event::TextInput {
            target,
            text: text.to_string(),
        });
    }

    /// Push a CompositionStart event.
    pub fn composition_start(&mut self, target: ElementId, text: &str) {
        self.events.push(Event::CompositionStart {
            target,
            text: text.to_string(),
        });
    }

    /// Push a CompositionUpdate event.
    pub fn composition_update(&mut self, target: ElementId, text: &str) {
        self.events.push(Event::CompositionUpdate {
            target,
            text: text.to_string(),
        });
    }

    /// Push a CompositionEnd event.
    pub fn composition_end(&mut self, target: ElementId, text: &str) {
        self.events.push(Event::CompositionEnd {
            target,
            text: text.to_string(),
        });
    }

    /// Push a TextInput event (used for paste operations).
    pub fn paste(&mut self, target: ElementId, text: &str) {
        self.events.push(Event::TextInput {
            target,
            text: text.to_string(),
        });
    }

    /// Clear hover/active/focused state for elements belonging to a removed
    /// subtree.
    ///
    /// `in_subtree(id)` must return `true` when `id` should be cleared — for
    /// Canvas mode this means "is a descendant of the removed root" (called
    /// *before* tree removal); for HTML mode this means "is no longer in the
    /// node map" (called *after* removal).
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

    fn apply_hover(&mut self, new_hover: Option<ElementId>) {
        if new_hover != self.hovered_element {
            if let Some(prev) = self.hovered_element {
                self.events.push(Event::HoverLeave { target: prev });
            }
            if let Some(cur) = new_hover {
                self.events.push(Event::HoverEnter { target: cur });
            }
            self.hovered_element = new_hover;
        }
    }
}
