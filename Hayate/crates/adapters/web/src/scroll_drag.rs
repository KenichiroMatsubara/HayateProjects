//! Pure touch/pen drag→scroll gesture logic for Canvas Mode (ADR-0082
//! Amendment, #350). The `CanvasRenderer` owns a `ScrollGesture` and drives it
//! from the drained pointer buffer; every decision (which pointers scroll, when
//! a press becomes a scroll, how a finger delta maps to a scroll offset) lives
//! here as a pure function unit-tested on all targets — the wasm wiring stays
//! thin (mirrors `coalesce_pointer_inputs`).
//!
//! Scope (tracer-bullets 1–3/3): 1:1 finger-following inside the range, flick
//! inertia (#351), and rubber-band overscroll with spring-back / bounce (#352).
//! Offsets are applied via `element_set_scroll_offset` (SCR-02, un-clamped); the
//! edge behaviour — resistance while dragging past it, a spring pulling it back
//! on release or after an inertial bounce — lives in the pure functions here.

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
    /// delta (content follows the finger 1:1 inside the range; past an edge the
    /// rubber-band resists it).
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

// ── Momentum / inertia (issue #351, ADR-0082 Amendment, tracer-bullet 2/3) ──
//
// Flicking and lifting hands a release velocity to a friction integrator that
// keeps the locked scroll-view moving and decelerating until it rests (or hits
// an edge and clamp-stops — no bounce this slice). Both halves are pure: the
// `CanvasRenderer` records finger samples while dragging, estimates the release
// velocity here on `pointerup`, then steps the decay once per rAF frame.

/// iOS-style scroll physics, gathered in one block so every coefficient stays a
/// single named, tunable knob instead of a magic number sprinkled through the
/// integrator (issue #351). The fling cap and the spring-back values were
/// calibrated on-device (#353) via the `tuning.json` overlay and baked back
/// here; adjust here and the whole feel changes.
pub mod physics {
    /// Per-millisecond velocity retention under friction. Matches UIScrollView's
    /// "normal" deceleration rate: after `t` ms a fling keeps `0.998^t` of its
    /// speed, so it bleeds off smoothly over roughly a second.
    pub const DECELERATION_RATE: f32 = 0.998;
    /// Release-fling speed cap (px/ms ≈ 16000 px/s) so a violent flick can't hurl
    /// the content across the entire document in a single frame. Calibrated
    /// on-device (#353) — a much snappier ceiling than the initial 4.0.
    pub const MAX_RELEASE_VELOCITY: f32 = 16.0;
    /// Speed (px/ms) below which momentum is treated as stopped and snaps to
    /// rest — about a sub-pixel per 60fps frame — so the animation terminates
    /// instead of crawling asymptotically toward zero.
    pub const MIN_VELOCITY: f32 = 0.02;
    /// Only finger samples within this window (ms) of the most recent one feed
    /// the release-velocity estimate, so a press that pauses before lifting
    /// releases at rest rather than replaying a stale early flick.
    pub const SAMPLE_WINDOW_MS: f64 = 100.0;

    // ── Overscroll / spring-back (issue #352, tracer-bullet 3/3) ──

    /// Rubber-band resistance constant — the fraction of raw finger travel that
    /// reaches the content at the very edge (the initial slope of the curve).
    /// Matches iOS's `0.55`: drag one pixel past the edge and the content moves
    /// roughly half a pixel, growing "heavier" the further you pull.
    pub const RUBBER_BAND_C: f32 = 0.55;
    /// Spring stiffness (px/ms² per px) pulling an overscrolled edge back to
    /// rest. Calibrated on-device (#353) via the `tuning.jsonc` overlay and
    /// baked back here — softer than the initial 0.0003 for a gentler return.
    pub const SPRING_STIFFNESS: f32 = 0.0001;
    /// Spring damping (px/ms per px/ms), held a touch *under* critical
    /// (`2 * sqrt(SPRING_STIFFNESS)` ≈ 0.02) for a livelier bounce. The lighter
    /// damping would normally let the bounce ring past the boundary, but
    /// [`scroll_motion_step`] snaps a bounce to rest exactly **at** the edge, so
    /// that overshoot is clamped away and only the snappier feel remains.
    /// Calibrated on-device (#353) via the `tuning.jsonc` overlay.
    pub const SPRING_DAMPING: f32 = 0.015;
    /// Displacement (px) from the edge below which spring-back is considered home.
    pub const SPRING_REST_OFFSET: f32 = 0.5;
    /// Velocity (px/ms) below which — once within [`SPRING_REST_OFFSET`] — the
    /// spring snaps to the edge and the animation ends. Calibrated on-device
    /// (#353) — ends the settle a touch earlier than the initial 0.05.
    pub const SPRING_REST_VELOCITY: f32 = 0.10;
}

/// A live, overridable copy of the scroll-physics knobs. The [`physics`] consts
/// (and [`SCROLL_SLOP_PX`]) remain the authoritative defaults — [`Default`]
/// reads them, so the numbers are never restated — but a dev build can overlay
/// values at runtime (a `tuning.json` loaded on init) to feel-tune on a real
/// device without recompiling. Production ships with no override, so every field
/// equals its const and the read is a plain struct load (no perf cost over the
/// old `const` reference).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollPhysicsTuning {
    pub slop_px: f32,
    pub deceleration_rate: f32,
    pub max_release_velocity: f32,
    pub min_velocity: f32,
    pub sample_window_ms: f64,
    pub rubber_band_c: f32,
    pub spring_stiffness: f32,
    pub spring_damping: f32,
    pub spring_rest_offset: f32,
    pub spring_rest_velocity: f32,
}

impl Default for ScrollPhysicsTuning {
    fn default() -> Self {
        // Mirror the authoritative consts — do not restate the literals here, so
        // the `physics` block stays the single source of the default numbers.
        Self {
            slop_px: SCROLL_SLOP_PX,
            deceleration_rate: physics::DECELERATION_RATE,
            max_release_velocity: physics::MAX_RELEASE_VELOCITY,
            min_velocity: physics::MIN_VELOCITY,
            sample_window_ms: physics::SAMPLE_WINDOW_MS,
            rubber_band_c: physics::RUBBER_BAND_C,
            spring_stiffness: physics::SPRING_STIFFNESS,
            spring_damping: physics::SPRING_DAMPING,
            spring_rest_offset: physics::SPRING_REST_OFFSET,
            spring_rest_velocity: physics::SPRING_REST_VELOCITY,
        }
    }
}

/// iOS-style rubber-band resistance for a single axis. `raw` is the offset a 1:1
/// finger drag would reach; the return is the *displayed* offset. Inside
/// `[0, max]` the drag passes through untouched; pulled past an edge the content
/// lags behind with a resistance that grows the further out it goes (each extra
/// pixel of drag moves the content less), asymptotically approaching `dimension`
/// of overscroll so the edge feels "heavy" but never tears off the screen.
/// Symmetric at both edges. A non-positive `dimension` disables overscroll
/// (the raw offset is returned as-is past the edge — nothing to rubber-band).
pub fn rubber_band_offset(raw: f32, max: f32, dimension: f32, t: &ScrollPhysicsTuning) -> f32 {
    let max = max.max(0.0);
    if raw >= 0.0 && raw <= max {
        raw
    } else if raw < 0.0 {
        -overscroll_curve(-raw, dimension, t)
    } else {
        max + overscroll_curve(raw - max, dimension, t)
    }
}

/// The resisted overscroll distance for `x` px of raw pull past an edge:
/// `(1 − 1/(x·c/d + 1))·d`. Zero at the edge, slope `c` ([`physics::RUBBER_BAND_C`])
/// there, concave, and bounded by `dimension` as `x → ∞`.
fn overscroll_curve(x: f32, dimension: f32, t: &ScrollPhysicsTuning) -> f32 {
    if dimension <= 0.0 || x <= 0.0 {
        return x.max(0.0);
    }
    (1.0 - 1.0 / (x * t.rubber_band_c / dimension + 1.0)) * dimension
}

/// Clamp a velocity (px/ms) to the symmetric release cap.
fn cap_release_velocity(v: f32, t: &ScrollPhysicsTuning) -> f32 {
    v.clamp(-t.max_release_velocity, t.max_release_velocity)
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
pub fn estimate_release_velocity(samples: &[(f32, f32, f64)], t: &ScrollPhysicsTuning) -> (f32, f32) {
    let Some(&(last_x, last_y, last_t)) = samples.last() else {
        return (0.0, 0.0);
    };
    let window_start = last_t - t.sample_window_ms;
    let Some(&(first_x, first_y, first_t)) = samples.iter().find(|&&(_, _, ts)| ts >= window_start)
    else {
        return (0.0, 0.0);
    };
    let dt = (last_t - first_t) as f32;
    if dt <= 0.0 {
        return (0.0, 0.0);
    }
    // Offset moves opposite the finger: offset delta = old_pos − new_pos.
    (
        cap_release_velocity((first_x - last_x) / dt, t),
        cap_release_velocity((first_y - last_y) / dt, t),
    )
}

/// Advance one momentum axis by `dt_ms` under exponential friction. Returns the
/// offset delta to apply this frame and the decayed velocity to carry into the
/// next, both in offset space (px and px/ms). Once the decayed speed falls below
/// [`physics::MIN_VELOCITY`] it snaps to `0.0`, so the caller can end the
/// animation instead of integrating an asymptotic crawl.
pub fn momentum_step(velocity: f32, dt_ms: f32, t: &ScrollPhysicsTuning) -> (f32, f32) {
    let delta = velocity * dt_ms;
    let next = velocity * t.deceleration_rate.powf(dt_ms);
    if next.abs() < t.min_velocity {
        (delta, 0.0)
    } else {
        (delta, next)
    }
}

/// Advance one spring-back axis by `dt_ms` toward its edge (issue #352).
/// `displacement` is the signed overscroll distance from the edge (negative past
/// the top, positive past the bottom) and `velocity` is its rate (px/ms, offset
/// space). A (near) critically-damped spring — [`physics::SPRING_STIFFNESS`] /
/// [`physics::SPRING_DAMPING`] — pulls the displacement to zero: a finger
/// released in overscroll eases back, and a fling that bounced past the edge
/// (entering with outward velocity) overshoots then returns without ringing.
/// Returns the next `(displacement, velocity)`, snapping to `(0.0, 0.0)` — home,
/// animation over — once both fall within their rest thresholds.
pub fn spring_step(displacement: f32, velocity: f32, dt_ms: f32, t: &ScrollPhysicsTuning) -> (f32, f32) {
    // Semi-implicit (symplectic) Euler: integrate velocity first, then position,
    // so the spring stays stable at frame-sized dt.
    let accel = -t.spring_stiffness * displacement - t.spring_damping * velocity;
    let next_v = velocity + accel * dt_ms;
    let next_x = displacement + next_v * dt_ms;
    if next_x.abs() < t.spring_rest_offset && next_v.abs() < t.spring_rest_velocity {
        (0.0, 0.0)
    } else {
        (next_x, next_v)
    }
}

/// Advance one axis of a released scroll by `dt_ms`, picking the right physics
/// from where the offset sits (issue #352). Inside `[0, max]` it coasts under
/// friction ([`momentum_step`]); a fling that runs off the edge keeps its
/// velocity and crosses into overscroll, where the next frame [`spring_step`]
/// pulls it back — so inertia reaching an edge bounces and returns. A release
/// already in overscroll springs straight home. Returns the next
/// `(offset, velocity)`; the offset is un-clamped (SCR-02) because overscroll is
/// the whole point. The caller stops the animation once velocity rests **and**
/// the offset is back within `[0, max]`.
///
/// A bounce settles **at** the edge it hit: when the spring would carry the
/// content back across the edge into the range, the offset snaps to the edge at
/// zero velocity rather than handing its residual inward speed to
/// [`momentum_step`]. So a fling overshoots the boundary exactly once and comes
/// to rest there — it can never re-cross the boundary and ping-pong between the
/// two edges, however the spring is tuned (#352 follow-up).
pub fn scroll_motion_step(offset: f32, velocity: f32, max: f32, dt_ms: f32, t: &ScrollPhysicsTuning) -> (f32, f32) {
    let max = max.max(0.0);
    if offset < 0.0 {
        // Past the top edge (edge = 0): spring toward it. A non-negative result
        // means the spring reached / crossed the edge — settle exactly there.
        let (disp, v) = spring_step(offset, velocity, dt_ms, t);
        if disp >= 0.0 {
            (0.0, 0.0)
        } else {
            (disp, v)
        }
    } else if offset > max {
        // Past the bottom edge (edge = max): symmetric — a non-positive
        // displacement means it returned to / past the edge, so rest at `max`.
        let (disp, v) = spring_step(offset - max, velocity, dt_ms, t);
        if disp <= 0.0 {
            (max, 0.0)
        } else {
            (max + disp, v)
        }
    } else {
        let (delta, v) = momentum_step(velocity, dt_ms, t);
        (offset + delta, v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Default tuning for the physics functions — equals the authoritative consts,
    /// so the behavioural assertions below are unchanged by the added parameter.
    fn t() -> ScrollPhysicsTuning {
        ScrollPhysicsTuning::default()
    }

    #[test]
    fn default_tuning_mirrors_the_authoritative_consts() {
        // Locks the invariant that `ScrollPhysicsTuning::default()` reflects the
        // `physics` block: a future const edit that forgets the struct is caught.
        let d = ScrollPhysicsTuning::default();
        assert_eq!(d.slop_px, SCROLL_SLOP_PX);
        assert_eq!(d.deceleration_rate, physics::DECELERATION_RATE);
        assert_eq!(d.max_release_velocity, physics::MAX_RELEASE_VELOCITY);
        assert_eq!(d.min_velocity, physics::MIN_VELOCITY);
        assert_eq!(d.sample_window_ms, physics::SAMPLE_WINDOW_MS);
        assert_eq!(d.rubber_band_c, physics::RUBBER_BAND_C);
        assert_eq!(d.spring_stiffness, physics::SPRING_STIFFNESS);
        assert_eq!(d.spring_damping, physics::SPRING_DAMPING);
        assert_eq!(d.spring_rest_offset, physics::SPRING_REST_OFFSET);
        assert_eq!(d.spring_rest_velocity, physics::SPRING_REST_VELOCITY);
    }

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
        let (vx, vy) = estimate_release_velocity(&samples, &t());
        assert_eq!(vx, 0.0);
        assert!((vy - 1.0).abs() < 1e-6, "vy = {vy}");
    }

    #[test]
    fn release_velocity_needs_two_in_window_samples_with_a_real_time_span() {
        // No samples, or a single one, give nothing to measure speed against.
        assert_eq!(estimate_release_velocity(&[], &t()), (0.0, 0.0));
        assert_eq!(estimate_release_velocity(&[(0.0, 0.0, 5.0)], &t()), (0.0, 0.0));
        // Two samples stamped at the same instant: a position jump with no
        // elapsed time is not a measurable velocity (avoids a divide-by-zero).
        assert_eq!(
            estimate_release_velocity(&[(0.0, 0.0, 5.0), (0.0, 50.0, 5.0)], &t()),
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
        let (_, vy) = estimate_release_velocity(&samples, &t());
        assert_eq!(vy, 0.0, "a finger that paused before lifting releases at rest");
    }

    #[test]
    fn release_velocity_is_capped_so_a_violent_flick_cannot_launch_too_far() {
        // 1000px in 10ms = 100 px/ms, far above the cap.
        let samples = [(0.0, 1000.0, 0.0), (0.0, 0.0, 10.0)];
        let (_, vy) = estimate_release_velocity(&samples, &t());
        assert_eq!(vy, physics::MAX_RELEASE_VELOCITY);
    }

    #[test]
    fn momentum_advances_in_its_direction_and_friction_bleeds_the_speed() {
        // A 16ms frame: the offset advances along the velocity, and the carried
        // velocity is smaller but keeps its sign (decelerating, not reversing).
        let (delta, next) = momentum_step(2.0, 16.0, &t());
        assert!(delta > 0.0, "offset advances in the velocity direction");
        assert!(next > 0.0 && next < 2.0, "friction bleeds speed, keeps sign (next = {next})");
        // Symmetric for a downward fling.
        let (delta_neg, next_neg) = momentum_step(-2.0, 16.0, &t());
        assert!(delta_neg < 0.0);
        assert!(next_neg < 0.0 && next_neg > -2.0);
    }

    #[test]
    fn momentum_snaps_to_rest_once_it_drops_below_the_stop_threshold() {
        // Starting right at the threshold, one long frame decays it under
        // MIN_VELOCITY, so it snaps to 0 instead of crawling forever.
        let (_, next) = momentum_step(physics::MIN_VELOCITY, 1000.0, &t());
        assert_eq!(next, 0.0, "below the stop threshold momentum ends");
    }

    #[test]
    fn physics_coefficients_are_named_constants_gathered_in_one_place() {
        // Pinned so the iOS-ish knobs stay a single tunable block, not magic
        // numbers scattered through the integrator.
        assert_eq!(physics::DECELERATION_RATE, 0.998);
        assert_eq!(physics::MAX_RELEASE_VELOCITY, 16.0);
        assert_eq!(physics::MIN_VELOCITY, 0.02);
        assert_eq!(physics::SAMPLE_WINDOW_MS, 100.0);
        // Overscroll / spring-back knobs (#352) live in the same block.
        assert_eq!(physics::RUBBER_BAND_C, 0.55);
        assert_eq!(physics::SPRING_STIFFNESS, 0.0001);
        assert_eq!(physics::SPRING_DAMPING, 0.015);
        assert_eq!(physics::SPRING_REST_OFFSET, 0.5);
        assert_eq!(physics::SPRING_REST_VELOCITY, 0.10);
    }

    #[test]
    fn within_range_the_drag_follows_the_finger_one_to_one() {
        // No rubber-band inside the scrollable range: the displayed offset is the
        // raw finger offset, at both ends and the middle.
        assert_eq!(rubber_band_offset(0.0, 400.0, 200.0, &t()), 0.0);
        assert_eq!(rubber_band_offset(150.0, 400.0, 200.0, &t()), 150.0);
        assert_eq!(rubber_band_offset(400.0, 400.0, 200.0, &t()), 400.0);
    }

    #[test]
    fn pulling_past_an_edge_resists_so_the_content_lags_the_finger() {
        // 100px of raw pull past the top edge shows less than 100px of overscroll
        // (resisted), and stays on the overscroll side of the edge.
        let shown = rubber_band_offset(-100.0, 400.0, 200.0, &t());
        assert!(shown < 0.0, "overscroll is past the top edge (got {shown})");
        assert!(shown > -100.0, "resisted: content lags the finger (got {shown})");
        // Symmetric past the bottom edge (max = 400).
        let shown_bottom = rubber_band_offset(500.0, 400.0, 200.0, &t());
        assert!(shown_bottom > 400.0 && shown_bottom < 500.0, "got {shown_bottom}");
        assert!(
            (shown_bottom - 400.0 + shown).abs() < 1e-3,
            "the curve is symmetric at both edges",
        );
    }

    #[test]
    fn the_further_past_the_edge_the_heavier_each_extra_pixel_moves() {
        // Equal raw increments yield ever-smaller displayed increments: the
        // diminishing-returns "heavy" feel of a rubber band.
        let near = rubber_band_offset(-50.0, 400.0, 200.0, &t()).abs();
        let mid = rubber_band_offset(-100.0, 400.0, 200.0, &t()).abs();
        let far = rubber_band_offset(-150.0, 400.0, 200.0, &t()).abs();
        let first_step = mid - near;
        let second_step = far - mid;
        assert!(mid > near && far > mid, "still monotonic outward");
        assert!(
            second_step < first_step,
            "each further pull moves the content less ({second_step} !< {first_step})",
        );
    }

    #[test]
    fn overscroll_is_bounded_so_the_content_never_tears_off_screen() {
        // Even an enormous pull cannot reveal more than `dimension` of overscroll.
        let extreme = rubber_band_offset(-100_000.0, 400.0, 200.0, &t());
        assert!(extreme > -200.0, "overscroll asymptotes to the dimension (got {extreme})");
    }

    #[test]
    fn spring_back_eases_an_overscrolled_edge_toward_home() {
        // Released 60px past the top edge at rest: the spring pulls the
        // displacement back toward zero (smaller magnitude, moving inward).
        let (x, v) = spring_step(-60.0, 0.0, 16.0, &t());
        assert!(x > -60.0 && x < 0.0, "displacement shrinks toward the edge (got {x})");
        assert!(v > 0.0, "velocity points back toward the edge (got {v})");
    }

    #[test]
    fn spring_back_converges_to_the_edge_and_ends() {
        // From a deep overscroll at rest, repeated steps must reach home (0,0) in
        // a bounded number of frames — the animation terminates, it doesn't crawl.
        let mut x = -120.0;
        let mut v = 0.0;
        let mut frames = 0;
        while (x, v) != (0.0, 0.0) {
            let (nx, nv) = spring_step(x, v, 16.0, &t());
            x = nx;
            v = nv;
            frames += 1;
            assert!(frames < 1000, "spring-back must settle, not ring forever");
        }
        assert_eq!((x, v), (0.0, 0.0));
    }

    #[test]
    fn a_fling_bounce_overshoots_past_the_edge_then_returns() {
        // Inertia reaches the edge (displacement 0) still moving outward: the
        // spring lets it bounce past, then brings it home without crossing to the
        // opposite side (critically damped, no ringing). Follow one trajectory
        // launched at the edge with outward velocity.
        let mut x = 0.0_f32;
        let mut v = -2.0_f32;
        let mut min_x = 0.0_f32;
        for _ in 0..1000 {
            let (nx, nv) = spring_step(x, v, 16.0, &t());
            x = nx;
            v = nv;
            min_x = min_x.min(x);
            if (x, v) == (0.0, 0.0) {
                break;
            }
        }
        assert!(min_x < 0.0, "the bounce carried the content past the edge (min {min_x})");
        assert_eq!((x, v), (0.0, 0.0), "and eased back to rest at the edge");
    }

    #[test]
    fn spring_back_snaps_home_once_within_the_rest_thresholds() {
        // A sub-pixel displacement at near-zero velocity is home — snap to the
        // edge so the animation stops instead of asymptoting.
        assert_eq!(spring_step(0.2, 0.0, 16.0, &t()), (0.0, 0.0));
    }

    #[test]
    fn motion_inside_the_range_coasts_under_friction() {
        // Well within [0, 400], a released fling decelerates like plain momentum:
        // the offset advances along the velocity, the speed bleeds off.
        let (offset, v) = scroll_motion_step(100.0, 2.0, 400.0, 16.0, &t());
        assert!(offset > 100.0, "coasts forward (got {offset})");
        assert!(v > 0.0 && v < 2.0, "friction bleeds the speed (got {v})");
    }

    #[test]
    fn inertia_reaching_an_edge_carries_past_it_into_overscroll() {
        // A fling that overruns the bottom edge crosses into overscroll (offset >
        // max) still moving outward, so the next frame can bounce it back.
        let (offset, v) = scroll_motion_step(395.0, 2.0, 400.0, 16.0, &t());
        assert!(offset > 400.0, "inertia carries past the edge (got {offset})");
        assert!(v > 0.0, "still moving outward, to be sprung back next frame (got {v})");
    }

    #[test]
    fn motion_in_overscroll_springs_back_toward_the_edge() {
        // Past the bottom edge at rest: spring-back pulls the offset toward max
        // and the velocity points inward.
        let (offset, v) = scroll_motion_step(440.0, 0.0, 400.0, 16.0, &t());
        assert!(offset < 440.0 && offset > 400.0, "eases back toward the edge (got {offset})");
        assert!(v < 0.0, "velocity points back inward (got {v})");
        // Symmetric past the top edge.
        let (top_offset, top_v) = scroll_motion_step(-40.0, 0.0, 400.0, 16.0, &t());
        assert!(top_offset < 0.0 && top_offset > -40.0, "got {top_offset}");
        assert!(top_v > 0.0, "got {top_v}");
    }

    #[test]
    fn a_flick_that_overruns_the_edge_bounces_and_settles_back_at_it() {
        // End to end on the pure layer: a strong fling overshoots the bottom edge,
        // is seen in overscroll at some frame, then spring-back returns it to rest
        // exactly at the edge.
        let max = 400.0;
        let mut offset = 380.0;
        let mut v = 3.0; // strong enough to overrun the 20px left to the edge
        let mut max_seen = offset;
        let mut settled = None;
        for frame in 0..2000 {
            let (no, nv) = scroll_motion_step(offset, v, max, 16.0, &t());
            offset = no;
            v = nv;
            max_seen = max_seen.max(offset);
            if nv == 0.0 && (0.0..=max).contains(&offset) {
                settled = Some(frame);
                break;
            }
        }
        assert!(max_seen > max, "the fling bounced into overscroll (peak {max_seen})");
        assert!(settled.is_some(), "and the bounce settled");
        assert!((offset - max).abs() < 1.0, "resting at the edge (got {offset})");
    }

    #[test]
    fn a_bounce_settles_at_the_edge_and_never_re_crosses_the_boundary() {
        // Structural guarantee (#352 follow-up): once a fling has bounced into
        // overscroll, the spring brings it to rest AT the edge it hit — its
        // residual inward velocity is never handed back to momentum, so the
        // content can't shoot back across the range and ping-pong between the two
        // edges. Drive a violent flick well past the bottom edge and watch the
        // frame it returns to the range: it must arrive exactly at the edge, at
        // rest, never re-entering with speed to spare.
        let max = 400.0;
        let mut offset = 380.0;
        let mut v = 8.0; // far overruns the 20px left to the edge
        let mut re_entered_with_speed = false;
        let mut rest_offset = None;
        for _ in 0..4000 {
            let (no, nv) = scroll_motion_step(offset, v, max, 16.0, &t());
            // The transition from overscroll (offset > max) back into the range.
            if offset > max && no <= max {
                re_entered_with_speed = nv != 0.0 || (no - max).abs() > 1e-3;
            }
            offset = no;
            v = nv;
            if v == 0.0 && (0.0..=max).contains(&offset) {
                rest_offset = Some(offset);
                break;
            }
        }
        assert!(!re_entered_with_speed, "the bounce re-crossed the boundary carrying speed");
        assert_eq!(rest_offset, Some(max), "the fling settled exactly at the edge it hit");
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

}
