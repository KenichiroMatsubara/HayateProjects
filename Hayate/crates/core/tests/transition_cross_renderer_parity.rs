//! Issue #229: cross-renderer parity (Semantics Parity, ADR-0002) for the
//! up-levelled transition trigger (#227 / ADR-0093). The Canvas Render Layer now
//! interpolates `setStyle`- and inheritance-driven changes, so its on-screen
//! frames must match what a browser CSS transition (the DOM path) shows at the
//! same instant. This regression fixes that parity.
//!
//! The Canvas side is the real `ElementTree`: `render(timestamp_ms)` advances the
//! retained interpolation and `draw_ops` reads the painted (post-blend) colour.
//! The DOM side is an *independent* reference simulator of a browser CSS
//! transition — linear interpolation from the on-screen value toward the resolved
//! target over the **after-change** `transition-duration`. Both are driven by the
//! same Hayate CSS input and sampled at identical timestamps; only the external
//! draw result is compared (ADR-0079), never either renderer's internals.

use hayate_core::{
    Color, Dimension, DrawOp, ElementKind, ElementTree, PseudoState, RecordingPainter, StyleProp,
    TransitionTimingValue, render_scene_graph,
};

/// Tolerance for matching two independently-computed colour channels.
const PARITY_EPS: f32 = 1e-3;

// ---------------------------------------------------------------------------
// Canvas side: the real retained Render Layer, read through its draw output.
// ---------------------------------------------------------------------------

/// Background colour painted by the first filled rect in the current scene.
fn canvas_background(tree: &ElementTree) -> [f32; 4] {
    let sg = tree.scene_graph();
    let mut painter = RecordingPainter::new();
    render_scene_graph(sg, &mut painter);
    for op in painter.into_ops() {
        if let DrawOp::FillRect { color, .. } = op {
            return color;
        }
    }
    panic!("no FillRect in scene");
}

// ---------------------------------------------------------------------------
// DOM side: an independent reference model of a browser CSS transition.
//
// A browser starts a transition when the computed value of an animatable
// property changes: it interpolates linearly from the value shown on screen
// (`from`) toward the new computed value (`target`) over the property's
// *after-change* `transition-duration`, eased by the timing function. The clock
// is anchored at the first frame after the change. duration 0 ⇒ no transition,
// the new value shows immediately. This mirrors Blink without sharing any code
// with the Canvas Render Layer, so a drift in the Canvas implementation diverges.
// ---------------------------------------------------------------------------

struct DomTransition {
    from: Color,
    target: Color,
    duration_ms: f32,
    start_ms: Option<f64>,
}

/// A single animatable property as a browser would transition it.
struct DomProperty {
    displayed: Color,
    active: Option<DomTransition>,
}

impl DomProperty {
    fn new(initial: Color) -> Self {
        Self {
            displayed: initial,
            active: None,
        }
    }

    /// Apply a computed-style change. `duration_ms` is the after-change resolved
    /// `transition-duration` (a browser reads the value that is in effect *after*
    /// the state/style change, e.g. the base value when leaving a `:hover` that
    /// set `transition-duration: 0`).
    fn set_target(&mut self, target: Color, duration_ms: f32) {
        if colors_eq(target, self.displayed) {
            return;
        }
        if duration_ms <= 0.0 {
            // No transition: the new computed value is shown immediately.
            self.displayed = target;
            self.active = None;
            return;
        }
        self.active = Some(DomTransition {
            from: self.displayed,
            target,
            duration_ms,
            start_ms: None,
        });
    }

    /// Advance to `now_ms`, anchoring the clock on the first frame after a change.
    fn render(&mut self, now_ms: f64) -> Color {
        if let Some(tr) = self.active.as_mut() {
            let start = *tr.start_ms.get_or_insert(now_ms);
            let progress = (((now_ms - start) as f32) / tr.duration_ms).clamp(0.0, 1.0);
            self.displayed = lerp_color(tr.from, tr.target, progress);
            if progress >= 1.0 {
                self.active = None;
            }
        }
        self.displayed
    }
}

fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let t = t as f64;
    let l = |x: f64, y: f64| x + (y - x) * t;
    Color::new(l(a.r, b.r), l(a.g, b.g), l(a.b, b.b), l(a.a, b.a))
}

fn colors_eq(a: Color, b: Color) -> bool {
    (a.r - b.r).abs() < 1e-9
        && (a.g - b.g).abs() < 1e-9
        && (a.b - b.b).abs() < 1e-9
        && (a.a - b.a).abs() < 1e-9
}

fn assert_parity(label: &str, canvas: [f32; 4], dom: Color) {
    let dom = [dom.r as f32, dom.g as f32, dom.b as f32, dom.a as f32];
    for (i, channel) in ["r", "g", "b", "a"].iter().enumerate() {
        assert!(
            (canvas[i] - dom[i]).abs() < PARITY_EPS,
            "{label}: {channel} channel diverged — canvas {} vs dom {}",
            canvas[i],
            dom[i],
        );
    }
}

// ---------------------------------------------------------------------------
// Shared scene builder.
// ---------------------------------------------------------------------------

const RED: Color = Color {
    r: 1.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};
const GREEN: Color = Color {
    r: 0.0,
    g: 1.0,
    b: 0.0,
    a: 1.0,
};
const BLUE: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 1.0,
    a: 1.0,
};

/// A red box whose `transition-duration` is `duration_ms` with **linear** timing
/// (so the browser-reference interpolation is unambiguous and code-independent).
fn linear_box(duration_ms: f32) -> (ElementTree, hayate_core::ElementId) {
    let mut tree = ElementTree::new();
    let root = tree.element_create(1, ElementKind::View);
    tree.set_root(root);
    tree.set_viewport(200.0, 200.0);
    tree.element_set_style(
        root,
        &[
            StyleProp::Width(Dimension::px(100.0)),
            StyleProp::Height(Dimension::px(50.0)),
            StyleProp::BackgroundColor(RED),
            StyleProp::TransitionDuration(duration_ms),
            StyleProp::TransitionTiming(TransitionTimingValue::Linear),
        ],
    );
    (tree, root)
}

/// A linear box that turns green on `:hover`. When `hover_duration_ms` is set it
/// also overrides `transition-duration` while hovered (the `:hover { … 0 }`
/// asymmetry case): hover-in reads the override, hover-out reads the base.
fn linear_hover_box(
    base_duration_ms: f32,
    hover_duration_ms: Option<f32>,
) -> (ElementTree, hayate_core::ElementId) {
    let (mut tree, root) = linear_box(base_duration_ms);
    let mut hover = vec![StyleProp::BackgroundColor(GREEN)];
    if let Some(d) = hover_duration_ms {
        hover.push(StyleProp::TransitionDuration(d));
    }
    tree.element_set_pseudo_style(root, PseudoState::Hover, &hover);
    (tree, root)
}

// ---------------------------------------------------------------------------
// AC1: a `setStyle` continuous-property change interpolates identically on
// Canvas and DOM at every sampled instant.
// ---------------------------------------------------------------------------

#[test]
fn set_style_change_has_canvas_dom_parity() {
    let (mut tree, root) = linear_box(200.0);
    let mut dom = DomProperty::new(RED);

    tree.render(0.0);
    assert_parity("initial", canvas_background(&tree), dom.render(0.0));

    // Same input on both sides: change background to blue via setStyle.
    tree.element_set_style(root, &[StyleProp::BackgroundColor(BLUE)]);
    dom.set_target(BLUE, 200.0);

    // Anchor frame (clock starts here): still red on both.
    let anchor = canvas_background(&tree);
    tree.render(100.0);
    assert_parity("anchor", canvas_background(&tree), dom.render(100.0));
    assert_eq!(anchor[0], 1.0, "anchor frame still red before advancing");

    // Quarter, half and three-quarter points all agree.
    tree.render(150.0);
    assert_parity("t=150", canvas_background(&tree), dom.render(150.0));
    tree.render(200.0);
    assert_parity("t=200", canvas_background(&tree), dom.render(200.0));
    tree.render(250.0);
    assert_parity("t=250", canvas_background(&tree), dom.render(250.0));

    // Past the window: both settle exactly on blue.
    tree.render(300.0);
    let end = canvas_background(&tree);
    assert_parity("settled", end, dom.render(300.0));
    assert!(end[2] > 0.999 && end[0].abs() < PARITY_EPS, "settles on blue: {end:?}");
}

// ---------------------------------------------------------------------------
// AC2: a reverse interrupt (un-hover mid-flight) continues from the on-screen
// value on both renderers — neither jumps to the resolved target.
// ---------------------------------------------------------------------------

#[test]
fn reverse_interrupt_has_canvas_dom_parity() {
    let (mut tree, root) = linear_hover_box(200.0, None);
    let mut dom = DomProperty::new(RED);

    tree.render(0.0);
    assert_parity("initial", canvas_background(&tree), dom.render(0.0));

    // Hover-in: animate red → green.
    tree.update_pointer_hover(Some(root));
    dom.set_target(GREEN, 200.0);
    tree.render(100.0); // anchor
    assert_parity("anchor", canvas_background(&tree), dom.render(100.0));
    tree.render(200.0); // halfway
    let mid = canvas_background(&tree);
    assert_parity("midway", mid, dom.render(200.0));
    assert!(mid[0] > 0.0 && mid[1] > 0.0, "captured a mid value: {mid:?}");

    // Reverse at the same instant: target flips back to red. Both must hold the
    // displayed mid value (continuous reversal), not jump to red or green.
    tree.update_pointer_hover(None);
    dom.set_target(RED, 200.0);
    tree.render(200.0);
    let reversed = canvas_background(&tree);
    assert_parity("reverse-instant", reversed, dom.render(200.0));
    assert!(
        (reversed[0] - mid[0]).abs() < 1e-2 && (reversed[1] - mid[1]).abs() < 1e-2,
        "reversal is continuous, not a jump: {mid:?} -> {reversed:?}"
    );

    // Continuing the reverse heads back toward red, in lockstep.
    tree.render(300.0);
    let back = canvas_background(&tree);
    assert_parity("reverse-midway", back, dom.render(300.0));
    assert!(back[0] > reversed[0], "red channel climbs back: {} -> {}", reversed[0], back[0]);

    tree.render(400.0);
    let done = canvas_background(&tree);
    assert_parity("reverse-settled", done, dom.render(400.0));
    assert!((done[0] - 1.0).abs() < PARITY_EPS, "settles back on red: {done:?}");
}

// ---------------------------------------------------------------------------
// AC3: `:hover { transition-duration: 0 }` over a base duration makes hover-in
// instant and hover-out animated. Because both renderers read duration from the
// *after-change* resolved value, that in/out asymmetry is identical.
// ---------------------------------------------------------------------------

#[test]
fn hover_duration_zero_asymmetry_has_canvas_dom_parity() {
    let (mut tree, root) = linear_hover_box(500.0, Some(0.0));
    let mut dom = DomProperty::new(RED);

    tree.render(0.0);
    assert_parity("initial", canvas_background(&tree), dom.render(0.0));

    // Hover-in: after-change duration is 0 → instant green on both, no animation.
    tree.update_pointer_hover(Some(root));
    dom.set_target(GREEN, 0.0);
    tree.render(100.0);
    let hovered = canvas_background(&tree);
    assert_parity("hover-in", hovered, dom.render(100.0));
    assert!(
        (hovered[1] - 1.0).abs() < PARITY_EPS && hovered[0].abs() < PARITY_EPS,
        "hover-in is instant green: {hovered:?}"
    );

    // Hover-out: after-change duration is the base 500ms → both animate green→red.
    tree.update_pointer_hover(None);
    dom.set_target(RED, 500.0);
    tree.render(200.0); // anchor
    assert_parity("hover-out-anchor", canvas_background(&tree), dom.render(200.0));
    tree.render(450.0); // 250ms into the 500ms window
    let mid = canvas_background(&tree);
    assert_parity("hover-out-midway", mid, dom.render(450.0));
    assert!(
        mid[0] > 0.0 && mid[0] < 1.0 && mid[1] > 0.0 && mid[1] < 1.0,
        "hover-out animates over 500ms: {mid:?}"
    );

    tree.render(700.0);
    let done = canvas_background(&tree);
    assert_parity("hover-out-settled", done, dom.render(700.0));
    assert!((done[0] - 1.0).abs() < PARITY_EPS, "settles back on red: {done:?}");
}
