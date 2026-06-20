use crate::element::edit_state::{Direction, EditIntent, Granularity};
use crate::element::event_spec::{event_document_kind, DocumentEventKind, Event};
use crate::element::id::ElementId;
use crate::element::inline_text::{byte_index_at_point, ifc_root};
use crate::element::pointer::PointerKind;
use crate::element::selection::{
    self, Selection, SelectionPoint, MOD_ALT, MOD_CTRL, MOD_PRIMARY, MOD_SHIFT,
};
use crate::element::style::CursorValue;
use crate::element::tree::ElementTree;
use crate::element::visual_invalidation::VisualInvalidationReach;

/// Map an editing keystroke to an [`EditIntent`] (ADR-0103). Horizontal arrows
/// move the caret (Shift extends; Alt/Ctrl widens a grapheme step to a word);
/// Backspace / Delete remove one char backward / forward. Returns `None` for any
/// other key so callers fall through to the raw `on_key_down` path. This is the
/// OS-independent core bridge; the Platform Adapter owns the full OS keymap.
fn key_edit_intent(key: &str, modifiers: u32) -> Option<EditIntent> {
    // Clipboard / select-all on the primary modifier (Ctrl on Win/Linux, Cmd on
    // macOS). These reach a focused text-input through the same seam as the
    // arrows (ADR-0103 §5③, #361); the document-selection path handles them only
    // when a read-only Selection exists, so without this a focused field never
    // saw Ctrl/Cmd+A/C/X/V.
    if modifiers & MOD_PRIMARY != 0 {
        if let Some(intent) = clipboard_edit_intent(key) {
            return Some(intent);
        }
    }
    // Forward/backward delete, widened from a grapheme to a whole word by Alt
    // (macOS Option) or Ctrl (Win/Linux) — the same "by word" modifiers as the
    // arrows (ADR-0103 §5, #363).
    if let Some(direction) = match key {
        "Backspace" => Some(Direction::Backward),
        "Delete" => Some(Direction::Forward),
        _ => None,
    } {
        let granularity = if modifiers & (MOD_ALT | MOD_CTRL) != 0 {
            Granularity::Word
        } else {
            Granularity::Grapheme
        };
        return Some(EditIntent::Delete {
            granularity,
            direction,
        });
    }
    let direction = match key {
        "ArrowLeft" => Direction::Backward,
        "ArrowRight" => Direction::Forward,
        // Vertical motion (#368): bare ↑/↓. Multi-line fields move between display
        // lines; single-line fields jump to the field ends (resolved downstream).
        "ArrowUp" => Direction::Up,
        "ArrowDown" => Direction::Down,
        _ => return None,
    };
    // Alt/Ctrl widen a *horizontal* step to a word; they have no effect on a
    // vertical motion, which always steps one display line.
    let granularity = if modifiers & (MOD_ALT | MOD_CTRL) != 0
        && matches!(direction, Direction::Backward | Direction::Forward)
    {
        Granularity::Word
    } else {
        Granularity::Grapheme
    };
    Some(if modifiers & MOD_SHIFT != 0 {
        EditIntent::Extend {
            granularity,
            direction,
        }
    } else {
        EditIntent::Move {
            granularity,
            direction,
        }
    })
}

/// The display line the caret at `offset` sits on, plus its content-local x
/// (#368). The line is the one whose block band contains the caret's vertical
/// centre; past the last line it is the last line. Shared by vertical motion
/// (↑/↓) and display-line Home/End, which both key off the caret's current row.
fn caret_display_line(
    layout: &parley::Layout<crate::element::text::TextBrush>,
    offset: usize,
) -> (usize, f32) {
    use parley::{Affinity, Cursor};
    let g = Cursor::from_byte_index(layout, offset, Affinity::Downstream).geometry(layout, 0.0);
    let caret_x = g.x0 as f32;
    let caret_mid_y = ((g.y0 + g.y1) / 2.0) as f32;
    let line_count = layout.len();
    let line = (0..line_count)
        .find(|&i| {
            layout.get(i).is_some_and(|line| {
                let m = line.metrics();
                caret_mid_y >= m.block_min_coord && caret_mid_y < m.block_max_coord
            })
        })
        .unwrap_or_else(|| line_count.saturating_sub(1));
    (line, caret_x)
}

/// Map a primary-modifier letter to its clipboard / select-all [`EditIntent`]
/// (ADR-0103 §5③, #361). The caller has already checked the primary modifier is
/// held, so this only inspects the letter. `None` for any other key.
fn clipboard_edit_intent(key: &str) -> Option<EditIntent> {
    if key.eq_ignore_ascii_case("a") {
        Some(EditIntent::SelectAll)
    } else if key.eq_ignore_ascii_case("c") {
        Some(EditIntent::Copy)
    } else if key.eq_ignore_ascii_case("x") {
        Some(EditIntent::Cut)
    } else if key.eq_ignore_ascii_case("v") {
        Some(EditIntent::Paste)
    } else {
        None
    }
}

/// Output of `on_pointer_move` (ADR-0088). `moved` is false when the move was
/// coalesced by the 1px dedup or skipped because layout is not ready; `cursor`
/// carries the cursor resolved from the element under the pointer so the
/// Platform Adapter can drive the OS/browser cursor without touching styles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PointerMoveResult {
    pub moved: bool,
    pub resolved_cursor: CursorValue,
}

/// Modality of the most recent input event (#335, ADR-0102). Chromium's
/// `:focus-visible` heuristic keys off the last interaction: a keyboard
/// interaction makes the next focus ring-worthy, while a pointer interaction
/// suppresses the ring on widgets that don't need it (e.g. buttons). Tracked in
/// core so both Canvas backends derive the ring identically.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputModality {
    Pointer,
    Keyboard,
}

/// An in-flight Mouse/Pen scrollbar-thumb drag (ADR-0110, #409). Captured on a
/// pointer-down on the thumb and driven by `on_pointer_move`: each move converts
/// the pointer's travel along the axis into a Scroll Offset delta and commits it
/// through the same `apply_wheel_delta` seam the wheel uses (so reaching the axis
/// end chains the remainder to the ancestor ScrollView).
#[derive(Clone, Copy, Debug)]
pub(crate) struct ScrollbarDrag {
    /// The ScrollView whose thumb is being dragged.
    pub scroll_view: ElementId,
    /// Axis the thumb slides along.
    pub axis: crate::element::scene_build::ScrollAxis,
    /// Last pointer coordinate along the drag axis (canvas space).
    pub last_pos: f32,
    /// Offset px per track px — `max_offset / thumb_travel`, captured at grab so
    /// the thumb tracks the pointer 1:1 in track space.
    pub offset_per_px: f32,
}

impl ElementTree {
    /// Pointer down at canvas coordinates (hit-test driven).
    pub fn on_pointer_down(&mut self, x: f32, y: f32) {
        self.on_pointer_down_with(x, y, 0);
    }

    /// Pointer down carrying both keyboard modifiers and the physical
    /// [`PointerKind`] (#357). The Platform Adapter forwards the DOM
    /// `PointerEvent.pointerType` here so Core retains it per interaction
    /// (`last_pointer_kind`); selection/active behaviour is otherwise identical
    /// to [`on_pointer_down_with`](Self::on_pointer_down_with).
    pub fn on_pointer_down_with_kind(
        &mut self,
        x: f32,
        y: f32,
        modifiers: u32,
        kind: PointerKind,
    ) {
        self.last_pointer_kind = kind;
        self.on_pointer_down_with(x, y, modifiers);
    }

    /// Pointer down carrying keyboard modifiers (ADR-0097, #267): Shift extends
    /// the current selection's focus instead of starting a fresh one.
    pub fn on_pointer_down_with(&mut self, x: f32, y: f32, modifiers: u32) {
        // A press on the Mouse/Pen scrollbar (thumb or track) operates it and
        // consumes the gesture — the overlay chrome sits above the content, so a
        // press on it never reaches the content's selection/focus (ADR-0110, #409).
        if self.begin_scrollbar_gesture(x, y) {
            return;
        }
        // A press on a floating-toolbar button runs its action and consumes the
        // gesture, so it neither moves the caret nor clears the selection
        // (ADR-0097, #272).
        if self.try_selection_toolbar_tap(x, y) {
            return;
        }
        // A press on a selection drag handle grabs that endpoint and rides the
        // same active-session capture as a drag-select (ADR-0097, #273), so it
        // adjusts the range without dropping a fresh caret.
        if self.begin_handle_drag(x, y) {
            return;
        }
        let hit = self.hit_test(x, y);
        self.pointer_down_on_target(hit, x, y);
        // A press inside a text-input drives its edit selection (ADR-0097, #271)
        // and takes precedence over the read-only SelectionArea path below.
        if self.begin_edit_selection(hit, x, y, modifiers) {
            return;
        }
        // Selection drag rides on the same active-session capture (ADR-0097):
        // a press inside a Selection Region collapses the selection to a caret,
        // double/triple presses expand to word/paragraph, Shift extends focus.
        self.begin_selection_at(x, y, modifiers);
    }

    /// Long-press at canvas `(x, y)` — the mobile gesture that begins a read-only
    /// word selection and brings up the drag handles + floating toolbar (ADR-0097,
    /// #273). The Platform Adapter reports the raw long-press (its OS gesture
    /// recognizer owns the timing, the same way double-tap timing originates from
    /// the OS); core owns *what* it does. A press clear of any `selectable`
    /// subtree selects nothing. Any text-input edit selection is cleared first
    /// (single active across the document).
    pub fn on_long_press(&mut self, x: f32, y: f32) {
        // Long-press is a touch gesture: the selection it starts is a Touch-modality
        // interaction, so its chrome (handles + toolbar) is raised (ADR-0104, #365).
        self.last_pointer_kind = PointerKind::Touch;
        let Some(point) = self.selection_point_at(x, y) else {
            return;
        };
        self.collapse_edit_selections();
        self.select_bounds_at(point, selection::word_bounds);
        // A fresh gesture: a following tap should drop a caret, not resume the
        // multi-click cycle from this long-press.
        self.selection_drag = false;
        self.last_click_pos = None;
        self.click_count = 0;
    }

    /// Pointer down on an explicit target (HTML Mode).
    pub fn on_pointer_down_on(&mut self, target: ElementId, x: f32, y: f32) {
        self.pointer_down_on_target(Some(target), x, y);
    }

    fn pointer_down_on_target(&mut self, target: Option<ElementId>, x: f32, y: f32) {
        self.last_input_modality = InputModality::Pointer;
        if let Some(t) = target {
            self.emit_interaction(Event::Click {
                target_id: t,
                x,
                y,
            });
            self.emit_interaction(Event::ActiveStart { target_id: t });
            // Setting the active state captures the transition's pre-switch
            // visual and marks `:active` invalidation in the same operation
            // (ADR-0100), so the not-yet-active appearance seeds the transition
            // (ADR-0089).
            self.set_active_element(Some(t));
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
        self.edit_drag = None;
        self.scrollbar_drag = None;
    }

    /// Pointer up carrying the physical [`PointerKind`] (#357), retained per
    /// interaction. Release behaviour is identical to
    /// [`on_pointer_up`](Self::on_pointer_up).
    pub fn on_pointer_up_with_kind(&mut self, x: f32, y: f32, kind: PointerKind) {
        self.last_pointer_kind = kind;
        self.on_pointer_up(x, y);
    }

    /// Pointer up with an explicit fallback target (HTML Mode).
    pub fn on_pointer_up_on(&mut self, explicit_target: Option<ElementId>) {
        self.pointer_up_with_fallback(explicit_target);
    }

    fn pointer_up_with_fallback(&mut self, explicit_target: Option<ElementId>) {
        let target = self.active_element.or(explicit_target);
        if let Some(t) = target {
            self.emit_interaction(Event::ActiveEnd { target_id: t });
        }
        // Clearing the active state captures the still-active appearance as the
        // transition start and marks `:active` invalidation in the same
        // operation (ADR-0100, ADR-0089).
        self.set_active_element(None);
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
        self.edit_drag = None;
        self.scrollbar_drag = None;
        if let Some(t) = self.active_element {
            self.emit_interaction(Event::ActiveEnd { target_id: t });
        }
        // Ending the press clears the active state and marks its `:active`
        // invalidation atomically (ADR-0100), mirroring the pointer-up path.
        self.set_active_element(None);
    }

    /// Pointer move carrying the physical [`PointerKind`] (#357), retained per
    /// interaction so the emitted `PointerMove` wire event and `last_pointer_kind`
    /// reflect the live device (hybrid devices switch mid-session). Hover/cursor
    /// behaviour is identical to [`on_pointer_move`](Self::on_pointer_move).
    pub fn on_pointer_move_with_kind(
        &mut self,
        x: f32,
        y: f32,
        kind: PointerKind,
    ) -> PointerMoveResult {
        self.last_pointer_kind = kind;
        self.on_pointer_move(x, y)
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
        self.push_event(Event::PointerMove {
            x,
            y,
            pointer_kind: self.last_pointer_kind,
        });
        let hit = self.hit_test(x, y);
        self.apply_pointer_hover(hit);
        let resolved_cursor = self.resolve_cursor(hit);
        self.last_cursor = resolved_cursor;
        // Drive the in-flight drag. A scrollbar-thumb drag (ADR-0110, #409) wins:
        // it was grabbed before any selection could begin, so the two never
        // coexist. Otherwise extend the in-flight selection's focus (ADR-0097).
        if let Some(drag) = self.scrollbar_drag {
            self.drag_scrollbar(drag, x, y);
        } else if let Some(input) = self.edit_drag {
            self.extend_edit_drag(input, x, y);
        } else if self.selection_drag {
            if let Some(point) = self.selection_point_at(x, y) {
                self.update_selection_focus(point);
            }
        }
        PointerMoveResult {
            moved: true,
            resolved_cursor,
        }
    }

    /// Resolve the effective cursor for the element under the pointer in the
    /// order "explicit `cursor` → element-kind UA default → `Default`"
    /// (ADR-0105), mirroring the browser's UA stylesheet. An explicit `cursor`
    /// anywhere up the ancestor chain always wins (CSS `cursor` inherits); only
    /// when none is set does the kind default apply — `text-input` and any
    /// `selectable` text resolve to `text` (I-beam), `button` to `pointer`.
    /// `Default` when nothing in the chain contributes or the pointer hit nothing.
    fn resolve_cursor(&self, hit: Option<ElementId>) -> CursorValue {
        // Pass 1: an explicit `cursor` on the hit element or any ancestor wins.
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
        // Pass 2: no explicit cursor — fall back to the element-kind UA default,
        // walking up so a kind/selectable region default still reaches the text
        // or child elements painted inside it.
        let mut current = hit;
        while let Some(id) = current {
            let Some(el) = self.elements.get(&id) else {
                break;
            };
            let kind_default = el.kind.default_cursor();
            if kind_default != CursorValue::Default {
                return kind_default;
            }
            if el.selectable {
                return CursorValue::Text;
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
        self.push_event(Event::PointerMove {
            x,
            y,
            pointer_kind: self.last_pointer_kind,
        });
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
        // A keyboard interaction makes the next focus ring-worthy under Chromium's
        // `:focus-visible` heuristic (#335). Recorded before the early returns so a
        // key press that doesn't reach a focused element still flips the modality.
        self.last_input_modality = InputModality::Keyboard;
        // Selection keyboard gestures (#267) act on the document-wide selection
        // and are independent of element focus, so they run first and consume the
        // key when they apply (e.g. Ctrl/Cmd+A, Shift+Arrow over a SelectionArea).
        if self.handle_selection_key(key, modifiers) {
            return;
        }
        let Some(focused) = self.focused_element else {
            return;
        };
        // Editing keys inside a focused text-input are interpreted as an
        // EditIntent and applied through the single editing seam (ADR-0103):
        // a bare arrow moves the caret (collapsing any selection to its edge),
        // Shift extends the selection, Alt/Ctrl widens the step to a word, and
        // Backspace/Delete remove one char. Consumed when it applies (never
        // while an IME composition is active, so a delete key can't break it).
        if let Some(intent) = key_edit_intent(key, modifiers) {
            if self.apply_edit_intent(focused, intent) {
                return;
            }
        }
        // Enter inserts a newline at the caret only in a multi-line field (#362);
        // a single-line field leaves the text alone so the trailing KeyDown below
        // is the app's submit signal. `apply_key_down` handles Enter alone.
        let multiline = self
            .elements
            .get(&focused)
            .map(|el| el.multiline)
            .unwrap_or(false);
        if key == "Enter" && multiline {
            if let Some(edit) = self
                .elements
                .get_mut(&focused)
                .and_then(|el| el.edit.as_mut())
            {
                if edit.apply_key_down(key) {
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
            // Inserts at the caret, replacing any selected range (ADR-0097, #271).
            edit.insert(text);
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
        self.on_composition_update_formatted(target, text, Vec::new());
    }

    /// IME preedit update carrying the EditContext `textformatupdate` clause
    /// format ranges (ADR-0102), so Canvas Mode can draw the per-clause
    /// conversion underlines. `clauses` offsets are relative to `text`.
    pub fn on_composition_update_formatted(
        &mut self,
        target: ElementId,
        text: &str,
        clauses: Vec<crate::element::edit_state::CompositionClause>,
    ) {
        if let Some(edit) = self
            .elements
            .get_mut(&target)
            .and_then(|el| el.edit.as_mut())
        {
            edit.set_preedit_with_clauses(text, clauses);
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

    /// The active selection's endpoints normalized to document order:
    /// `(start, end)` where `start` precedes `end` in the tree's pre-order walk
    /// (ADR-0097, #269). A same-block selection normalizes by byte offset; a
    /// cross-block one by the blocks' document order. `None` with no selection.
    pub fn selection_ordered(&self) -> Option<(SelectionPoint, SelectionPoint)> {
        let sel = self.selection?;
        if sel.anchor.element == sel.focus.element {
            let lo = sel.anchor.offset.min(sel.focus.offset);
            let hi = sel.anchor.offset.max(sel.focus.offset);
            let el = sel.anchor.element;
            return Some((SelectionPoint::new(el, lo), SelectionPoint::new(el, hi)));
        }
        match self.document_order(sel.anchor.element, sel.focus.element) {
            std::cmp::Ordering::Greater => Some((sel.focus, sel.anchor)),
            _ => Some((sel.anchor, sel.focus)),
        }
    }

    /// Programmatically set the document-global selection to `anchor`..`focus`
    /// (ADR-0097 growth point — selection without a pointer/keyboard gesture).
    /// Applies only when both endpoints share one Selection Region (their
    /// nearest `selectable` ancestor matches and exists), so a programmatic call
    /// honors the same `selectable` boundary as a drag and never leaks across
    /// one. Returns whether it was applied. Routed through the shared selection
    /// path, so it re-lowers the highlight and emits a `selection-change`
    /// notification exactly like a gesture would.
    pub fn set_selection_range(&mut self, anchor: SelectionPoint, focus: SelectionPoint) -> bool {
        let region = self.selection_region_of(anchor.element);
        if region.is_none() || region != self.selection_region_of(focus.element) {
            return false;
        }
        self.set_selection(Some(Selection { anchor, focus }));
        true
    }

    /// Programmatically clear the document-global selection (ADR-0097 growth
    /// point). A no-op when nothing is selected. Routed through the shared
    /// selection path, so it re-lowers the dropped highlight and emits a
    /// `selection-change` notification.
    pub fn clear_selection(&mut self) {
        self.set_selection(None);
    }

    /// The byte range of IFC block `block` covered by the active selection,
    /// normalized to document order (ADR-0097, #269). For a same-block selection
    /// this is the in-block range. For a cross-block one: the first block runs
    /// from its start offset to end-of-text, the last block from 0 to its end
    /// offset, and any block strictly between is covered whole. `None` when the
    /// selection does not touch `block`, or `block` belongs to a different
    /// Selection Region (the selection never leaks across a `selectable`
    /// boundary).
    pub(crate) fn selection_range_in_block(&self, block: ElementId) -> Option<(usize, usize)> {
        // A `user-select: none` block (or one under a `none` subtree) carries no
        // selection: it is skipped identically by the highlight and the copied
        // text, which both read the covered range through this one seam (ADR-0108).
        if self.user_select_excludes(block) {
            return None;
        }
        let (start, end) = self.selection_ordered()?;
        if start.element == end.element {
            return (start.element == block).then_some((start.offset, end.offset));
        }
        if self.selection_region_of(block) != self.selection_region_of(start.element) {
            return None;
        }
        let block_len = || self.ifc_text(block).map(|t| t.len()).unwrap_or(0);
        if block == start.element {
            Some((start.offset, block_len()))
        } else if block == end.element {
            Some((0, end.offset))
        } else if self.document_order(start.element, block) == std::cmp::Ordering::Less
            && self.document_order(block, end.element) == std::cmp::Ordering::Less
        {
            Some((0, block_len()))
        } else {
            None
        }
    }

    /// Whether `id` is excluded from the document selection by CSS
    /// `user-select: none` on itself or any ancestor (ADR-0108: `none` excludes
    /// the whole subtree). The single gate the covered-range, highlight, and
    /// copied-text paths share — all routed through `selection_range_in_block` —
    /// so a `none` element drops out of every one of them at once.
    fn user_select_excludes(&self, id: ElementId) -> bool {
        let mut current = Some(id);
        while let Some(eid) = current {
            let Some(el) = self.elements.get(&eid) else {
                break;
            };
            if el.user_select == crate::element::style::UserSelectValue::None {
                return true;
            }
            current = el.parent;
        }
        false
    }

    /// Compare two elements by document order (their position in a pre-order DFS
    /// of the tree). An ancestor precedes its descendants; earlier siblings
    /// precede later ones. Implemented by comparing the elements' root-paths
    /// (the sequence of child indices from the root) lexicographically.
    fn document_order(&self, a: ElementId, b: ElementId) -> std::cmp::Ordering {
        self.root_path(a).cmp(&self.root_path(b))
    }

    /// The path from the document root to `id` as a sequence of child indices
    /// (root-relative). Comparing two such paths lexicographically yields
    /// pre-order: a prefix (ancestor) sorts before a longer path (descendant).
    fn root_path(&self, id: ElementId) -> Vec<usize> {
        let mut path = Vec::new();
        let mut cur = id;
        while let Some(el) = self.elements.get(&cur) {
            let Some(parent) = el.parent else { break };
            let idx = self
                .elements
                .get(&parent)
                .and_then(|p| p.children.iter().position(|&c| c == cur))
                .unwrap_or(0);
            path.push(idx);
            cur = parent;
        }
        path.reverse();
        path
    }

    /// The text currently under the selection, as a single string (ADR-0097,
    /// #268 / ADR-0108 decision 5). Walks every IFC-root block the selection
    /// covers in document order — the same `selection_range_in_block` seam the
    /// highlight lowers through, so copy and paint agree (no duplicate ordering
    /// logic) — slices each block's covered byte range out of its shaped text
    /// (which already joins inline children in document order, so styled runs
    /// come back joined), and inserts a single `\n` at each block-box boundary
    /// (ADR-0108: same shape as a browser copy; per-block-kind newline counts
    /// are a growth point). `None` when there is no selection or nothing
    /// non-empty is covered (a collapsed caret has nothing to copy).
    pub fn selected_text(&self) -> Option<String> {
        let sel = self.selection?;
        let mut parts: Vec<String> = Vec::new();
        for block in self.blocks_spanned_by(sel) {
            let Some((start, end)) = self.selection_range_in_block(block) else {
                continue;
            };
            if start == end {
                continue;
            }
            let Some(text) = self.ifc_text(block) else {
                continue;
            };
            parts.push(text[start..end].to_string());
        }
        if parts.is_empty() {
            return None;
        }
        Some(parts.join("\n"))
    }

    /// Whether selection chrome (the drag handles and floating toolbar) should be
    /// drawn for the current interaction — true only under Touch modality
    /// (ADR-0104 decision 2, #365). Mouse/Pen get the thin caret and drag-select
    /// alone, matching desktop-browser behaviour, while Touch raises the mobile
    /// gesture surface. Read per interaction from [`last_pointer_kind`] so hybrid
    /// devices (touch laptop, mouse-equipped tablet) follow the live device. The
    /// highlight tint is deliberately *not* gated here — it paints under every
    /// modality (ADR-0097, tint=Chromium).
    ///
    /// [`last_pointer_kind`]: Self::last_pointer_kind
    fn touch_chrome_visible(&self) -> bool {
        self.last_pointer_kind == PointerKind::Touch
    }

    /// The floating selection toolbar for the active selection, or `None` when
    /// no selection is active (ADR-0097, #272). The toolbar is core-drawn chrome:
    /// a read-only SelectionArea selection offers Copy / Select All; an editable
    /// text-input selection adds Cut / Paste. It floats over the selection's
    /// canvas-space bounding box, themed by the current chrome style. Drawn only
    /// under Touch modality; Mouse/Pen selections get the thin caret alone
    /// (ADR-0104 decision 2, #365).
    pub fn selection_toolbar(&self) -> Option<crate::element::selection_chrome::SelectionToolbar> {
        if !self.touch_chrome_visible() {
            return None;
        }
        let (actions, bounds) = self.active_selection_chrome()?;
        crate::element::selection_chrome::layout(
            self.selection_chrome_style,
            &actions,
            bounds,
            self.viewport,
        )
    }

    /// The pair of Material drag handles flanking the active read-only selection,
    /// or `None` when no non-collapsed SelectionArea selection is active
    /// (ADR-0097, #273). The handles are core-drawn chrome: a teardrop knob hangs
    /// just below each end of the range, themed by the current chrome style. They
    /// are the mobile gesture surface — dragging one adjusts that endpoint — so
    /// they are raised only under Touch modality; Mouse/Pen selections show none
    /// (ADR-0104 decision 2, #365).
    /// Text-input edit-selection handles are a growth point (the toolbar already
    /// covers both paths; handles ride the read-only SelectionArea for now).
    pub fn selection_handles(
        &self,
    ) -> Option<crate::element::selection_chrome::SelectionHandles> {
        if !self.touch_chrome_visible() {
            return None;
        }
        let sel = self.selection?;
        if sel.is_caret() {
            return None;
        }
        let (start, end) = self.selection_ordered()?;
        let start_caret = self.selection_caret_canvas(start)?;
        let end_caret = self.selection_caret_canvas(end)?;
        Some(crate::element::selection_chrome::layout_handles(
            self.selection_chrome_style,
            start_caret,
            end_caret,
        ))
    }

    /// Canvas-space caret edge `(x, baseline_bottom_y)` for a read-only selection
    /// endpoint, from its IFC's Parley layout offset by the block's cached
    /// origin. `None` when the endpoint's block has no shaped geometry yet.
    fn selection_caret_canvas(&self, point: SelectionPoint) -> Option<(f32, f32)> {
        use parley::{Affinity, Cursor};
        let tl = self.elements.get(&point.element)?.text_layout.as_ref()?;
        let (ex, ey, _, _) = self.layout.geometry(point.element)?;
        let g = Cursor::from_byte_index(&tl.layout, point.offset, Affinity::Downstream)
            .geometry(&tl.layout, 0.0);
        Some((ex + g.x0 as f32, ey + g.y1 as f32))
    }

    /// Begin a handle drag when the press `(x, y)` grabs one of the selection's
    /// drag handles (ADR-0097, #273). The grabbed end becomes the selection's
    /// `focus` and the opposite end the fixed `anchor`, so the existing
    /// drag-select move path (`selection_drag` → `update_selection_focus`)
    /// adjusts exactly that endpoint and clamps it to the Selection Region.
    /// Returns whether a handle was grabbed (and the gesture consumed).
    fn begin_handle_drag(&mut self, x: f32, y: f32) -> bool {
        use crate::element::selection_chrome::SelectionHandleEnd;
        let Some(grabbed) = self.selection_handles().and_then(|h| h.handle_at(x, y)) else {
            return false;
        };
        let Some((start, end)) = self.selection_ordered() else {
            return false;
        };
        // Drag the grabbed end; pin the opposite one as the anchor.
        let (anchor, focus) = match grabbed {
            SelectionHandleEnd::Start => (end, start),
            SelectionHandleEnd::End => (start, end),
        };
        self.set_selection(Some(Selection { anchor, focus }));
        self.selection_drag = true;
        true
    }

    /// Begin a Mouse/Pen scrollbar operation from a pointer-down at `(x, y)`
    /// (ADR-0110, SCR-04, #409). A press on a thumb starts a drag; a press on the
    /// track margin pages the Scroll Offset one [`SCROLLBAR_PAGE_STEP`] toward the
    /// click. Both commit through the wheel's `apply_wheel_delta` seam, so they
    /// converge on the same Scroll Offset (ADR-0046) and chain to ancestors
    /// identically (ADR-0084). Touch shows a non-interactive transient indicator
    /// (a later slice), so this is a no-op under Touch modality. Returns whether
    /// the press hit the scrollbar (and the gesture was consumed).
    ///
    /// [`SCROLLBAR_PAGE_STEP`]: crate::element::scene_build::SCROLLBAR_PAGE_STEP
    fn begin_scrollbar_gesture(&mut self, x: f32, y: f32) -> bool {
        use crate::element::scene_build::{ScrollAxis, SCROLLBAR_PAGE_STEP};
        if self.last_pointer_kind == PointerKind::Touch {
            return false;
        }
        let Some((sv, axis, on_thumb)) = self.scrollbar_hit_at(x, y) else {
            return false;
        };
        if on_thumb {
            // Map a future track-pixel drag to an offset delta so the thumb
            // tracks the pointer 1:1 in track space.
            let offset_per_px = if axis.thumb_travel > 0.0 {
                axis.max_offset / axis.thumb_travel
            } else {
                0.0
            };
            let last_pos = match axis.axis {
                ScrollAxis::Vertical => y,
                ScrollAxis::Horizontal => x,
            };
            self.scrollbar_drag = Some(ScrollbarDrag {
                scroll_view: sv,
                axis: axis.axis,
                last_pos,
                offset_per_px,
            });
        } else {
            // Track margin: page toward the click — past the thumb's far end pages
            // forward, before its near end pages back, one named step either way.
            let (tx, ty, tw, th) = axis.thumb;
            let step = match axis.axis {
                ScrollAxis::Vertical => {
                    let forward = y > ty + th;
                    (
                        0.0,
                        if forward {
                            SCROLLBAR_PAGE_STEP
                        } else {
                            -SCROLLBAR_PAGE_STEP
                        },
                    )
                }
                ScrollAxis::Horizontal => {
                    let forward = x > tx + tw;
                    (
                        if forward {
                            SCROLLBAR_PAGE_STEP
                        } else {
                            -SCROLLBAR_PAGE_STEP
                        },
                        0.0,
                    )
                }
            };
            self.apply_wheel_delta(sv, step.0, step.1);
        }
        true
    }

    /// The scrollbar axis under `(x, y)`, if any — `(scroll_view, geometry,
    /// on_thumb)` where `on_thumb` is true for a thumb hit and false for a track
    /// hit (ADR-0110, #409). Reads the shared `scrollbar_axes` geometry so the hit
    /// region is exactly what the overlay paints. The deepest (most nested)
    /// matching ScrollView wins, since its thumb is painted last (on top); a thumb
    /// hit beats a track hit at equal depth.
    fn scrollbar_hit_at(
        &self,
        x: f32,
        y: f32,
    ) -> Option<(
        ElementId,
        crate::element::scene_build::ScrollbarAxisGeometry,
        bool,
    )> {
        let in_rect = |(rx, ry, rw, rh): (f32, f32, f32, f32)| {
            x >= rx && x <= rx + rw && y >= ry && y <= ry + rh
        };
        let mut best: Option<(usize, ElementId, _, bool)> = None;
        for (&id, el) in &self.elements {
            if el.kind != crate::element::kind::ElementKind::ScrollView {
                continue;
            }
            for axis in crate::element::scene_build::scrollbar_axes(self, id) {
                let on_thumb = in_rect(axis.thumb);
                if !on_thumb && !in_rect(axis.track) {
                    continue;
                }
                let depth = self.root_path(id).len();
                let better = match &best {
                    None => true,
                    Some((bd, _, _, bt)) => depth > *bd || (depth == *bd && on_thumb && !*bt),
                };
                if better {
                    best = Some((depth, id, axis, on_thumb));
                }
            }
        }
        best.map(|(_, id, axis, on_thumb)| (id, axis, on_thumb))
    }

    /// Advance an in-flight thumb drag to pointer `(x, y)` (ADR-0110, #409). The
    /// pointer's travel along the drag axis since the last move becomes a Scroll
    /// Offset delta and is committed through `apply_wheel_delta` — the wheel's
    /// seam — so the offset moves continuously and, once this ScrollView hits its
    /// axis end, the unconsumed remainder chains to the ancestor ScrollView.
    fn drag_scrollbar(&mut self, mut drag: ScrollbarDrag, x: f32, y: f32) {
        use crate::element::scene_build::ScrollAxis;
        let pos = match drag.axis {
            ScrollAxis::Vertical => y,
            ScrollAxis::Horizontal => x,
        };
        let pointer_delta = pos - drag.last_pos;
        if pointer_delta.abs() < 1e-6 {
            return;
        }
        let offset_delta = pointer_delta * drag.offset_per_px;
        match drag.axis {
            ScrollAxis::Vertical => {
                self.apply_wheel_delta(drag.scroll_view, 0.0, offset_delta);
            }
            ScrollAxis::Horizontal => {
                self.apply_wheel_delta(drag.scroll_view, offset_delta, 0.0);
            }
        }
        drag.last_pos = pos;
        self.scrollbar_drag = Some(drag);
    }

    /// Run a floating-toolbar button under `(x, y)`, if the press lands on one.
    /// Returns whether the gesture was consumed by the toolbar (ADR-0097, #272).
    fn try_selection_toolbar_tap(&mut self, x: f32, y: f32) -> bool {
        let Some(action) = self.selection_toolbar().and_then(|tb| tb.action_at(x, y)) else {
            return false;
        };
        self.dispatch_toolbar_action(action);
        true
    }

    /// Run a toolbar action against the active selection (ADR-0097, #272).
    fn dispatch_toolbar_action(&mut self, action: crate::element::selection_chrome::ToolbarAction) {
        use crate::element::selection_chrome::ToolbarAction;
        match action {
            ToolbarAction::Copy => self.copy_active_selection(),
            ToolbarAction::Cut => self.cut_active_selection(),
            ToolbarAction::Paste => self.paste_active_selection(),
            ToolbarAction::SelectAll => self.select_all_active_selection(),
        }
    }

    /// The text under whichever selection is active — the read-only SelectionArea
    /// selection, else the editable text-input's edit selection (single active,
    /// ADR-0097, #271). `None` when nothing non-empty is selected.
    fn active_selection_text(&self) -> Option<String> {
        if let Some(text) = self.selected_text() {
            return Some(text);
        }
        let input = self.edit_selection_owner()?;
        let edit = self.elements.get(&input)?.edit.as_ref()?;
        let (start, end) = edit.selection_range()?;
        Some(edit.text_content[start..end].to_string())
    }

    /// Copy the active selection through the Platform Adapter clipboard. A no-op
    /// when nothing is selected or no clipboard is installed (ADR-0097, #268).
    fn copy_active_selection(&mut self) {
        if let Some(text) = self.active_selection_text() {
            if let Some(clipboard) = self.clipboard.as_ref() {
                clipboard.write_text(&text);
            }
        }
    }

    /// Cut the editable selection: copy it to the clipboard, then delete the
    /// range from the text-input. Read-only SelectionArea selections cannot be
    /// cut, so this is a no-op there (ADR-0097, #272).
    fn cut_active_selection(&mut self) {
        let Some(input) = self.edit_selection_owner() else {
            return;
        };
        let Some(removed) = self
            .elements
            .get_mut(&input)
            .and_then(|el| el.edit.as_mut())
            .and_then(|edit| edit.cut())
        else {
            return;
        };
        if let Some(clipboard) = self.clipboard.as_ref() {
            clipboard.write_text(&removed);
        }
        self.engine
            .mark_visual_dirty(input, VisualInvalidationReach::SelfOnly);
    }

    /// Paste clipboard text into the editable selection, replacing it. Pulls the
    /// text through the Platform Adapter's synchronous clipboard read; an adapter
    /// whose read is async feeds the result back via `element_paste` instead, so
    /// this is a no-op there. Read-only selections cannot paste (ADR-0097, #272).
    fn paste_active_selection(&mut self) {
        let Some(input) = self.edit_selection_owner() else {
            return;
        };
        let Some(text) = self.clipboard.as_ref().and_then(|c| c.read_text()) else {
            return;
        };
        self.element_paste(input, &text);
    }

    /// Paste clipboard text into a specific text-input (the keyboard Ctrl/Cmd+V
    /// path, ADR-0103 §5③). Unlike `paste_active_selection`, this targets the
    /// focused field directly, so it also pastes at a collapsed caret (an empty
    /// field with no selection). The text is pulled through the Platform Adapter's
    /// synchronous clipboard read; an adapter whose read is async (Canvas Mode)
    /// returns `None` here and feeds the text back via `element_paste` instead.
    fn paste_into_text_input(&mut self, target: ElementId) {
        let Some(text) = self.clipboard.as_ref().and_then(|c| c.read_text()) else {
            return;
        };
        self.element_paste(target, &text);
    }

    /// Select All against the active selection: the whole focus IFC for a
    /// read-only SelectionArea selection, or the text-input's whole content for
    /// an editable one (ADR-0097, #272).
    fn select_all_active_selection(&mut self) {
        if let Some(sel) = self.selection {
            self.select_all_in(sel.focus.element);
            return;
        }
        let Some(input) = self.edit_selection_owner() else {
            return;
        };
        if let Some(edit) = self.elements.get_mut(&input).and_then(|el| el.edit.as_mut()) {
            let len = edit.text_content.len();
            edit.set_selection(0, len);
        }
        self.engine
            .mark_visual_dirty(input, VisualInvalidationReach::SelfOnly);
    }

    /// Resolve the active selection into its toolbar action set and canvas-space
    /// bounding box. A non-collapsed read-only SelectionArea selection wins;
    /// otherwise the editable text-input that holds a non-collapsed edit
    /// selection (the two never coexist — single active, ADR-0097, #271).
    fn active_selection_chrome(
        &self,
    ) -> Option<(
        Vec<crate::element::selection_chrome::ToolbarAction>,
        crate::element::selection_chrome::ToolbarRect,
    )> {
        use crate::element::selection_chrome::ToolbarAction;
        if self.selection.is_some_and(|s| !s.is_caret()) {
            let bounds = self.read_only_selection_bounds()?;
            return Some((vec![ToolbarAction::Copy, ToolbarAction::SelectAll], bounds));
        }
        let input = self.edit_selection_owner()?;
        let bounds = self.edit_selection_bounds(input)?;
        Some((
            vec![
                ToolbarAction::Cut,
                ToolbarAction::Copy,
                ToolbarAction::Paste,
                ToolbarAction::SelectAll,
            ],
            bounds,
        ))
    }

    /// The text-input holding the active (= focused) non-collapsed edit
    /// selection, if any. Selection chrome is focus-linked (ADR-0104): an
    /// unfocused field that still remembers a Mouse/Pen range shows no chrome, so
    /// at most one selection — the focused one — is ever active across the
    /// document (single-active, ADR-0097).
    fn edit_selection_owner(&self) -> Option<ElementId> {
        let id = self.focused_element?;
        self.elements
            .get(&id)?
            .edit
            .as_ref()
            .filter(|e| e.selection_range().is_some())
            .map(|_| id)
    }

    /// Canvas-space bounding box of the read-only selection's highlight, unioned
    /// across the blocks it touches (anchor and focus IFCs). `None` when the
    /// selection has no shaped geometry yet.
    fn read_only_selection_bounds(&self) -> Option<crate::element::selection_chrome::ToolbarRect> {
        let (start, end) = self.selection_ordered()?;
        let mut acc: Option<(f32, f32, f32, f32)> = None;
        for block in [start.element, end.element] {
            let Some((s, e)) = self.selection_range_in_block(block) else {
                continue;
            };
            let Some(el) = self.elements.get(&block) else {
                continue;
            };
            let Some(tl) = el.text_layout.as_ref() else {
                continue;
            };
            let Some((ex, ey, _, _)) = self.layout.geometry(block) else {
                continue;
            };
            for (rx, ry, rw, rh) in
                crate::element::scene_build::selection_highlight_rects(&tl.layout, s, e)
            {
                accumulate_rect(&mut acc, ex + rx, ey + ry, rw, rh);
            }
        }
        acc.map(rect_from_bounds)
    }

    /// Canvas-space bounding box of a text-input's edit-selection highlight.
    fn edit_selection_bounds(
        &self,
        input: ElementId,
    ) -> Option<crate::element::selection_chrome::ToolbarRect> {
        let el = self.elements.get(&input)?;
        let (s, e) = el.edit.as_ref()?.selection_range()?;
        let cl = el.content_layout.as_ref()?;
        let (ex, ey, _, _) = self.layout.geometry(input)?;
        let taffy_node = self.layout.projection.node_id(input)?;
        let box_layout = self.layout.projection.taffy.layout(taffy_node).ok()?;
        let content_x = ex + box_layout.border.left + box_layout.padding.left;
        let content_y = ey + box_layout.border.top + box_layout.padding.top;
        let mut acc: Option<(f32, f32, f32, f32)> = None;
        for (rx, ry, rw, rh) in
            crate::element::scene_build::selection_highlight_rects(&cl.layout, s, e)
        {
            accumulate_rect(&mut acc, content_x + rx, content_y + ry, rw, rh);
        }
        acc.map(rect_from_bounds)
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

        // A SelectionArea selection and any text-input edit selection are
        // mutually exclusive (single active, ADR-0097, #271).
        self.collapse_edit_selections();

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

    /// Begin (or extend) a text-input's edit selection from a pointer-down
    /// inside it (ADR-0097, #271). A plain press drops a caret and arms a drag;
    /// Shift+click extends the focus from the existing anchor. Consecutive presses
    /// near the same spot cycle the gesture like the read-only SelectionArea path
    /// (`begin_selection_at`): 1 = caret, 2 = word, 3 = line (#366). Word/line
    /// expansion is a Mouse/Pen gesture — under Touch a press stays a caret, so it
    /// never competes with the long-press word selection (ADR-0104). Either way
    /// the read-only SelectionArea selection is cleared (single active). Returns
    /// whether the press landed inside an editable text-input.
    fn begin_edit_selection(
        &mut self,
        hit: Option<ElementId>,
        x: f32,
        y: f32,
        modifiers: u32,
    ) -> bool {
        let Some(input) = hit else { return false };
        let is_text_input = self
            .elements
            .get(&input)
            .is_some_and(|el| {
                el.kind == crate::element::kind::ElementKind::TextInput && el.edit.is_some()
            });
        if !is_text_input {
            return false;
        }
        let Some(offset) = self.edit_offset_at(input, x, y) else {
            return false;
        };

        // Shift+click extends the focus from the existing anchor (range
        // extension), not a fresh caret, and does not advance the multi-click
        // cycle — mirroring the read-only `begin_selection_at` Shift path.
        if modifiers & MOD_SHIFT != 0 {
            if let Some(edit) = self.elements.get_mut(&input).and_then(|el| el.edit.as_mut()) {
                edit.move_focus(offset);
            }
            self.edit_drag = Some(input);
            self.last_click_pos = Some((x, y));
            self.click_count = 1;
            self.finish_edit_selection(input);
            return true;
        }

        // The press count near the same spot cycles caret → word → line. Word and
        // line are Mouse/Pen expansions; under Touch every press stays a caret.
        let phase = self.advance_click_phase(x, y);
        let bounds: Option<fn(&str, usize) -> (usize, usize)> =
            match (phase, self.last_pointer_kind == PointerKind::Touch) {
                (2, false) => Some(selection::word_bounds),
                (3, false) => Some(selection::line_bounds),
                _ => None,
            };
        if let Some(edit) = self.elements.get_mut(&input).and_then(|el| el.edit.as_mut()) {
            match bounds {
                Some(bounds) => {
                    let (start, end) = bounds(&edit.text_content, offset);
                    edit.set_selection(start, end);
                }
                None => edit.set_selection(offset, offset),
            }
        }
        // A word/line selection is not drag-extendable (parity with the read-only
        // path); a caret arms a drag so the user can extend it.
        self.edit_drag = bounds.is_none().then_some(input);
        self.finish_edit_selection(input);
        true
    }

    /// Shared tail of [`begin_edit_selection`](Self::begin_edit_selection): a
    /// text-input selection and a SelectionArea selection never coexist (single
    /// active, ADR-0097), so clear the document selection and repaint the field.
    fn finish_edit_selection(&mut self, input: ElementId) {
        self.set_selection(None);
        self.engine
            .mark_visual_dirty(input, VisualInvalidationReach::SelfOnly);
    }

    /// Collapse every text-input's edit selection to a caret. Called when a
    /// read-only SelectionArea selection starts, so at most one selection is
    /// active across the document (ADR-0097, #271). Only fields that actually
    /// held a range are repainted.
    fn collapse_edit_selections(&mut self) {
        let collapsed: Vec<ElementId> = self
            .elements
            .iter_mut()
            .filter_map(|(&id, el)| {
                let edit = el.edit.as_mut()?;
                if edit.is_caret() {
                    return None;
                }
                edit.collapse();
                Some(id)
            })
            .collect();
        for id in collapsed {
            self.engine
                .mark_visual_dirty(id, VisualInvalidationReach::SelfOnly);
        }
    }

    /// Extend the in-flight text-input drag: move the edit selection's focus to
    /// the byte offset under the pointer, keeping the anchor (ADR-0097, #271).
    fn extend_edit_drag(&mut self, input: ElementId, x: f32, y: f32) {
        let Some(offset) = self.edit_offset_at(input, x, y) else {
            return;
        };
        if let Some(edit) = self.elements.get_mut(&input).and_then(|el| el.edit.as_mut()) {
            if edit.cursor_byte_index == offset {
                return;
            }
            edit.move_focus(offset);
        }
        self.engine
            .mark_visual_dirty(input, VisualInvalidationReach::SelfOnly);
    }

    /// Resolve a canvas point to a byte offset within a text-input's content,
    /// using its Parley `content_layout` in the element's content box (inset by
    /// border + padding, matching `element_character_bounds`). `None` when the
    /// field has not been laid out yet.
    fn edit_offset_at(&self, input: ElementId, x: f32, y: f32) -> Option<usize> {
        let el = self.elements.get(&input)?;
        let cl = el.content_layout.as_ref()?;
        let (ex, ey, _, _) = self.layout.geometry(input)?;
        let taffy_node = self.layout.projection.node_id(input)?;
        let box_layout = self.layout.projection.taffy.layout(taffy_node).ok()?;
        let content_x = ex + box_layout.border.left + box_layout.padding.left;
        let content_y = ey + box_layout.border.top + box_layout.padding.top;
        Some(byte_index_at_point(cl, x - content_x, y - content_y))
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
            self.copy_active_selection();
            return true;
        }
        if modifiers & MOD_SHIFT == 0 {
            return false;
        }
        let Some(text) = self.ifc_text(sel.focus.element) else {
            return false;
        };
        let by_word = modifiers & (MOD_ALT | MOD_CTRL) != 0;
        let Some(next) = selection::arrow_step(&text, key, sel.focus.offset, by_word) else {
            return false;
        };
        self.set_selection(Some(Selection {
            anchor: sel.anchor,
            focus: SelectionPoint::new(sel.focus.element, next),
        }));
        true
    }

    /// Apply one [`EditIntent`] to `target` through the single editing seam
    /// (ADR-0103) and report whether it was consumed. This is the OS-independent
    /// entry point the Platform Adapter drives after mapping an OS keystroke to an
    /// intent; `core` never inspects which key produced it.
    ///
    /// Consumed only for an editable text-input with no active IME composition —
    /// an in-progress preedit is left untouched so caret keys never break a
    /// composition. Altering a text-input selection clears any read-only
    /// SelectionArea selection (single-active rule, ADR-0097).
    pub fn apply_edit_intent(&mut self, target: ElementId, intent: EditIntent) -> bool {
        let Some(el) = self.elements.get(&target) else {
            return false;
        };
        if el.kind != crate::element::kind::ElementKind::TextInput {
            return false;
        }
        let Some(edit) = el.edit.as_ref() else {
            return false;
        };
        if edit.preedit.is_some() {
            return false;
        }
        // Clipboard members of the vocabulary cross the Platform Adapter boundary
        // (ADR-0097): the system clipboard lives on this seam, not on EditState,
        // so they are resolved here by reusing the toolbar clipboard actions
        // (which already act on the focused text-input's edit selection). The
        // pure-state members (Move / Extend / Delete / SelectAll) go straight to
        // the EditState seam.
        // Vertical motion (↑/↓) and display-line Home/End in a *multi-line* field
        // need Parley line geometry, which lives here on the tree seam, not on the
        // pure `EditState` (ADR-0103, #368). Resolve those first; a single-line
        // field (or one not yet laid out) falls through to `EditState::apply`,
        // where ↑/↓ jump to the field ends and Home/End to the field boundary
        // (Chromium `<input>`).
        if self.element_is_multiline(target) {
            let geometric = match intent {
                EditIntent::Move { direction, .. } | EditIntent::Extend { direction, .. }
                    if matches!(direction, Direction::Up | Direction::Down) =>
                {
                    self.apply_vertical_motion(
                        target,
                        direction,
                        matches!(intent, EditIntent::Extend { .. }),
                    )
                }
                EditIntent::Move { granularity: Granularity::LineBoundary, direction }
                | EditIntent::Extend { granularity: Granularity::LineBoundary, direction } => self
                    .apply_display_line_boundary(
                        target,
                        direction,
                        matches!(intent, EditIntent::Extend { .. }),
                    ),
                _ => false,
            };
            if geometric {
                self.set_selection(None);
                return true;
            }
        }
        match intent {
            EditIntent::Copy => self.copy_active_selection(),
            EditIntent::Cut => self.cut_active_selection(),
            EditIntent::Paste => self.paste_into_text_input(target),
            _ => {
                if let Some(edit) = self
                    .elements
                    .get_mut(&target)
                    .and_then(|el| el.edit.as_mut())
                {
                    edit.apply(intent);
                }
                self.engine
                    .mark_visual_dirty(target, VisualInvalidationReach::SelfOnly);
            }
        }
        self.set_selection(None);
        true
    }

    /// Whether `id` is a multi-line text-input (`<textarea>` semantics), so ↑/↓
    /// move between display lines and Home/End snap to display-line ends.
    fn element_is_multiline(&self, id: ElementId) -> bool {
        self.elements.get(&id).map(|el| el.multiline).unwrap_or(false)
    }

    /// Move the caret up or down one display line in a multi-line field, keeping
    /// the sticky goal column (ADR-0103, #368). `extend` keeps the anchor (Shift).
    /// Returns whether it applied — `false` when the field has no shaped layout
    /// yet, so the caller falls back to the pure single-line semantics.
    fn apply_vertical_motion(
        &mut self,
        target: ElementId,
        direction: Direction,
        extend: bool,
    ) -> bool {
        let delta = match direction {
            Direction::Up => -1,
            Direction::Down => 1,
            _ => return false,
        };
        let Some((offset, goal_x)) = self.vertical_caret_target(target, delta) else {
            return false;
        };
        if let Some(edit) = self.elements.get_mut(&target).and_then(|el| el.edit.as_mut()) {
            if extend {
                edit.move_focus(offset);
            } else {
                edit.set_selection(offset, offset);
            }
            // The goal column survives the move so a run of ↑/↓ through short
            // lines returns to the original column.
            edit.desired_x = Some(goal_x);
        }
        self.engine
            .mark_visual_dirty(target, VisualInvalidationReach::SelfOnly);
        true
    }

    /// Byte offset the caret lands on after moving `delta` display lines, paired
    /// with the goal column it aimed for (content-local x). Resolved from the
    /// field's Parley `content_layout`. `None` when there is no shaped layout.
    fn vertical_caret_target(&self, target: ElementId, delta: isize) -> Option<(usize, f32)> {
        use parley::Cursor;
        let el = self.elements.get(&target)?;
        let edit = el.edit.as_ref()?;
        let cl = el.content_layout.as_ref()?;
        let layout = &cl.layout;
        let line_count = layout.len();
        if line_count == 0 {
            return None;
        }
        let (current_line, caret_x) = caret_display_line(layout, edit.cursor_byte_index);
        // Aim for the stored goal column, or the caret's current x on first move.
        let goal_x = edit.desired_x.unwrap_or(caret_x);
        let target_line = current_line as isize + delta;
        if target_line < 0 {
            // Above the first line → the field start (Chromium).
            return Some((0, goal_x));
        }
        if target_line as usize >= line_count {
            // Below the last line → the field end.
            return Some((edit.text_content.len(), goal_x));
        }
        let line = layout.get(target_line as usize)?;
        let m = line.metrics();
        // A y inside the target line, near its baseline (mirrors Parley's own
        // line stepping), hit-tested at the goal column.
        let y = m.block_max_coord - m.ascent * 0.5;
        let dest = Cursor::from_point(layout, goal_x, y);
        Some((dest.index(), goal_x))
    }

    /// Move the caret to the start/end of its current *display* line in a
    /// multi-line field (Home/End over soft-wrapped rows, ADR-0103, #368).
    /// `extend` keeps the anchor (Shift+Home/End). Returns whether it applied.
    fn apply_display_line_boundary(
        &mut self,
        target: ElementId,
        direction: Direction,
        extend: bool,
    ) -> bool {
        let Some(offset) = self.display_line_boundary_target(target, direction) else {
            return false;
        };
        if let Some(edit) = self.elements.get_mut(&target).and_then(|el| el.edit.as_mut()) {
            // Home/End is a horizontal motion: it drops the sticky goal column.
            edit.desired_x = None;
            if extend {
                edit.move_focus(offset);
            } else {
                edit.set_selection(offset, offset);
            }
        }
        self.engine
            .mark_visual_dirty(target, VisualInvalidationReach::SelfOnly);
        true
    }

    /// Byte offset of the start (Backward) or end (Forward) of the caret's
    /// current display line, from the field's Parley `content_layout`. `None`
    /// when there is no shaped layout.
    fn display_line_boundary_target(
        &self,
        target: ElementId,
        direction: Direction,
    ) -> Option<usize> {
        let el = self.elements.get(&target)?;
        let edit = el.edit.as_ref()?;
        let cl = el.content_layout.as_ref()?;
        let layout = &cl.layout;
        if layout.len() == 0 {
            return None;
        }
        let (current_line, _) = caret_display_line(layout, edit.cursor_byte_index);
        let line = layout.get(current_line)?;
        let range = line.text_range();
        match direction {
            Direction::Backward => Some(range.start),
            // Exclude a soft-wrap's boundary whitespace/newline so End lands at the
            // last visible glyph of the row, matching Parley's `line_end`.
            Direction::Forward => {
                let mut end = range.end;
                let text = &edit.text_content;
                while end > range.start {
                    match text[..end].chars().next_back() {
                        Some(c) if c == '\n' || c == '\r' => end -= c.len_utf8(),
                        _ => break,
                    }
                }
                Some(end)
            }
            // Home/End only carry a horizontal direction.
            Direction::Up | Direction::Down => None,
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
        let (ex, ey, _, _) = self.layout.geometry(ifc)?;
        let offset = byte_index_at_point(tl, x - ex, y - ey);
        Some(SelectionPoint::new(ifc, offset))
    }

    /// Whether `id` lies within a `selectable` subtree (nearest ancestor wins).
    fn within_selectable(&self, id: ElementId) -> bool {
        self.selection_region_of(id).is_some()
    }

    /// The nearest Selection Region root ancestor of `id` (inclusive): an element
    /// that confines selection to its subtree. Two markers establish one — the
    /// legacy `selectable` flag (ADR-0097) and `user-select: contains`, the
    /// CSS-authored containment boundary (ADR-0108 decision 3). `None` when `id`
    /// is under neither. The nearest such ancestor wins, so a nested boundary (a
    /// `contains` box inside an outer region, or `contains` within `contains`)
    /// shadows its ancestors: two points share a region only when their nearest
    /// root matches, which is what keeps a selection from leaking across it.
    fn selection_region_of(&self, id: ElementId) -> Option<ElementId> {
        let mut current = Some(id);
        while let Some(eid) = current {
            let el = self.elements.get(&eid)?;
            if el.selectable || el.user_select == crate::element::style::UserSelectValue::Contains {
                return Some(eid);
            }
            current = el.parent;
        }
        None
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
        // Every real change to the document-global Selection — set, moved, or
        // cleared, whether from a gesture or the programmatic API — notifies the
        // host once (ADR-0097 growth point). The equality guard above means a
        // redundant set emits nothing. Payload-less by design: the host polls
        // `selection()` for the new state, like the DOM `selectionchange` event.
        self.emit_interaction(Event::SelectionChange);
    }

    /// Move the drag focus to `point`, keeping the anchor (ADR-0097). The focus
    /// is clamped to the anchor's Selection Region: a drag that wanders into a
    /// different `selectable` region (a nested one, or none) leaves the focus
    /// where it was, so the selection never leaks past a `selectable` boundary.
    /// Routed through `set_selection` so every block the range gained or lost
    /// re-lowers its highlight.
    fn update_selection_focus(&mut self, point: SelectionPoint) {
        let Some(sel) = self.selection else {
            return;
        };
        if sel.focus == point {
            return;
        }
        if self.selection_region_of(point.element) != self.selection_region_of(sel.anchor.element) {
            return;
        }
        self.set_selection(Some(Selection {
            anchor: sel.anchor,
            focus: point,
        }));
    }

    /// Re-lower every block the selection covers so the highlight follows it —
    /// the two endpoint blocks plus any block document-ordered between them
    /// (#269), so a cross-block range repaints intermediate paragraphs too.
    fn mark_selection_dirty(&mut self, sel: Selection) {
        for block in self.blocks_spanned_by(sel) {
            self.engine
                .mark_visual_dirty(block, VisualInvalidationReach::SelfOnly);
        }
    }

    /// The IFC blocks a selection covers, in document order: just the one block
    /// for a same-block selection, otherwise every IFC root in the anchor's
    /// Selection Region from the earlier endpoint's block through the later one.
    fn blocks_spanned_by(&self, sel: Selection) -> Vec<ElementId> {
        if sel.anchor.element == sel.focus.element {
            return vec![sel.anchor.element];
        }
        let region = self.selection_region_of(sel.anchor.element);
        let roots: Vec<ElementId> = self
            .preorder_ifc_roots()
            .filter(|&b| self.selection_region_of(b) == region)
            .collect();
        let ai = roots.iter().position(|&b| b == sel.anchor.element);
        let fi = roots.iter().position(|&b| b == sel.focus.element);
        match (ai, fi) {
            (Some(a), Some(f)) => roots[a.min(f)..=a.max(f)].to_vec(),
            _ => vec![sel.anchor.element, sel.focus.element],
        }
    }

    /// IFC-root blocks in document order (pre-order DFS from the document root).
    fn preorder_ifc_roots(&self) -> impl Iterator<Item = ElementId> + '_ {
        let mut out = Vec::new();
        if let Some(root) = self.root {
            let mut stack = vec![root];
            while let Some(id) = stack.pop() {
                if crate::element::inline_text::is_ifc_root(&self.elements, id) {
                    out.push(id);
                }
                if let Some(el) = self.elements.get(&id) {
                    for &child in el.children.iter().rev() {
                        stack.push(child);
                    }
                }
            }
        }
        out.into_iter()
    }

    fn emit_interaction(&mut self, event: Event) {
        if let Some(kind) = event_document_kind(&event) {
            self.dispatch_event(kind, event);
        } else {
            self.push_event(event);
        }
    }

    pub(crate) fn transition_focus(&mut self, id: ElementId) {
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
        // Modality-dependent blur lifecycle (ADR-0104, #364). Touch dismisses the
        // selection — collapse the edit range to a caret (Android: tapping outside
        // the field clears the selection and its chrome). Mouse/Pen keep the range
        // in EditState, hidden by the focus-linked highlight, so a refocus restores
        // it (Chromium form-control parity).
        if self.last_pointer_kind == PointerKind::Touch {
            self.collapse_edit_selection_of(id);
        }
        self.dispatch_event(
            DocumentEventKind::Blur,
            Event::Blur { target_id: id },
        );
    }

    /// Collapse a single text-input's edit selection to a caret, repainting it
    /// only when it actually held a range. The blur-time counterpart of the
    /// document-wide [`collapse_edit_selections`](Self::collapse_edit_selections).
    fn collapse_edit_selection_of(&mut self, id: ElementId) {
        let collapsed = self
            .elements
            .get_mut(&id)
            .and_then(|el| el.edit.as_mut())
            .is_some_and(|edit| {
                if edit.is_caret() {
                    false
                } else {
                    edit.collapse();
                    true
                }
            });
        if collapsed {
            self.engine
                .mark_visual_dirty(id, VisualInvalidationReach::SelfOnly);
        }
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

/// Grow `acc` (min-x, min-y, max-x, max-y) to include the rect `(x, y, w, h)`.
fn accumulate_rect(acc: &mut Option<(f32, f32, f32, f32)>, x: f32, y: f32, w: f32, h: f32) {
    let (x1, y1) = (x + w, y + h);
    *acc = Some(match *acc {
        None => (x, y, x1, y1),
        Some((ax0, ay0, ax1, ay1)) => (ax0.min(x), ay0.min(y), ax1.max(x1), ay1.max(y1)),
    });
}

/// Convert accumulated (min-x, min-y, max-x, max-y) bounds into a positioned rect.
fn rect_from_bounds(
    (x0, y0, x1, y1): (f32, f32, f32, f32),
) -> crate::element::selection_chrome::ToolbarRect {
    crate::element::selection_chrome::ToolbarRect {
        x: x0,
        y: y0,
        width: x1 - x0,
        height: y1 - y0,
    }
}
