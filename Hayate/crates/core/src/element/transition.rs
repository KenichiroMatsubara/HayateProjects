//! Pseudo-state transition interpolation (ADR-0089, issue #209).
//!
//! When a `:hover` / `:active` / `:focus` switch occurs on an element whose
//! resolved `transition-duration` is positive, the render layer interpolates
//! the element's continuous visual properties from their on-screen value
//! (`from`) toward the freshly-resolved target over the duration, eased by
//! `transition-timing`. Enum-valued and discrete properties are not
//! interpolated — they take the target value immediately. The interpolation is
//! advanced by `render(timestamp_ms)` and keeps the element visual-dirty until
//! it completes, reusing the existing dirty/frame-loop infrastructure rather
//! than introducing a separate timer.

use crate::color::Color;
use crate::element::style::TransitionTimingValue;
use crate::element::tree::Visual;

/// A single in-flight transition for one element.
#[derive(Clone, Debug)]
pub(crate) struct TransitionState {
    /// Visual snapshot displayed when the transition began (the curve's start).
    from: Visual,
    duration_ms: f32,
    timing: TransitionTimingValue,
    /// Host clock at which interpolation started. `None` until the first
    /// `render` after the trigger anchors the clock (CSS starts a transition on
    /// first observation, not at the input event).
    start_ms: Option<f64>,
    /// Eased progress in `[0, 1]` from the most recent `advance`.
    progress: f32,
}

impl TransitionState {
    pub(crate) fn new(from: Visual, duration_ms: f32, timing: TransitionTimingValue) -> Self {
        Self {
            from,
            duration_ms,
            timing,
            start_ms: None,
            progress: 0.0,
        }
    }

    /// Advance the clock to `now_ms`, returning `true` once the transition has
    /// reached its end (the caller drops finished transitions after a final
    /// frame that paints the target).
    pub(crate) fn advance(&mut self, now_ms: f64) -> bool {
        let start = *self.start_ms.get_or_insert(now_ms);
        let raw = if self.duration_ms > 0.0 {
            ((now_ms - start) as f32 / self.duration_ms).clamp(0.0, 1.0)
        } else {
            1.0
        };
        self.progress = ease(self.timing, raw);
        raw >= 1.0
    }

    /// Blend `from` toward `target` at the current eased progress.
    pub(crate) fn blend(&self, target: &Visual) -> Visual {
        lerp_visual(&self.from, target, self.progress)
    }
}

/// Interpolate the continuous visual properties; discrete props take `to`.
fn lerp_visual(from: &Visual, to: &Visual, t: f32) -> Visual {
    let mut out = to.clone();
    out.background_color = lerp_color_opt(from.background_color, to.background_color, t);
    out.border_color = lerp_color_opt(from.border_color, to.border_color, t);
    out.text_color = lerp_color_opt(from.text_color, to.text_color, t);
    out.opacity = lerp_f32(from.opacity, to.opacity, t);
    out.border_radius = lerp_f32(from.border_radius, to.border_radius, t);
    out.border_width = lerp_f32(from.border_width, to.border_width, t);
    out
}

fn lerp_f32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Interpolate two optional colours. When only one side is set there is no
/// continuous path between them, so snap to the target.
fn lerp_color_opt(a: Option<Color>, b: Option<Color>, t: f32) -> Option<Color> {
    match (a, b) {
        (Some(a), Some(b)) => {
            let t = t as f64;
            let lerp = |x: f64, y: f64| x + (y - x) * t;
            Some(Color::new(
                lerp(a.r, b.r),
                lerp(a.g, b.g),
                lerp(a.b, b.b),
                lerp(a.a, b.a),
            ))
        }
        _ => b,
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
