//! Effective-visual transition interpolation (ADR-0089 / ADR-0093, issue #227).
//!
//! When an element's resolved effective visual (ADR-0067) changes a continuous
//! property and the after-change `transition-duration` is positive, the render
//! layer interpolates that property from its on-screen value (`from`) toward the
//! freshly-resolved target over the duration, eased by `transition-timing`. The
//! trigger is the per-property diff at the `resolve_effective` seam, so pseudo
//! switches, `setStyle`, and inherited changes are treated alike (Blink's
//! computed-style diff). Enum-valued and discrete properties are not
//! interpolated — they take the target value immediately. State is kept per
//! element × property so several properties interpolate from independent `from`
//! values and anchor their own start time. Interpolation is advanced by
//! `render(timestamp_ms)` and keeps the element visual-dirty until it completes,
//! reusing the existing dirty/frame-loop infrastructure (ADR-0086/0032) rather
//! than introducing a separate timer.

use crate::color::Color;
use crate::element::style::{Shadow, TransitionTimingValue};
use crate::element::tree::Visual;

/// A continuous value that can be linearly interpolated during a transition.
pub(crate) trait Lerp: Clone + PartialEq {
    fn lerp(&self, to: &Self, t: f32) -> Self;
}

impl Lerp for f32 {
    fn lerp(&self, to: &Self, t: f32) -> Self {
        self + (to - self) * t
    }
}

fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let t = t as f64;
    let lerp = |x: f64, y: f64| x + (y - x) * t;
    Color::new(lerp(a.r, b.r), lerp(a.g, b.g), lerp(a.b, b.b), lerp(a.a, b.a))
}

impl Lerp for Option<Color> {
    /// When only one side is set there is no continuous path between them, so
    /// snap straight to the target.
    fn lerp(&self, to: &Self, t: f32) -> Self {
        match (self, to) {
            (Some(a), Some(b)) => Some(lerp_color(*a, *b, t)),
            _ => *to,
        }
    }
}

impl Lerp for Vec<Shadow> {
    /// Box-shadow interpolation is CSS-conformant (ADR-0095): only when the
    /// before/after lists have equal length and matching `inset` flags at every
    /// position do we interpolate each layer's offset/blur/spread/color; any
    /// mismatch is discrete (the target is adopted immediately).
    fn lerp(&self, to: &Self, t: f32) -> Self {
        if self.len() != to.len() || self.iter().zip(to).any(|(a, b)| a.inset != b.inset) {
            return to.clone();
        }
        self.iter()
            .zip(to)
            .map(|(a, b)| Shadow {
                offset_x: a.offset_x.lerp(&b.offset_x, t),
                offset_y: a.offset_y.lerp(&b.offset_y, t),
                blur: a.blur.lerp(&b.blur, t),
                spread: a.spread.lerp(&b.spread, t),
                color: lerp_color(a.color, b.color, t),
                inset: a.inset,
            })
            .collect()
    }
}

/// One in-flight transition for a single continuous property.
#[derive(Clone, Debug)]
struct Track<T> {
    /// Value displayed when this curve began (its start).
    from: T,
    /// Resolved value the curve runs toward.
    target: T,
    duration_ms: f32,
    timing: TransitionTimingValue,
    /// Host clock at which interpolation started. `None` until the first
    /// `advance` after the trigger anchors the clock (CSS starts a transition on
    /// first observation, not at the triggering mutation).
    start_ms: Option<f64>,
    /// Eased progress in `[0, 1]` from the most recent `advance`.
    progress: f32,
}

impl<T: Lerp> Track<T> {
    fn new(from: T, target: T, duration_ms: f32, timing: TransitionTimingValue) -> Self {
        Self {
            from,
            target,
            duration_ms,
            timing,
            start_ms: None,
            progress: 0.0,
        }
    }

    /// Advance the clock to `now_ms`, returning `true` once the curve has
    /// reached its end (the caller drops finished tracks after the final frame
    /// that paints the target).
    fn advance(&mut self, now_ms: f64) -> bool {
        let start = *self.start_ms.get_or_insert(now_ms);
        let raw = if self.duration_ms > 0.0 {
            ((now_ms - start) as f32 / self.duration_ms).clamp(0.0, 1.0)
        } else {
            1.0
        };
        self.progress = ease(self.timing, raw);
        raw >= 1.0
    }

    /// The currently displayed value (`from` blended toward `target`).
    fn current(&self) -> T {
        self.from.lerp(&self.target, self.progress)
    }
}

/// Step one property's `track` toward `target`, returning the value to display.
///
/// `prev_displayed` is last frame's on-screen value for this property (`None` on
/// the element's first emit — initial styles never transition). A change of
/// `target` redirects continuously from the current displayed value, so a
/// reverse interrupt never jumps. `duration_ms` / `timing` are read from the
/// after-change resolved visual.
fn step<T: Lerp>(
    track: &mut Option<Track<T>>,
    prev_displayed: Option<T>,
    target: T,
    duration_ms: f32,
    timing: TransitionTimingValue,
    now_ms: f64,
) -> T {
    let target_changed = match track {
        Some(tr) => tr.target != target,
        None => prev_displayed.as_ref().is_some_and(|p| *p != target),
    };
    if target_changed {
        if duration_ms > 0.0 {
            let from = match track {
                Some(tr) => tr.current(),
                None => prev_displayed.expect("target_changed implies a previous value"),
            };
            *track = Some(Track::new(from, target.clone(), duration_ms, timing));
        } else {
            // After-change duration is zero: snap immediately (CSS/DOM parity).
            *track = None;
        }
    }
    match track {
        Some(tr) => {
            let done = tr.advance(now_ms);
            let cur = tr.current();
            if done {
                *track = None;
            }
            cur
        }
        None => target,
    }
}

/// Per-property transition state for one element (ADR-0093). Each continuous
/// property interpolates independently from its own `from` and start time.
#[derive(Clone, Debug, Default)]
pub(crate) struct ElementTransitions {
    background_color: Option<Track<Option<Color>>>,
    border_color: Option<Track<Option<Color>>>,
    text_color: Option<Track<Option<Color>>>,
    opacity: Option<Track<f32>>,
    border_radius: Option<Track<f32>>,
    border_width: Option<Track<f32>>,
    box_shadow: Option<Track<Vec<Shadow>>>,
}

impl ElementTransitions {
    /// Whether any property is still interpolating.
    pub(crate) fn is_active(&self) -> bool {
        self.background_color.is_some()
            || self.border_color.is_some()
            || self.text_color.is_some()
            || self.opacity.is_some()
            || self.border_radius.is_some()
            || self.border_width.is_some()
            || self.box_shadow.is_some()
    }

    /// Diff the after-change resolved `target` against the previous frame's
    /// displayed visual, (re)starting per-property transitions where it differs,
    /// and return the visual to display this frame. Discrete / enum properties
    /// take the target immediately. duration / timing come from `target` (the
    /// after-change resolved effective visual).
    pub(crate) fn blend(
        &mut self,
        prev_displayed: Option<&Visual>,
        target: &Visual,
        now_ms: f64,
    ) -> Visual {
        let dur = target.transition_duration;
        let timing = target.transition_timing;
        let mut out = target.clone();
        out.background_color = step(
            &mut self.background_color,
            prev_displayed.map(|v| v.background_color),
            target.background_color,
            dur,
            timing,
            now_ms,
        );
        out.border_color = step(
            &mut self.border_color,
            prev_displayed.map(|v| v.border_color),
            target.border_color,
            dur,
            timing,
            now_ms,
        );
        out.text_color = step(
            &mut self.text_color,
            prev_displayed.map(|v| v.text_color),
            target.text_color,
            dur,
            timing,
            now_ms,
        );
        out.opacity = step(
            &mut self.opacity,
            prev_displayed.map(|v| v.opacity),
            target.opacity,
            dur,
            timing,
            now_ms,
        );
        out.border_radius = step(
            &mut self.border_radius,
            prev_displayed.map(|v| v.border_radius),
            target.border_radius,
            dur,
            timing,
            now_ms,
        );
        out.border_width = step(
            &mut self.border_width,
            prev_displayed.map(|v| v.border_width),
            target.border_width,
            dur,
            timing,
            now_ms,
        );
        out.box_shadow = step(
            &mut self.box_shadow,
            prev_displayed.map(|v| v.box_shadow.clone()),
            target.box_shadow.clone(),
            dur,
            timing,
            now_ms,
        );
        out
    }
}

/// Map a linear time fraction `t` through the easing curve.
pub(crate) fn ease(timing: TransitionTimingValue, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    match timing {
        TransitionTimingValue::Linear => t,
        // Control points match the CSS keyword cubic-bezier definitions.
        TransitionTimingValue::Ease => cubic_bezier_ease(0.25, 0.1, 0.25, 1.0, t),
        TransitionTimingValue::EaseIn => cubic_bezier_ease(0.42, 0.0, 1.0, 1.0, t),
        TransitionTimingValue::EaseOut => cubic_bezier_ease(0.0, 0.0, 0.58, 1.0, t),
        TransitionTimingValue::EaseInOut => cubic_bezier_ease(0.42, 0.0, 0.58, 1.0, t),
    }
}

/// Evaluate a CSS timing cubic-bezier `(0,0) (p1x,p1y) (p2x,p2y) (1,1)` at time
/// fraction `x`: solve `Bx(s) = x` for the curve parameter `s`, then return
/// `By(s)`. Newton–Raphson with a bisection fallback (the standard approach).
fn cubic_bezier_ease(p1x: f32, p1y: f32, p2x: f32, p2y: f32, x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let bezier = |a: f32, b: f32, s: f32| {
        // (1-s)^3*0 + 3(1-s)^2 s a + 3(1-s) s^2 b + s^3*1
        let u = 1.0 - s;
        3.0 * u * u * s * a + 3.0 * u * s * s * b + s * s * s
    };
    let bezier_dx = |a: f32, b: f32, s: f32| {
        let u = 1.0 - s;
        3.0 * u * u * a + 6.0 * u * s * (b - a) + 3.0 * s * s * (1.0 - b)
    };

    let mut s = x; // initial guess
    for _ in 0..8 {
        let x_est = bezier(p1x, p2x, s) - x;
        if x_est.abs() < 1e-5 {
            return bezier(p1y, p2y, s);
        }
        let dx = bezier_dx(p1x, p2x, s);
        if dx.abs() < 1e-6 {
            break;
        }
        s -= x_est / dx;
    }

    // Bisection fallback for ill-conditioned derivatives.
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    s = x;
    for _ in 0..20 {
        let x_est = bezier(p1x, p2x, s);
        if (x_est - x).abs() < 1e-5 {
            break;
        }
        if x_est < x {
            lo = s;
        } else {
            hi = s;
        }
        s = (lo + hi) * 0.5;
    }
    bezier(p1y, p2y, s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_is_identity() {
        assert_eq!(ease(TransitionTimingValue::Linear, 0.5), 0.5);
    }

    fn shadow(offset: f32, blur: f32, spread: f32, alpha: f64, inset: bool) -> Shadow {
        Shadow {
            offset_x: offset,
            offset_y: offset,
            blur,
            spread,
            color: Color::new(0.0, 0.0, 0.0, alpha),
            inset,
        }
    }

    #[test]
    fn box_shadow_interpolates_per_layer_when_length_and_inset_match() {
        let from = vec![shadow(0.0, 0.0, 0.0, 0.0, false)];
        let to = vec![shadow(10.0, 20.0, 4.0, 1.0, false)];
        let mid = from.lerp(&to, 0.5);
        assert_eq!(mid.len(), 1);
        assert!((mid[0].offset_x - 5.0).abs() < 1e-4);
        assert!((mid[0].blur - 10.0).abs() < 1e-4);
        assert!((mid[0].spread - 2.0).abs() < 1e-4);
        assert!((mid[0].color.a - 0.5).abs() < 1e-4);
        assert!(!mid[0].inset);
    }

    #[test]
    fn box_shadow_is_discrete_on_length_mismatch() {
        let from = vec![shadow(0.0, 0.0, 0.0, 1.0, false)];
        let to = vec![
            shadow(10.0, 4.0, 0.0, 1.0, false),
            shadow(2.0, 1.0, 0.0, 1.0, false),
        ];
        // Mid-transition still snaps straight to the target list.
        assert_eq!(from.lerp(&to, 0.5), to);
    }

    #[test]
    fn box_shadow_is_discrete_on_inset_mismatch() {
        let from = vec![shadow(0.0, 0.0, 0.0, 1.0, false)];
        let to = vec![shadow(10.0, 4.0, 0.0, 1.0, true)];
        assert_eq!(from.lerp(&to, 0.5), to);
    }

    #[test]
    fn eases_pin_endpoints_and_stay_monotonic() {
        for timing in [
            TransitionTimingValue::Ease,
            TransitionTimingValue::EaseIn,
            TransitionTimingValue::EaseOut,
            TransitionTimingValue::EaseInOut,
        ] {
            assert!(ease(timing, 0.0).abs() < 1e-4);
            assert!((ease(timing, 1.0) - 1.0).abs() < 1e-4);
            let mid = ease(timing, 0.5);
            assert!(mid > 0.0 && mid < 1.0, "mid out of range for {timing:?}: {mid}");
        }
    }
}
