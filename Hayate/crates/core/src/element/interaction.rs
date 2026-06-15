use crate::element::event_spec::{event_document_kind, DocumentEventKind, Event};
use crate::element::id::ElementId;
use crate::element::inline_text::{byte_index_at_point, ifc_root};
use crate::element::pseudo_state::PseudoState;
use crate::element::selection::{
    self, Selection, SelectionPoint, MOD_ALT, MOD_CTRL, MOD_PRIMARY, MOD_SHIFT,
};
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
        self.on_pointer_down_with(x, y, 0);
    }

    /// Pointer down carrying keyboard modifiers (ADR-0097, #267): Shift extends
    /// the current selection's focus instead of starting a fresh one.
    pub fn on_pointer_down_with(&mut self, x: f32, y: f32, modifiers: u32) {
        let hit = self.hit_test(x, y);
        self.pointer_down_on_target(hit, x, y);
        // Selection drag rides on the same active-session capture (ADR-0097):
        // a press inside a Selection Region collapses the selection to a caret,
        // double/triple presses expand to word/paragraph, Shift extends focus.
        self.begin_selection_at(x, y, modifiers);
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
        // Selection keyboard gestures (#267) act on the document-wide selection
        // and are independent of element focus, so they run first and consume the
        // key when they apply (e.g. Ctrl/Cmd+A, Shift+Arrow over a SelectionArea).
        if self.handle_selection_key(key, modifiers) {
            return;
        }
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

    /// The text currently under the selection, as a single string (ADR-0097,
    /// #268). The selected byte range is sliced out of the focus IFC's shaped
    /// text, which already concatenates the IFC's inline children in document
    /// order — so a range that spans several styled runs comes back joined.
    /// `None` when there is no selection or it is collapsed to a caret (nothing
    /// to copy). Cross-IFC selection is a growth point (single-IFC for now).
    pub fn selected_text(&self) -> Option<String> {
        let sel = self.selection?;
        let ifc = sel.anchor.element;
        let (start, end) = sel.range_within(ifc)?;
        if start == end {
            return None;
        }
        let text = self.ifc_text(ifc)?;
        Some(text[start..end].to_string())
    }

    /// Begin a selection from a pointer-down inside a Selection Region:
    ///
    /// - Shift+click keeps the existing anchor and moves the focus to the hit
    ///   point (range extension), when an anchor lives in the same IFC.
    /// - Otherwise the press count near the same spot cycles the gesture:
    ///   1 = caret (drag-extendable), 2 = word, 3 = paragraph.
    ///
    /// A press outside any `selectable` subtree clears the selection and does not
    /// start a drag.
    fn begin_selection_at(&mut self, x: f32, y: f32, modifiers: u32) {
        let Some(point) = self.selection_point_at(x, y) else {
            self.selection_drag = false;
            self.click_count = 0;
            self.last_click_pos = None;
            if self.selection.is_some() {
                self.set_selection(None);
            }
            return;
        };

        if modifiers & MOD_SHIFT != 0 && self.extend_focus_to(point) {
            // Shift+click adjusts focus; stay in drag so the user can keep
            // dragging, but do not advance the multi-click cycle.
            self.selection_drag = true;
            self.last_click_pos = Some((x, y));
            self.click_count = 1;
            return;
        }

        match self.advance_click_phase(x, y) {
            1 => {
                self.selection_drag = true;
                self.set_selection(Some(Selection::caret(point)));
            }
            2 => {
                self.selection_drag = false;
                self.select_bounds_at(point, selection::word_bounds);
            }
            _ => {
                self.selection_drag = false;
                self.select_bounds_at(point, selection::line_bounds);
            }
        }
    }

    /// Increment the consecutive-press counter when the pointer-down lands near
    /// the previous one, else restart it, and return the 1-based gesture phase
    /// cycling caret → word → paragraph (1, 2, 3, 1, …).
    fn advance_click_phase(&mut self, x: f32, y: f32) -> u32 {
        const MULTI_CLICK_TOLERANCE: f32 = 4.0;
        let near = self.last_click_pos.is_some_and(|(lx, ly)| {
            (x - lx).abs() <= MULTI_CLICK_TOLERANCE && (y - ly).abs() <= MULTI_CLICK_TOLERANCE
        });
        self.click_count = if near { self.click_count + 1 } else { 1 };
        self.last_click_pos = Some((x, y));
        (self.click_count - 1) % 3 + 1
    }

    /// Replace the selection with the byte range that `bounds` computes around
    /// `point` within its IFC's shaped text. Falls back to a caret when the IFC
    /// has no shaped text.
    fn select_bounds_at(&mut self, point: SelectionPoint, bounds: fn(&str, usize) -> (usize, usize)) {
        let Some(text) = self.ifc_text(point.element) else {
            self.set_selection(Some(Selection::caret(point)));
            return;
        };
        let (start, end) = bounds(&text, point.offset);
        self.set_selection(Some(Selection {
            anchor: SelectionPoint::new(point.element, start),
            focus: SelectionPoint::new(point.element, end),
        }));
    }

    /// Move the current selection's focus to `point`, keeping the anchor, when an
    /// active selection's anchor is in the same IFC. Returns whether it applied.
    fn extend_focus_to(&mut self, point: SelectionPoint) -> bool {
        let Some(sel) = self.selection else {
            return false;
        };
        if sel.anchor.element != point.element {
            return false;
        }
        self.set_selection(Some(Selection {
            anchor: sel.anchor,
            focus: point,
        }));
        true
    }

    /// Apply a keyboard selection gesture to the active selection (#267) and
    /// report whether the key was consumed:
    ///
    /// - Ctrl/Cmd+A selects the whole Selection Region (the focus IFC).
    /// - Shift+Arrow moves the focus by one character, or by a word when Alt
    ///   (macOS) or Ctrl (Win/Linux) is also held; the anchor stays fixed, so
    ///   repeated presses extend or contract the range.
    fn handle_selection_key(&mut self, key: &str, modifiers: u32) -> bool {
        let Some(sel) = self.selection else {
            return false;
        };
        if modifiers & MOD_PRIMARY != 0 && key.eq_ignore_ascii_case("a") {
            return self.select_all_in(sel.focus.element);
        }
        if modifiers & MOD_PRIMARY != 0 && key.eq_ignore_ascii_case("c") {
            self.copy_selection_to_clipboard();
            return true;
        }
        if modifiers & MOD_SHIFT == 0 {
            return false;
        }
        let Some(text) = self.ifc_text(sel.focus.element) else {
            return false;
        };
        let by_word = modifiers & (MOD_ALT | MOD_CTRL) != 0;
        let offset = sel.focus.offset;
        let next = match (key, by_word) {
            ("ArrowRight", false) => selection::next_grapheme(&text, offset),
            ("ArrowLeft", false) => selection::prev_grapheme(&text, offset),
            ("ArrowRight", true) => selection::next_word(&text, offset),
            ("ArrowLeft", true) => selection::prev_word(&text, offset),
            _ => return false,
        };
        self.set_selection(Some(Selection {
            anchor: sel.anchor,
            focus: SelectionPoint::new(sel.focus.element, next),
        }));
        true
    }

    /// Copy the selected text to the Platform Adapter's clipboard (Cmd/Ctrl+C,
    /// ADR-0097). A no-op when nothing non-empty is selected or no clipboard is
    /// installed; core never touches the concrete clipboard, only the trait.
    fn copy_selection_to_clipboard(&mut self) {
        let Some(text) = self.selected_text() else {
            return;
        };
        if let Some(clipboard) = self.clipboard.as_ref() {
            clipboard.write_text(&text);
        }
    }

    /// Select the entire shaped text of `ifc` (Ctrl/Cmd+A). Returns whether a
    /// range was set (false when the element carries no shaped text).
    fn select_all_in(&mut self, ifc: ElementId) -> bool {
        let Some(text) = self.ifc_text(ifc) else {
            return false;
        };
        self.set_selection(Some(Selection {
            anchor: SelectionPoint::new(ifc, 0),
            focus: SelectionPoint::new(ifc, text.len()),
        }));
        true
    }

    /// The concatenated shaped text of an IFC root, for byte-boundary gestures.
    fn ifc_text(&self, ifc: ElementId) -> Option<std::sync::Arc<str>> {
        self.elements
            .get(&ifc)?
            .text_layout
            .as_ref()
            .map(|tl| tl.text.clone())
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
