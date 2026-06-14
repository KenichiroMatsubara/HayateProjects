use crate::element::event_spec::{event_document_kind, DocumentEventKind, Event};
use crate::element::id::ElementId;
use crate::element::inline_text::{byte_index_at_point, ifc_root};
use crate::element::pseudo_state::PseudoState;
use crate::element::selection::{Selection, SelectionPoint};
use crate::element::style::CursorValue;
use crate::element::tree::ElementTree;
use crate::element::visual_invalidation::VisualInvalidationReach;

/// Output of `on_pointer_move` (ADR-0088). `moved` is false when the move was
/// coalesced by the 1px dedup or skipped because layout is not ready; `cursor`
/// carries the cursor resolved from the element under the pointer so the
/// Platform Adapter can drive the OS/browser cursor without touching styles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PointerMoveResult {
    pub moved: bool,
    pub resolved_cursor: CursorValue,
}

impl ElementTree {
    /// Pointer down at canvas coordinates (hit-test driven).
    pub fn on_pointer_down(&mut self, x: f32, y: f32) {
        let hit = self.hit_test(x, y);
        self.pointer_down_on_target(hit, x, y);
        // Selection drag rides on the same active-session capture (ADR-0097):
        // a press inside a Selection Region collapses the selection to a caret.
        self.begin_selection_at(x, y);
    }

    /// Pointer down on an explicit target (HTML Mode).
    pub fn on_pointer_down_on(&mut self, target: ElementId, x: f32, y: f32) {
        self.pointer_down_on_target(Some(target), x, y);
    }

    fn pointer_down_on_target(&mut self, target: Option<ElementId>, x: f32, y: f32) {
        if let Some(t) = target {
            self.emit_interaction(Event::Click {
                target_id: t,
                x,
                y,
            });
            self.emit_interaction(Event::ActiveStart { target_id: t });
            // Mark (and capture the transition's pre-switch visual) before the
            // active state flips, so `:active` transitions start from the
            // not-yet-active appearance (ADR-0089).
            self.mark_pseudo_activation_dirty(t, PseudoState::Active);
            self.active_element = Some(t);
            self.transition_focus(t);
        } else if let Some(prev) = self.focused_element {
            self.blur_with_events(prev);
        }
    }

    /// Pointer up. `explicit_target` is used only when no active session exists.
    pub fn on_pointer_up(&mut self, x: f32, y: f32) {
        let fallback = self.hit_test(x, y);
        self.pointer_up_with_fallback(fallback);
        self.selection_drag = false;
    }

    /// Pointer up with an explicit fallback target (HTML Mode).
    pub fn on_pointer_up_on(&mut self, explicit_target: Option<ElementId>) {
        self.pointer_up_with_fallback(explicit_target);
    }

    fn pointer_up_with_fallback(&mut self, explicit_target: Option<ElementId>) {
        let target = self.active_element.or(explicit_target);
        if let Some(t) = target {
            self.emit_interaction(Event::ActiveEnd { target_id: t });
            // Capture the still-active appearance as the transition start before
            // clearing the active state (ADR-0089).
            self.mark_pseudo_activation_dirty(t, PseudoState::Active);
            self.active_element = None;
        }
    }

    /// Pointer cancel (touch interruption / pointer-capture loss). Coordinate-
    /// independent: clears the whole hover set — emitting `HoverLeave` for each
    /// left element and resetting the stored last-pointer position, identical to
    /// the surface-leave hover-clear — and additionally ends the active press
    /// (`active_element.take()` → `ActiveEnd` + pseudo-activation dirty, mirroring
    /// the pointer-up path). Does not fabricate a `PointerMove`.
    pub fn on_pointer_cancel(&mut self) {
        self.apply_pointer_hover(None);
        self.last_pointer_pos = None;
        self.selection_drag = false;
        if let Some(t) = self.active_element {
            self.emit_interaction(Event::ActiveEnd { target_id: t });
            self.mark_pseudo_activation_dirty(t, PseudoState::Active);
            self.active_element = None;
        }
    }

    /// Pointer move with layout guard and 1 px dedup. `moved` is false when
    /// coalesced; `resolved_cursor` is the cursor resolved from the element under
    /// the pointer (ADR-0088), carried forward unchanged on coalesced moves.
    pub fn on_pointer_move(&mut self, x: f32, y: f32) -> PointerMoveResult {
        if !self.has_layout() {
            return PointerMoveResult {
                moved: false,
                resolved_cursor: self.last_cursor,
            };
        }
        if let Some((lx, ly)) = self.last_pointer_pos {
            if (x - lx).abs() < 1.0 && (y - ly).abs() < 1.0 {
                return PointerMoveResult {
                    moved: false,
                    resolved_cursor: self.last_cursor,
                };
            }
        }
        self.last_pointer_pos = Some((x, y));
        self.push_event(Event::PointerMove { x, y });
        let hit = self.hit_test(x, y);
        self.apply_pointer_hover(hit);
        let resolved_cursor = self.resolve_cursor(hit);
        self.last_cursor = resolved_cursor;
        // Extend the in-flight selection's focus to follow the drag (ADR-0097).
        if self.selection_drag {
            if let Some(point) = self.selection_point_at(x, y) {
                self.update_selection_focus(point);
            }
        }
        PointerMoveResult {
            moved: true,
            resolved_cursor,
        }
    }

    /// Resolve the effective cursor for the element under the pointer, walking
    /// up the ancestor chain (CSS `cursor` inherits). `Default` when nothing in
    /// the chain sets a cursor or the pointer hit nothing.
    fn resolve_cursor(&self, hit: Option<ElementId>) -> CursorValue {
        let mut current = hit;
        while let Some(id) = current {
            let Some(el) = self.elements.get(&id) else {
                break;
            };
            if let Some(cursor) = el.visual.cursor {
                return cursor;
            }
            current = el.parent;
        }
        CursorValue::Default
    }

    /// Target-less pointer move (HTML Mode coordinate stream without hit-test hover).
    pub fn on_pointer_move_coords(&mut self, x: f32, y: f32) -> bool {
        if let Some((lx, ly)) = self.last_pointer_pos {
            if (x - lx).abs() < 1.0 && (y - ly).abs() < 1.0 {
                return false;
            }
        }
        self.last_pointer_pos = Some((x, y));
        self.push_event(Event::PointerMove { x, y });
        true
    }

    /// Pointer left the surface (coordinate-independent). Clears the entire
    /// hover set — emitting `HoverLeave` for each left element and marking
    /// pseudo-activation dirty — and resets the stored last-pointer-position so
    /// a subsequent re-entry is not coalesced away. Does NOT push a phantom
    /// `PointerMove`. Symmetric with the HTML adapter's per-element leave seam.
    pub fn on_pointer_leave(&mut self) {
        self.apply_pointer_hover(None);
        self.last_pointer_pos = None;
    }

    pub fn on_wheel(&mut self, target: ElementId, delta_x: f32, delta_y: f32) {
        self.emit_interaction(Event::Scroll {
            target_id: target,
            delta_x,
            delta_y,
        });
    }

    pub fn on_resize(&mut self, width: f32, height: f32) {
        self.set_viewport(width, height);
        self.push_event(Event::Resize { width, height });
    }

    pub fn on_key_down(&mut self, key: &str, modifiers: u32) {
        let Some(focused) = self.focused_element else {
            return;
        };
        if let Some(edit) = self
            .elements
            .get_mut(&focused)
            .and_then(|el| el.edit.as_mut())
        {
            if edit.apply_key_down(key) {
                if key == "Enter" {
                    self.emit_interaction(Event::TextInput {
                        target_id: focused,
                        text: "\n".to_string(),
                    });
                }
            }
        }
        self.emit_interaction(Event::KeyDown {
            target_id: focused,
            key: key.to_string(),
            modifiers,
        });
    }

    pub fn on_text_input(&mut self, target: ElementId, text: &str) {
        if let Some(edit) = self
            .elements
            .get_mut(&target)
            .and_then(|el| el.edit.as_mut())
        {
            edit.append(text);
        }
        self.emit_interaction(Event::TextInput {
            target_id: target,
            text: text.to_string(),
        });
    }

    pub fn on_composition_start(&mut self, target: ElementId, text: &str) {
        if let Some(edit) = self
            .elements
            .get_mut(&target)
            .and_then(|el| el.edit.as_mut())
        {
            edit.set_preedit(text);
        }
        self.emit_interaction(Event::CompositionStart {
            target_id: target,
            text: text.to_string(),
        });
    }

    pub fn on_composition_update(&mut self, target: ElementId, text: &str) {
        if let Some(edit) = self
            .elements
            .get_mut(&target)
            .and_then(|el| el.edit.as_mut())
        {
            edit.set_preedit(text);
        }
        self.emit_interaction(Event::CompositionUpdate {
            target_id: target,
            text: text.to_string(),
        });
    }

    pub fn on_composition_end(&mut self, target: ElementId, text: &str) {
        if let Some(edit) = self
            .elements
            .get_mut(&target)
            .and_then(|el| el.edit.as_mut())
        {
            edit.finish_composition(text);
        }
        self.emit_interaction(Event::CompositionEnd {
            target_id: target,
            text: text.to_string(),
        });
    }

    pub fn on_hover_enter(&mut self, target: ElementId) {
        if self.hover_enter_element(target) {
            self.emit_interaction(Event::HoverEnter { target_id: target });
        }
    }

    pub fn on_hover_leave(&mut self, target: ElementId) {
        if self.hover_leave_element(target) {
            self.emit_interaction(Event::HoverLeave { target_id: target });
        }
    }

    /// Programmatic focus (mutation batch / accessibility).
    pub fn on_focus(&mut self, id: ElementId) {
        self.transition_focus(id);
    }

    /// Programmatic blur (mutation batch).
    pub fn on_blur(&mut self, id: ElementId) {
        self.blur_with_events(id);
    }

    pub fn active_element(&self) -> Option<ElementId> {
        self.active_element
    }

    /// The single document-wide text selection, if any (ADR-0097).
    pub fn selection(&self) -> Option<&Selection> {
        self.selection.as_ref()
    }

    /// Begin a selection from a pointer-down: collapse to a caret at the hit
    /// point when inside a Selection Region, otherwise clear any selection and
    /// stay out of drag mode (a press outside `selectable` does not start one).
    fn begin_selection_at(&mut self, x: f32, y: f32) {
        match self.selection_point_at(x, y) {
            Some(point) => {
                self.selection_drag = true;
                self.set_selection(Some(Selection::caret(point)));
            }
            None => {
                self.selection_drag = false;
                if self.selection.is_some() {
                    self.set_selection(None);
                }
            }
        }
    }

    /// Resolve a canvas point to a selection endpoint `(IFC root, byte offset)`,
    /// using the IFC's Parley content layout (ADR-0097). `None` when the point is
    /// outside any `selectable` subtree or hits no shaped text.
    fn selection_point_at(&self, x: f32, y: f32) -> Option<SelectionPoint> {
        let hit = self.hit_test(x, y)?;
        if !self.within_selectable(hit) {
            return None;
        }
        let ifc = ifc_root(&self.elements, hit).unwrap_or(hit);
        let tl = self.elements.get(&ifc)?.text_layout.as_ref()?;
        let &(ex, ey, _, _) = self.layout.layout_cache.get(&ifc)?;
        let offset = byte_index_at_point(tl, x - ex, y - ey);
        Some(SelectionPoint::new(ifc, offset))
    }

    /// Whether `id` lies within a `selectable` subtree (nearest ancestor wins).
    fn within_selectable(&self, id: ElementId) -> bool {
        let mut current = Some(id);
        while let Some(eid) = current {
            let Some(el) = self.elements.get(&eid) else {
                break;
            };
            if el.selectable {
                return true;
            }
            current = el.parent;
        }
        false
    }

    fn set_selection(&mut self, next: Option<Selection>) {
        if self.selection == next {
            return;
        }
        if let Some(prev) = self.selection {
            self.mark_selection_dirty(prev);
        }
        self.selection = next;
        if let Some(now) = self.selection {
            self.mark_selection_dirty(now);
        }
    }

    fn update_selection_focus(&mut self, point: SelectionPoint) {
        let Some(sel) = self.selection.as_mut() else {
            return;
        };
        if sel.focus == point {
            return;
        }
        sel.focus = point;
        self.engine
            .mark_visual_dirty(point.element, VisualInvalidationReach::SelfOnly);
    }

    /// Re-lower the elements a selection touches so the highlight follows it.
    fn mark_selection_dirty(&mut self, sel: Selection) {
        self.engine
            .mark_visual_dirty(sel.anchor.element, VisualInvalidationReach::SelfOnly);
        self.engine
            .mark_visual_dirty(sel.focus.element, VisualInvalidationReach::SelfOnly);
    }

    fn emit_interaction(&mut self, event: Event) {
        if let Some(kind) = event_document_kind(&event) {
            self.dispatch_event(kind, event);
        } else {
            self.push_event(event);
        }
    }

    fn transition_focus(&mut self, id: ElementId) {
        if self.focused_element == Some(id) {
            return;
        }
        if let Some(prev) = self.focused_element {
            self.blur_with_events(prev);
        }
        self.element_focus(id);
        self.dispatch_event(
            DocumentEventKind::Focus,
            Event::Focus { target_id: id },
        );
    }

    fn blur_with_events(&mut self, id: ElementId) {
        if self.focused_element != Some(id) {
            return;
        }
        self.element_blur(id);
        self.dispatch_event(
            DocumentEventKind::Blur,
            Event::Blur { target_id: id },
        );
    }

    fn apply_pointer_hover(&mut self, deepest_hit: Option<ElementId>) {
        let (entered, left) = self.update_pointer_hover(deepest_hit);
        for id in left {
            self.emit_interaction(Event::HoverLeave { target_id: id });
        }
        for id in entered {
            self.emit_interaction(Event::HoverEnter { target_id: id });
        }
    }
}
