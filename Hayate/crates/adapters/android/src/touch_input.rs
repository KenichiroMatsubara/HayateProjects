//! Android touch-input translation (ADR-0087, stage B).
//!
//! `android-activity` delivers `MotionEvent`s on the event loop; this module
//! maps a single pointer's action + surface-pixel coordinates to the
//! coordinate-based `hayate-core` pointer API (`on_pointer_down` /
//! `on_pointer_move` / `on_pointer_up`, already pointer-type-independent per
//! ADR-0082). It is kept free of `android_activity`/`ndk` types so the
//! translation is unit-testable on the host without the NDK — mirroring
//! `canvas_resize_metrics` (`hayate-adapter-web/src/resize_observer.rs`) and the
//! `ImeBridge` seam (`ElementTree::drive_ime` decides, the adapter reflects),
//! which push logic into host-testable pure functions and keep the dirty
//! platform glue thin.

/// A single pointer's touch action, mirroring Android `MotionAction` without
/// depending on `android_activity`/`ndk` types.
#[cfg(any(target_os = "android", test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchAction {
    Down,
    Move,
    Up,
    Cancel,
}

/// The `hayate-core` pointer call a touch action maps to, at surface pixels.
#[cfg(any(target_os = "android", test))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PointerInput {
    Down { x: f32, y: f32 },
    Move { x: f32, y: f32 },
    Up { x: f32, y: f32 },
}

/// Translate one Android touch action + surface-pixel coordinates into the
/// corresponding `hayate-core` pointer call.
#[cfg(any(target_os = "android", test))]
pub fn translate_touch(action: TouchAction, x: f32, y: f32) -> PointerInput {
    match action {
        TouchAction::Down => PointerInput::Down { x, y },
        TouchAction::Move => PointerInput::Move { x, y },
        // Cancel releases the active press (no `on_pointer_cancel` until #213).
        TouchAction::Up | TouchAction::Cancel => PointerInput::Up { x, y },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn down_maps_to_pointer_down() {
        assert_eq!(
            translate_touch(TouchAction::Down, 10.0, 20.0),
            PointerInput::Down { x: 10.0, y: 20.0 }
        );
    }

    #[test]
    fn move_maps_to_pointer_move() {
        assert_eq!(
            translate_touch(TouchAction::Move, 33.0, 44.0),
            PointerInput::Move { x: 33.0, y: 44.0 }
        );
    }

    #[test]
    fn up_maps_to_pointer_up() {
        assert_eq!(
            translate_touch(TouchAction::Up, 5.0, 6.0),
            PointerInput::Up { x: 5.0, y: 6.0 }
        );
    }

    // Cancel (e.g. scroll takeover / pointer capture loss) releases the active
    // press at the cancel coordinates. Core has no `on_pointer_cancel` yet
    // (that arrives with #213), so the closest existing behavior is a pointer
    // up, which prevents a stuck `:active` state.
    #[test]
    fn cancel_maps_to_pointer_up() {
        assert_eq!(
            translate_touch(TouchAction::Cancel, 7.0, 8.0),
            PointerInput::Up { x: 7.0, y: 8.0 }
        );
    }
}
