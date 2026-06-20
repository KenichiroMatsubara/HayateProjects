//! Self-wired canvas pointer input (ADR-0080 / ADR-0082, #211 / #212 / #213).
//!
//! Canvas DOM Pointer Events (`pointerdown` / `pointermove` / `pointerup` /
//! `pointerleave` / `pointercancel`) plus `wheel` are subscribed behind
//! `attach_pointer_input` (wasm32 only), mirroring `attach_resize_observer`: the
//! listener `Closure`s are held alive by a guard and enqueue into an ordered
//! `pending_pointer` buffer drained at the start of `render()`. `pointerleave`
//! and `pointercancel` are coordinate-independent and clear hover via
//! `ElementTree::on_pointer_leave()` (#212) / `on_pointer_cancel()` (#213, which
//! also ends the active press). The `toCanvas` coordinate transform and the 1px
//! move-coalescing (including the leave/cancel-driven anchor reset) are pure and
//! unit-tested on all targets.

#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;
#[cfg(target_arch = "wasm32")]
use std::rc::Rc;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::closure::Closure;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use web_sys::{
    AddEventListenerOptions, Event, HtmlCanvasElement, MouseEvent, PointerEvent, WheelEvent,
};

/// A raw canvas pointer input buffered between frames. Coordinates are already
/// in canvas backing-store space (the `toCanvas` transform is applied when the
/// DOM event is captured, mirroring the former TS `attachPointerInput`).
use hayate_core::PointerKind;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PointerInput {
    /// `pointerdown`, carrying the modifier-key bitfield (`MODIFIER_*` wire
    /// contract) so Shift+click can extend a selection (ADR-0097, #267) and the
    /// [`PointerKind`] (ADR-0082/ADR-0104, #350/#357) so Core retains the device
    /// per interaction and only touch/pen take the drag→scroll path while mouse
    /// keeps selection/drag.
    Down {
        x: f32,
        y: f32,
        modifiers: u32,
        kind: PointerKind,
    },
    /// `pointermove`, carrying the [`PointerKind`] so Core's `last_pointer_kind`
    /// follows hybrid devices mid-session (#357).
    Move { x: f32, y: f32, kind: PointerKind },
    /// `pointerup`, carrying the [`PointerKind`] (#357).
    Up { x: f32, y: f32, kind: PointerKind },
    /// Pointer left the canvas surface (`pointerleave`). Coordinate-independent:
    /// drains to `ElementTree::on_pointer_leave()`, clearing hover.
    Leave,
    /// `pointercancel`: touch interruption / pointer-capture loss. Carries no
    /// coordinates — Core `on_pointer_cancel` is coordinate-independent (it
    /// clears hover and ends the active press regardless of position).
    Cancel,
    Wheel {
        x: f32,
        y: f32,
        delta_x: f32,
        delta_y: f32,
    },
}

/// Transform a viewport-relative client point into Hayate **layout
/// coordinates** (CSS px), the space Core hit-testing and `layout_cache` live
/// in. The layout viewport is set from the canvas' CSS content box
/// (`canvas_resize_metrics`: `viewport = css_size`), so a client point is mapped
/// by subtracting the canvas' CSS origin — no `devicePixelRatio` scaling.
///
/// Scaling into the backing-store buffer (CSS px × dpr) here would feed Core
/// physical-pixel coordinates and miss the hit-test on every HiDPI display
/// (clicks landing at `dpr×` the intended position). Rendering applies the dpr
/// scale separately via `content_scale` at the backend (`backend::mod` doc).
pub fn to_layout_coords(
    client_x: f32,
    client_y: f32,
    rect_left: f32,
    rect_top: f32,
) -> (f32, f32) {
    (client_x - rect_left, client_y - rect_top)
}

/// Coalesce consecutive pointer moves within 1px of the previous applied move,
/// preserving the arrival order of every other input. `seed` is the last move
/// position applied on a previous drain so micro-moves spanning a frame
/// boundary coalesce too. Non-move inputs pass through untouched and do not move
/// the coalescing anchor — matching Core, where `on_pointer_down/up`/wheel leave
/// `last_pointer_pos` unchanged.
pub fn coalesce_pointer_inputs(
    inputs: impl IntoIterator<Item = PointerInput>,
    seed: Option<(f32, f32)>,
) -> Vec<PointerInput> {
    let mut anchor = seed;
    let mut out = Vec::new();
    for input in inputs {
        match input {
            PointerInput::Move { x, y, .. } => {
                if let Some((ax, ay)) = anchor {
                    if (x - ax).abs() < 1.0 && (y - ay).abs() < 1.0 {
                        continue;
                    }
                }
                anchor = Some((x, y));
            }
            // A leave/cancel clears hover and resets Core's last_pointer_pos, so
            // it must also reset the coalescing anchor — a re-entry move at the
            // same coordinate has to pass through to reapply `:hover`.
            PointerInput::Leave | PointerInput::Cancel => anchor = None,
            _ => {}
        }
        out.push(input);
    }
    out
}

/// The coalescing anchor that should seed the next drain, replaying the same
/// move/leave anchor logic over `inputs` starting from `seed`. A move sets the
/// anchor, a leave clears it, and other inputs leave it unchanged — so the 1px
/// dedup carries across frame boundaries without leaking a stale position past a
/// `pointerleave`.
pub fn final_anchor(inputs: &[PointerInput], seed: Option<(f32, f32)>) -> Option<(f32, f32)> {
    let mut anchor = seed;
    for input in inputs {
        match input {
            PointerInput::Move { x, y, .. } => anchor = Some((*x, *y)),
            PointerInput::Leave | PointerInput::Cancel => anchor = None,
            _ => {}
        }
    }
    anchor
}

// ── web-sys wiring (wasm32 only, thin & untested — mirrors attach_resize_observer)

#[cfg(target_arch = "wasm32")]
pub(crate) struct PointerInputGuard {
    canvas: HtmlCanvasElement,
    listeners: Vec<(&'static str, Closure<dyn FnMut(Event)>)>,
}

#[cfg(target_arch = "wasm32")]
impl Drop for PointerInputGuard {
    fn drop(&mut self) {
        for (name, closure) in &self.listeners {
            let _ = self
                .canvas
                .remove_event_listener_with_callback(name, closure.as_ref().unchecked_ref());
        }
    }
}

/// Self-attach `pointerdown` / `pointermove` / `pointerup` + `wheel` listeners on
/// `canvas`, enqueueing each (coordinate-transformed) input into `pending`.
#[cfg(target_arch = "wasm32")]
pub(crate) fn attach_pointer_input(
    canvas: &HtmlCanvasElement,
    pending: Rc<RefCell<Vec<PointerInput>>>,
) -> Result<PointerInputGuard, JsValue> {
    let mut listeners: Vec<(&'static str, Closure<dyn FnMut(Event)>)> = Vec::new();

    // Canvas Mode owns touch scrolling, so suppress the browser's native
    // touch-action (pan/zoom) on the surface (ADR-0082 / ADR-0080: the Rust
    // adapter self-configures this, no host glue).
    let _ = canvas.style().set_property("touch-action", "none");

    {
        // `pointerdown` additionally captures modifier keys so Shift+click can
        // extend the active selection (#267) and the `pointerType` so the drain
        // can route touch/pen into the scroll gesture (#350). Only the primary
        // pointer is tracked — second fingers and pinch are ignored (v1 scope).
        // For touch/pen we also capture the pointer so the gesture keeps
        // receiving moves if the finger leaves the canvas.
        let canvas_for_cb = canvas.clone();
        let pending = pending.clone();
        let closure = Closure::wrap(Box::new(move |event: Event| {
            let Some(pe) = event.dyn_ref::<PointerEvent>() else {
                return;
            };
            if !pe.is_primary() {
                return;
            }
            let kind = PointerKind::from_dom(&pe.pointer_type());
            let (x, y) = pointer_event_to_canvas(&canvas_for_cb, pe.as_ref());
            let modifiers = mouse_modifiers(pe.as_ref());
            if crate::scroll_drag::is_drag_scroll_pointer(kind) {
                let _ = canvas_for_cb.set_pointer_capture(pe.pointer_id());
            }
            pending.borrow_mut().push(PointerInput::Down {
                x,
                y,
                modifiers,
                kind,
            });
        }) as Box<dyn FnMut(Event)>);
        canvas.add_event_listener_with_callback("pointerdown", closure.as_ref().unchecked_ref())?;
        listeners.push(("pointerdown", closure));
    }

    for (name, make) in [
        ("pointermove", make_move as fn(f32, f32, PointerKind) -> PointerInput),
        ("pointerup", make_up),
    ] {
        let canvas_for_cb = canvas.clone();
        let pending = pending.clone();
        let closure = Closure::wrap(Box::new(move |event: Event| {
            let Some(pe) = event.dyn_ref::<PointerEvent>() else {
                return;
            };
            // Track only the primary pointer (ignore extra fingers / pinch).
            if !pe.is_primary() {
                return;
            }
            // Carry the device so Core's `last_pointer_kind` follows the live
            // pointer, not just the press that began the interaction (#357).
            let kind = PointerKind::from_dom(&pe.pointer_type());
            let (x, y) = pointer_event_to_canvas(&canvas_for_cb, pe.as_ref());
            pending.borrow_mut().push(make(x, y, kind));
        }) as Box<dyn FnMut(Event)>);
        canvas.add_event_listener_with_callback(name, closure.as_ref().unchecked_ref())?;
        listeners.push((name, closure));
    }

    {
        // `pointerleave` is coordinate-independent — it clears the whole hover
        // set in Core, so no `toCanvas` transform is needed.
        let pending = pending.clone();
        let closure = Closure::wrap(Box::new(move |_event: Event| {
            pending.borrow_mut().push(PointerInput::Leave);
        }) as Box<dyn FnMut(Event)>);
        canvas.add_event_listener_with_callback("pointerleave", closure.as_ref().unchecked_ref())?;
        listeners.push(("pointerleave", closure));
    }

    {
        // `pointercancel` is coordinate-independent (Core clears hover + active
        // regardless of position), so it enqueues a bare `Cancel`.
        let pending = pending.clone();
        let closure = Closure::wrap(Box::new(move |event: Event| {
            if event.dyn_ref::<PointerEvent>().is_none() {
                return;
            }
            pending.borrow_mut().push(PointerInput::Cancel);
        }) as Box<dyn FnMut(Event)>);
        canvas.add_event_listener_with_callback("pointercancel", closure.as_ref().unchecked_ref())?;
        listeners.push(("pointercancel", closure));
    }

    {
        let canvas_for_cb = canvas.clone();
        let pending = pending.clone();
        let closure = Closure::wrap(Box::new(move |event: Event| {
            let Some(we) = event.dyn_ref::<WheelEvent>() else {
                return;
            };
            // Canvas Mode owns wheel scrolling end-to-end — `apply_wheel_delta`
            // moves the hit `scroll-view` and chains the unconsumed remainder up
            // to the root (ADR-0084). Suppress the browser's native default scroll
            // so a wheel inside the canvas does not *also* scroll the page (or any
            // native scrollable ancestor) on top of the in-canvas scroll — the
            // wheel sibling of `touch-action: none` for touch. Without this, a
            // child scroll-view and the surrounding page scroll together (二重スクロール).
            we.prevent_default();
            let (x, y) = pointer_event_to_canvas(&canvas_for_cb, we.as_ref());
            pending.borrow_mut().push(PointerInput::Wheel {
                x,
                y,
                delta_x: we.delta_x() as f32,
                delta_y: we.delta_y() as f32,
            });
        }) as Box<dyn FnMut(Event)>);
        // Non-passive so `prevent_default` above actually suppresses the native
        // scroll; a passive listener silently ignores it and the page scrolls too.
        let opts = AddEventListenerOptions::new();
        opts.set_passive(false);
        canvas.add_event_listener_with_callback_and_add_event_listener_options(
            "wheel",
            closure.as_ref().unchecked_ref(),
            &opts,
        )?;
        listeners.push(("wheel", closure));
    }

    Ok(PointerInputGuard {
        canvas: canvas.clone(),
        listeners,
    })
}

/// Pack a mouse/pointer event's modifier keys into the `MODIFIER_*` wire
/// bitfield (SHIFT=1, CTRL=2, ALT=4, META=8) shared with `on_key_down`.
#[cfg(target_arch = "wasm32")]
fn mouse_modifiers(event: &MouseEvent) -> u32 {
    let mut mods = 0;
    if event.shift_key() {
        mods |= 1;
    }
    if event.ctrl_key() {
        mods |= 2;
    }
    if event.alt_key() {
        mods |= 4;
    }
    if event.meta_key() {
        mods |= 8;
    }
    mods
}
#[cfg(target_arch = "wasm32")]
fn make_move(x: f32, y: f32, kind: PointerKind) -> PointerInput {
    PointerInput::Move { x, y, kind }
}
#[cfg(target_arch = "wasm32")]
fn make_up(x: f32, y: f32, kind: PointerKind) -> PointerInput {
    PointerInput::Up { x, y, kind }
}

/// Read `clientX/clientY` off a `MouseEvent` (or subclass) and convert to Hayate
/// layout coordinates (CSS px) using the canvas' live CSS bounding rect origin.
#[cfg(target_arch = "wasm32")]
fn pointer_event_to_canvas(canvas: &HtmlCanvasElement, event: &MouseEvent) -> (f32, f32) {
    let rect = canvas.get_bounding_client_rect();
    to_layout_coords(
        event.client_x() as f32,
        event.client_y() as f32,
        rect.left() as f32,
        rect.top() as f32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coalesce_preserves_arrival_order_of_distinct_inputs() {
        let inputs = vec![
            PointerInput::Down { x: 10.0, y: 10.0, modifiers: 0, kind: PointerKind::Mouse },
            PointerInput::Move { x: 20.0, y: 20.0, kind: PointerKind::Mouse },
            PointerInput::Up { x: 20.0, y: 20.0, kind: PointerKind::Mouse },
        ];
        let out = coalesce_pointer_inputs(inputs.clone(), None);
        assert_eq!(out, inputs);
    }

    #[test]
    fn coalesce_drops_consecutive_sub_pixel_moves() {
        let inputs = vec![
            PointerInput::Move { x: 50.0, y: 50.0, kind: PointerKind::Mouse },
            PointerInput::Move { x: 50.4, y: 50.2, kind: PointerKind::Mouse }, // within 1px of (50,50) → dropped
            PointerInput::Move { x: 60.0, y: 50.0, kind: PointerKind::Mouse }, // > 1px → kept
        ];
        let out = coalesce_pointer_inputs(inputs, None);
        assert_eq!(
            out,
            vec![
                PointerInput::Move { x: 50.0, y: 50.0, kind: PointerKind::Mouse },
                PointerInput::Move { x: 60.0, y: 50.0, kind: PointerKind::Mouse },
            ]
        );
    }

    #[test]
    fn coalesce_does_not_collapse_moves_across_a_down() {
        // A press between two near-identical positions must survive: down/up
        // never move the coalescing anchor, but they must keep their order.
        let inputs = vec![
            PointerInput::Move { x: 50.0, y: 50.0, kind: PointerKind::Mouse },
            PointerInput::Down { x: 50.0, y: 50.0, modifiers: 0, kind: PointerKind::Mouse },
            PointerInput::Move { x: 50.2, y: 50.0, kind: PointerKind::Mouse }, // still within 1px of anchor → dropped
        ];
        let out = coalesce_pointer_inputs(inputs, None);
        assert_eq!(
            out,
            vec![
                PointerInput::Move { x: 50.0, y: 50.0, kind: PointerKind::Mouse },
                PointerInput::Down { x: 50.0, y: 50.0, modifiers: 0, kind: PointerKind::Mouse },
            ]
        );
    }

    #[test]
    fn coalesce_resets_anchor_on_cancel_so_re_entry_move_survives() {
        // `pointercancel` clears hover and resets Core's last_pointer_pos (just
        // like leave), so it must also reset the coalescing anchor: a re-entry
        // move at the same coordinate has to pass through to reapply `:hover`.
        let inputs = vec![
            PointerInput::Move { x: 10.0, y: 10.0, kind: PointerKind::Mouse },
            PointerInput::Cancel,
            PointerInput::Move { x: 10.2, y: 10.0, kind: PointerKind::Mouse }, // within 1px of (10,10) but anchor reset → kept
        ];
        let out = coalesce_pointer_inputs(inputs, None);
        assert_eq!(
            out,
            vec![
                PointerInput::Move { x: 10.0, y: 10.0, kind: PointerKind::Mouse },
                PointerInput::Cancel,
                PointerInput::Move { x: 10.2, y: 10.0, kind: PointerKind::Mouse },
            ],
        );
    }

    #[test]
    fn coalesce_uses_seed_to_drop_first_move_across_frame_boundary() {
        // The first move repeats the position applied on the previous drain.
        let inputs = vec![PointerInput::Move { x: 100.0, y: 100.0, kind: PointerKind::Mouse }];
        let out = coalesce_pointer_inputs(inputs, Some((100.0, 100.0)));
        assert!(out.is_empty());
    }

    #[test]
    fn coalesce_resets_anchor_on_leave_so_re_entry_move_survives() {
        // A leave clears the coalescing anchor (Core resets last_pointer_pos),
        // so re-entering at the same coordinate must NOT be dropped — otherwise
        // the re-hover would never reach Core and `:hover` would not reapply.
        let inputs = vec![
            PointerInput::Move { x: 50.0, y: 50.0, kind: PointerKind::Mouse },
            PointerInput::Leave,
            PointerInput::Move { x: 50.0, y: 50.0, kind: PointerKind::Mouse },
        ];
        let out = coalesce_pointer_inputs(inputs.clone(), None);
        assert_eq!(out, inputs);
    }

    #[test]
    fn final_anchor_carries_last_move_and_clears_on_leave() {
        // Most recent move becomes the next-drain anchor; non-move inputs don't.
        let moved = vec![
            PointerInput::Move { x: 10.0, y: 20.0, kind: PointerKind::Mouse },
            PointerInput::Up { x: 10.0, y: 20.0, kind: PointerKind::Mouse },
        ];
        assert_eq!(final_anchor(&moved, None), Some((10.0, 20.0)));

        // A trailing leave clears the anchor so the next frame's re-entry move
        // (even at the same coordinate) is not coalesced across the boundary.
        let left = vec![
            PointerInput::Move { x: 10.0, y: 20.0, kind: PointerKind::Mouse },
            PointerInput::Leave,
        ];
        assert_eq!(final_anchor(&left, None), None);

        // No moves and no leave preserves the incoming seed (position unchanged).
        assert_eq!(final_anchor(&[], Some((5.0, 5.0))), Some((5.0, 5.0)));
    }

    #[test]
    fn to_layout_coords_maps_client_into_css_layout_space() {
        // Client (210,110) on a canvas whose CSS box origin is (10,10) maps to
        // the canvas-local CSS point (200, 100) — the same space as layout and
        // hit-testing.
        let (x, y) = to_layout_coords(210.0, 110.0, 10.0, 10.0);
        assert_eq!((x, y), (200.0, 100.0));
    }

    #[test]
    fn to_layout_coords_does_not_scale_by_device_pixel_ratio() {
        // Regression: layout/hit-test live in CSS px, while the backing store is
        // CSS px × dpr. The pointer transform must stay translation-only —
        // scaling by dpr (the old `canvas_width / rect_width` factor) put every
        // click at dpr× the intended position on HiDPI displays, so hit_test
        // missed and onClick never fired (Canvas mode, both backends, dpr ≠ 1).
        // A client point one CSS px inside a 400-CSS-px-wide box stays at CSS 1.0,
        // never the 2.0 a dpr-2 backing buffer would have produced.
        let (x, _) = to_layout_coords(1.0, 0.0, 0.0, 0.0);
        assert_eq!(x, 1.0);
    }
}
