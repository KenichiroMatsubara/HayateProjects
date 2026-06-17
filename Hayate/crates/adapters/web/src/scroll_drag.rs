//! Pure touch/pen drag→scroll gesture logic for Canvas Mode (ADR-0082
//! Amendment, #350). The `CanvasRenderer` owns a `ScrollGesture` and drives it
//! from the drained pointer buffer; every decision (which pointers scroll, when
//! a press becomes a scroll, how a finger delta maps to a scroll offset) lives
//! here as a pure function unit-tested on all targets — the wasm wiring stays
//! thin (mirrors `coalesce_pointer_inputs`).
//!
//! Scope (tracer-bullet 1/3): 1:1 finger-following, clamped at the edges (no
//! inertia, no rubber-band). Offsets are applied via `element_set_scroll_offset`
//! (SCR-02, no clamp) so the adapter clamps to `[0, max]` here.

/// The physical pointer device axis is the core proto/wire concept
/// [`PointerKind`](hayate_core::PointerKind) (#357). Threaded from the DOM
/// `PointerEvent.pointerType` so only `touch`/`pen` enter the drag→scroll path;
/// `mouse` keeps its selection/drag behaviour unchanged.
pub use hayate_core::PointerKind;

/// Whether a pointer of this kind drives the touch drag→scroll gesture. `Touch`
/// and `Pen` do; `Mouse` is left on the selection/drag path (ADR-0082).
pub fn is_drag_scroll_pointer(kind: PointerKind) -> bool {
    matches!(kind, PointerKind::Touch | PointerKind::Pen)
}

/// Movement (px) a press must travel from its `pointerdown` before it is treated
/// as a scroll rather than a tap. Below it, releasing fires a normal click;
/// crossing it cancels the press and takes over scrolling. Named and defined
/// once (not a magic number) so it can be tuned later — iOS-ish default.
pub const SCROLL_SLOP_PX: f32 = 8.0;

/// Whether `current` has travelled more than `slop` px (Euclidean) from `start`.
/// The dead-zone is a radius so diagonal drags cross at the same distance on
/// every axis.
pub fn exceeds_slop(start: (f32, f32), current: (f32, f32), slop: f32) -> bool {
    let dx = current.0 - start.0;
    let dy = current.1 - start.1;
    dx * dx + dy * dy > slop * slop
}

/// What a single `pointermove` does to a live gesture, decided purely so the
/// wasm layer only has to act on the verdict.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MoveOutcome {
    /// Still inside the slop dead-zone — the press is an unresolved tap, nothing
    /// to apply (and the press stays alive so a release can still click).
    Pending,
    /// This move crossed the slop: the press must be cancelled now
    /// (`on_pointer_cancel`, #213) and scrolling takes over. No offset is
    /// applied on the takeover frame — the dead-zone is consumed so scrolling
    /// starts from here without a jump.
    StartScroll,
    /// Already scrolling: shift the locked scroll-view's offset by this finger
    /// delta (content follows the finger 1:1) before clamping to the edges.
    Scroll { dx: f32, dy: f32 },
}

/// A live touch/pen drag locked to one scroll-view (ADR-0082). Tracks the
/// `pointerdown` origin (for slop), the last position (for per-move deltas) and
/// whether the slop has been crossed. The renderer creates one on a touch/pen
/// `pointerdown` over a scroll-view and drives it from drained moves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollGesture {
    /// The scroll-view the gesture is locked to — it never chains to an ancestor
    /// mid-gesture (v1 scope).
    pub scroll_view: hayate_core::ElementId,
    start: (f32, f32),
    last: (f32, f32),
    scrolling: bool,
}

impl ScrollGesture {
    /// Begin a gesture pending at `start` (the `pointerdown` position), locked to
    /// `scroll_view`. Not scrolling until the slop is crossed.
    pub fn new(scroll_view: hayate_core::ElementId, start: (f32, f32)) -> Self {
        Self {
            scroll_view,
            start,
            last: start,
            scrolling: false,
        }
    }

    /// Classify a move to `pos`, advancing slop/scroll state. Returns the action
    /// the renderer must take. The finger delta for `Scroll` is `last - pos`
    /// (content follows the finger: dragging up scrolls content up).
    pub fn on_move(&mut self, pos: (f32, f32), slop: f32) -> MoveOutcome {
        if self.scrolling {
            let dx = self.last.0 - pos.0;
            let dy = self.last.1 - pos.1;
            self.last = pos;
            MoveOutcome::Scroll { dx, dy }
        } else if exceeds_slop(self.start, pos, slop) {
            self.scrolling = true;
            self.last = pos;
            MoveOutcome::StartScroll
        } else {
            MoveOutcome::Pending
        }
    }

    /// Whether releasing now should fire a click: true while the gesture never
    /// crossed the slop (a tap), false once it became a scroll.
    pub fn is_tap(&self) -> bool {
        !self.scrolling
    }
}

/// Clamp a single scroll axis into `[0, max]`. `element_set_scroll_offset` is
/// the un-clamped SCR-02 mechanism, so this slice stops at the edges here (no
/// rubber-band yet). A negative `max` (content shorter than the viewport)
/// collapses to `0` — the only valid offset.
pub fn clamp_scroll_axis(offset: f32, max: f32) -> f32 {
    offset.clamp(0.0, max.max(0.0))
}

// ── Momentum / inertia (issue #351, ADR-0082 Amendment, tracer-bullet 2/3) ──
//
// Flicking and lifting hands a release velocity to a friction integrator that
// keeps the locked scroll-view moving and decelerating until it rests (or hits
// an edge and clamp-stops — no bounce this slice). Both halves are pure: the
// `CanvasRenderer` records finger samples while dragging, estimates the release
// velocity here on `pointerup`, then steps the decay once per rAF frame.

/// iOS-style scroll physics, gathered in one block so every coefficient stays a
/// single named, tunable knob instead of a magic number sprinkled through the
/// integrator (issue #351). Values are starting points to be tuned, not
/// load-bearing — adjust here and the whole feel changes.
pub mod physics {
    /// Per-millisecond velocity retention under friction. Matches UIScrollView's
    /// "normal" deceleration rate: after `t` ms a fling keeps `0.998^t` of its
    /// speed, so it bleeds off smoothly over roughly a second.
    pub const DECELERATION_RATE: f32 = 0.998;
    /// Release-fling speed cap (px/ms ≈ 4000 px/s) so a violent flick can't hurl
    /// the content across the entire document in a single frame.
    pub const MAX_RELEASE_VELOCITY: f32 = 4.0;
    /// Speed (px/ms) below which momentum is treated as stopped and snaps to
    /// rest — about a sub-pixel per 60fps frame — so the animation terminates
    /// instead of crawling asymptotically toward zero.
    pub const MIN_VELOCITY: f32 = 0.02;
    /// Only finger samples within this window (ms) of the most recent one feed
    /// the release-velocity estimate, so a press that pauses before lifting
    /// releases at rest rather than replaying a stale early flick.
    pub const SAMPLE_WINDOW_MS: f64 = 100.0;
}

/// Clamp a velocity (px/ms) to the symmetric release cap.
fn cap_release_velocity(v: f32) -> f32 {
    v.clamp(-physics::MAX_RELEASE_VELOCITY, physics::MAX_RELEASE_VELOCITY)
}

/// Estimate the release (fling) velocity in **offset space** (px/ms) from a
/// sequence of finger samples `(x, y, timestamp_ms)` in arrival order. Offset
/// space is the scroll-offset's sign convention — content follows the finger —
/// so a finger sliding up returns a positive `vy`, the same direction the drag
/// delta moves the offset.
///
/// Only samples within [`physics::SAMPLE_WINDOW_MS`] of the most recent one
/// contribute, so a finger that paused before lifting releases at rest. The
/// estimate is the average velocity across that window (first → last), capped per
/// axis at [`physics::MAX_RELEASE_VELOCITY`]. Fewer than two in-window samples,
/// or a zero-duration span, yield no fling `(0.0, 0.0)`.
pub fn estimate_release_velocity(samples: &[(f32, f32, f64)]) -> (f32, f32) {
    let Some(&(last_x, last_y, last_t)) = samples.last() else {
        return (0.0, 0.0);
    };
    let window_start = last_t - physics::SAMPLE_WINDOW_MS;
    let Some(&(first_x, first_y, first_t)) = samples.iter().find(|&&(_, _, t)| t >= window_start)
    else {
        return (0.0, 0.0);
    };
    let dt = (last_t - first_t) as f32;
    if dt <= 0.0 {
        return (0.0, 0.0);
    }
    // Offset moves opposite the finger: offset delta = old_pos − new_pos.
    (
        cap_release_velocity((first_x - last_x) / dt),
        cap_release_velocity((first_y - last_y) / dt),
    )
}

/// Advance one momentum axis by `dt_ms` under exponential friction. Returns the
/// offset delta to apply this frame and the decayed velocity to carry into the
/// next, both in offset space (px and px/ms). Once the decayed speed falls below
/// [`physics::MIN_VELOCITY`] it snaps to `0.0`, so the caller can end the
/// animation instead of integrating an asymptotic crawl.
pub fn momentum_step(velocity: f32, dt_ms: f32) -> (f32, f32) {
    let delta = velocity * dt_ms;
    let next = velocity * physics::DECELERATION_RATE.powf(dt_ms);
    if next.abs() < physics::MIN_VELOCITY {
        (delta, 0.0)
    } else {
        (delta, next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touch_and_pen_drive_scroll_but_mouse_does_not() {
        assert!(is_drag_scroll_pointer(PointerKind::Touch));
        assert!(is_drag_scroll_pointer(PointerKind::Pen));
        assert!(!is_drag_scroll_pointer(PointerKind::Mouse));
    }

    #[test]
    fn slop_is_a_named_tunable_constant_not_a_magic_number() {
        // Pinned so the dead-zone stays a single named knob; value is iOS-ish.
        assert_eq!(SCROLL_SLOP_PX, 8.0);
    }

    #[test]
    fn movement_within_the_slop_radius_is_not_yet_a_scroll() {
        let start = (100.0, 100.0);
        // 5px straight, and ~7.07px diagonal — both inside the 8px radius.
        assert!(!exceeds_slop(start, (105.0, 100.0), SCROLL_SLOP_PX));
        assert!(!exceeds_slop(start, (105.0, 105.0), SCROLL_SLOP_PX));
    }

    #[test]
    fn movement_past_the_slop_radius_becomes_a_scroll() {
        let start = (100.0, 100.0);
        assert!(exceeds_slop(start, (100.0, 109.0), SCROLL_SLOP_PX));
        assert!(exceeds_slop(start, (108.1, 100.0), SCROLL_SLOP_PX));
    }

    fn sv() -> hayate_core::ElementId {
        hayate_core::ElementId::from_u64(1)
    }

    #[test]
    fn release_velocity_is_the_offset_space_speed_over_the_recent_samples() {
        // Finger slides up (y: 100 → 40) over 60ms. Content follows the finger,
        // so the offset gains 60px in 60ms → +1 px/ms in offset space. X is still.
        let samples = [(0.0, 100.0, 0.0), (0.0, 70.0, 30.0), (0.0, 40.0, 60.0)];
        let (vx, vy) = estimate_release_velocity(&samples);
        assert_eq!(vx, 0.0);
        assert!((vy - 1.0).abs() < 1e-6, "vy = {vy}");
    }

    #[test]
    fn release_velocity_needs_two_in_window_samples_with_a_real_time_span() {
        // No samples, or a single one, give nothing to measure speed against.
        assert_eq!(estimate_release_velocity(&[]), (0.0, 0.0));
        assert_eq!(estimate_release_velocity(&[(0.0, 0.0, 5.0)]), (0.0, 0.0));
        // Two samples stamped at the same instant: a position jump with no
        // elapsed time is not a measurable velocity (avoids a divide-by-zero).
        assert_eq!(
            estimate_release_velocity(&[(0.0, 0.0, 5.0), (0.0, 50.0, 5.0)]),
            (0.0, 0.0),
        );
    }

    #[test]
    fn samples_older_than_the_window_are_ignored_so_a_pause_releases_at_rest() {
        // A fast slide long ago, then the finger came to rest at y=40 for the
        // last 60ms before lifting — only the resting samples are in-window.
        let samples = [
            (0.0, 200.0, 0.0), // outside the 100ms window before the lift
            (0.0, 40.0, 500.0),
            (0.0, 40.0, 560.0),
        ];
        let (_, vy) = estimate_release_velocity(&samples);
        assert_eq!(vy, 0.0, "a finger that paused before lifting releases at rest");
    }

    #[test]
    fn release_velocity_is_capped_so_a_violent_flick_cannot_launch_too_far() {
        // 1000px in 10ms = 100 px/ms, far above the cap.
        let samples = [(0.0, 1000.0, 0.0), (0.0, 0.0, 10.0)];
        let (_, vy) = estimate_release_velocity(&samples);
        assert_eq!(vy, physics::MAX_RELEASE_VELOCITY);
    }

    #[test]
    fn momentum_advances_in_its_direction_and_friction_bleeds_the_speed() {
        // A 16ms frame: the offset advances along the velocity, and the carried
        // velocity is smaller but keeps its sign (decelerating, not reversing).
        let (delta, next) = momentum_step(2.0, 16.0);
        assert!(delta > 0.0, "offset advances in the velocity direction");
        assert!(next > 0.0 && next < 2.0, "friction bleeds speed, keeps sign (next = {next})");
        // Symmetric for a downward fling.
        let (delta_neg, next_neg) = momentum_step(-2.0, 16.0);
        assert!(delta_neg < 0.0);
        assert!(next_neg < 0.0 && next_neg > -2.0);
    }

    #[test]
    fn momentum_snaps_to_rest_once_it_drops_below_the_stop_threshold() {
        // Starting right at the threshold, one long frame decays it under
        // MIN_VELOCITY, so it snaps to 0 instead of crawling forever.
        let (_, next) = momentum_step(physics::MIN_VELOCITY, 1000.0);
        assert_eq!(next, 0.0, "below the stop threshold momentum ends");
    }

    #[test]
    fn physics_coefficients_are_named_constants_gathered_in_one_place() {
        // Pinned so the iOS-ish knobs stay a single tunable block, not magic
        // numbers scattered through the integrator.
        assert_eq!(physics::DECELERATION_RATE, 0.998);
        assert_eq!(physics::MAX_RELEASE_VELOCITY, 4.0);
        assert_eq!(physics::MIN_VELOCITY, 0.02);
        assert_eq!(physics::SAMPLE_WINDOW_MS, 100.0);
    }

    #[test]
    fn a_move_within_slop_keeps_the_gesture_a_pending_tap() {
        let mut g = ScrollGesture::new(sv(), (100.0, 100.0));
        assert_eq!(g.on_move((104.0, 100.0), SCROLL_SLOP_PX), MoveOutcome::Pending);
        assert!(g.is_tap(), "an unresolved press is still a tap → click on release");
    }

    #[test]
    fn crossing_slop_takes_over_scrolling_without_applying_a_delta() {
        let mut g = ScrollGesture::new(sv(), (100.0, 100.0));
        // 20px up crosses the 8px dead-zone.
        assert_eq!(g.on_move((100.0, 80.0), SCROLL_SLOP_PX), MoveOutcome::StartScroll);
        assert!(!g.is_tap(), "after takeover a release must not click");
    }

    #[test]
    fn while_scrolling_content_follows_the_finger_one_to_one() {
        let mut g = ScrollGesture::new(sv(), (100.0, 100.0));
        g.on_move((100.0, 80.0), SCROLL_SLOP_PX); // takeover, last = (100,80)
        // Finger continues up to y=60: content follows → offset increases by 20.
        assert_eq!(
            g.on_move((100.0, 60.0), SCROLL_SLOP_PX),
            MoveOutcome::Scroll { dx: 0.0, dy: 20.0 },
        );
        // Finger back down to y=70: offset decreases by 10. Delta is measured
        // from the previous move, not the origin.
        assert_eq!(
            g.on_move((100.0, 70.0), SCROLL_SLOP_PX),
            MoveOutcome::Scroll { dx: 0.0, dy: -10.0 },
        );
    }

    #[test]
    fn scroll_axis_stops_at_both_edges() {
        assert_eq!(clamp_scroll_axis(50.0, 200.0), 50.0); // mid-range untouched
        assert_eq!(clamp_scroll_axis(-30.0, 200.0), 0.0); // past the top edge
        assert_eq!(clamp_scroll_axis(260.0, 200.0), 200.0); // past the bottom edge
        assert_eq!(clamp_scroll_axis(10.0, -5.0), 0.0); // no scrollable range
    }

}
