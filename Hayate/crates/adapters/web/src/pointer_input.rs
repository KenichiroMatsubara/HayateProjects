//! Self-wired canvas pointer input (ADR-0080 / ADR-0082, #211).
//!
//! Canvas DOM Pointer Events (`pointerdown` / `pointermove` / `pointerup`) plus
//! `wheel` are subscribed behind `attach_pointer_input` (wasm32 only), mirroring
//! `attach_resize_observer`: the listener `Closure`s are held alive by a guard
//! and enqueue into an ordered `pending_pointer` buffer drained at the start of
//! `render()`. The `toCanvas` coordinate transform and the 1px move-coalescing
//! are pure and unit-tested on all targets.

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
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PointerInput {
    Down { x: f32, y: f32 },
    Move { x: f32, y: f32 },
    Up { x: f32, y: f32 },
    Wheel {
        x: f32,
        y: f32,
        delta_x: f32,
        delta_y: f32,
    },
}

/// Transform a viewport-relative client point into canvas backing-store
/// coordinates, mirroring the former `toCanvas` in `init.ts`. A zero-sized rect
/// falls back to unit scale (avoids a divide-by-zero before first layout).
pub fn to_canvas_coords(
    client_x: f32,
    client_y: f32,
    rect_left: f32,
    rect_top: f32,
    rect_width: f32,
    rect_height: f32,
    canvas_width: f32,
    canvas_height: f32,
) -> (f32, f32) {
    let sx = if rect_width == 0.0 {
        1.0
    } else {
        canvas_width / rect_width
    };
    let sy = if rect_height == 0.0 {
        1.0
    } else {
        canvas_height / rect_height
    };
    ((client_x - rect_left) * sx, (client_y - rect_top) * sy)
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
        if let PointerInput::Move { x, y } = input {
            if let Some((ax, ay)) = anchor {
                if (x - ax).abs() < 1.0 && (y - ay).abs() < 1.0 {
                    continue;
                }
            }
            anchor = Some((x, y));
        }
        out.push(input);
    }
    out
}

/// The most recent move position in `inputs`, used to seed the next drain so
/// coalescing carries across frame boundaries.
pub fn last_move_position(inputs: &[PointerInput]) -> Option<(f32, f32)> {
    inputs.iter().rev().find_map(|input| match input {
        PointerInput::Move { x, y } => Some((*x, *y)),
        _ => None,
    })
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

    for (name, make) in [
        ("pointerdown", make_pointer as fn(f32, f32) -> PointerInput),
        ("pointermove", make_move),
        ("pointerup", make_up),
    ] {
        let canvas_for_cb = canvas.clone();
        let pending = pending.clone();
        let closure = Closure::wrap(Box::new(move |event: Event| {
            let Some(pe) = event.dyn_ref::<PointerEvent>() else {
                return;
            };
            let (x, y) = pointer_event_to_canvas(&canvas_for_cb, pe.as_ref());
            pending.borrow_mut().push(make(x, y));
        }) as Box<dyn FnMut(Event)>);
        canvas.add_event_listener_with_callback(name, closure.as_ref().unchecked_ref())?;
        listeners.push((name, closure));
    }

    {
        let canvas_for_cb = canvas.clone();
        let pending = pending.clone();
        let closure = Closure::wrap(Box::new(move |event: Event| {
            let Some(we) = event.dyn_ref::<WheelEvent>() else {
                return;
            };
            let (x, y) = pointer_event_to_canvas(&canvas_for_cb, we.as_ref());
            pending.borrow_mut().push(PointerInput::Wheel {
                x,
                y,
                delta_x: we.delta_x() as f32,
                delta_y: we.delta_y() as f32,
            });
        }) as Box<dyn FnMut(Event)>);
        let opts = AddEventListenerOptions::new();
        opts.set_passive(true);
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

#[cfg(target_arch = "wasm32")]
fn make_pointer(x: f32, y: f32) -> PointerInput {
    PointerInput::Down { x, y }
}
#[cfg(target_arch = "wasm32")]
fn make_move(x: f32, y: f32) -> PointerInput {
    PointerInput::Move { x, y }
}
#[cfg(target_arch = "wasm32")]
fn make_up(x: f32, y: f32) -> PointerInput {
    PointerInput::Up { x, y }
}

/// Read `clientX/clientY` off a `MouseEvent` (or subclass) and convert to canvas
/// coordinates using the canvas' live bounding rect + backing-store size.
#[cfg(target_arch = "wasm32")]
fn pointer_event_to_canvas(canvas: &HtmlCanvasElement, event: &MouseEvent) -> (f32, f32) {
    let rect = canvas.get_bounding_client_rect();
    to_canvas_coords(
        event.client_x() as f32,
        event.client_y() as f32,
        rect.left() as f32,
        rect.top() as f32,
        rect.width() as f32,
        rect.height() as f32,
        canvas.width() as f32,
        canvas.height() as f32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coalesce_preserves_arrival_order_of_distinct_inputs() {
        let inputs = vec![
            PointerInput::Down { x: 10.0, y: 10.0 },
            PointerInput::Move { x: 20.0, y: 20.0 },
            PointerInput::Up { x: 20.0, y: 20.0 },
        ];
        let out = coalesce_pointer_inputs(inputs.clone(), None);
        assert_eq!(out, inputs);
    }

    #[test]
    fn coalesce_drops_consecutive_sub_pixel_moves() {
        let inputs = vec![
            PointerInput::Move { x: 50.0, y: 50.0 },
            PointerInput::Move { x: 50.4, y: 50.2 }, // within 1px of (50,50) → dropped
            PointerInput::Move { x: 60.0, y: 50.0 }, // > 1px → kept
        ];
        let out = coalesce_pointer_inputs(inputs, None);
        assert_eq!(
            out,
            vec![
                PointerInput::Move { x: 50.0, y: 50.0 },
                PointerInput::Move { x: 60.0, y: 50.0 },
            ]
        );
    }

    #[test]
    fn coalesce_does_not_collapse_moves_across_a_down() {
        // A press between two near-identical positions must survive: down/up
        // never move the coalescing anchor, but they must keep their order.
        let inputs = vec![
            PointerInput::Move { x: 50.0, y: 50.0 },
            PointerInput::Down { x: 50.0, y: 50.0 },
            PointerInput::Move { x: 50.2, y: 50.0 }, // still within 1px of anchor → dropped
        ];
        let out = coalesce_pointer_inputs(inputs, None);
        assert_eq!(
            out,
            vec![
                PointerInput::Move { x: 50.0, y: 50.0 },
                PointerInput::Down { x: 50.0, y: 50.0 },
            ]
        );
    }

    #[test]
    fn coalesce_uses_seed_to_drop_first_move_across_frame_boundary() {
        // The first move repeats the position applied on the previous drain.
        let inputs = vec![PointerInput::Move { x: 100.0, y: 100.0 }];
        let out = coalesce_pointer_inputs(inputs, Some((100.0, 100.0)));
        assert!(out.is_empty());
    }

    #[test]
    fn last_move_position_finds_the_most_recent_move() {
        let inputs = vec![
            PointerInput::Move { x: 10.0, y: 10.0 },
            PointerInput::Move { x: 30.0, y: 40.0 },
            PointerInput::Up { x: 30.0, y: 40.0 },
        ];
        assert_eq!(last_move_position(&inputs), Some((30.0, 40.0)));
        assert_eq!(
            last_move_position(&[PointerInput::Down { x: 1.0, y: 2.0 }]),
            None
        );
    }

    #[test]
    fn to_canvas_coords_scales_client_point_into_backing_store() {
        // 400px CSS box backed by an 800px buffer (dpr 2): client (210,110) with
        // rect origin (10,10) maps to ((210-10)*2, (110-10)*2) = (400, 200).
        let (x, y) = to_canvas_coords(210.0, 110.0, 10.0, 10.0, 400.0, 300.0, 800.0, 600.0);
        assert_eq!((x, y), (400.0, 200.0));
    }

    #[test]
    fn to_canvas_coords_falls_back_to_unit_scale_for_zero_sized_rect() {
        let (x, y) = to_canvas_coords(30.0, 20.0, 0.0, 0.0, 0.0, 0.0, 800.0, 600.0);
        assert_eq!((x, y), (30.0, 20.0));
    }
}
