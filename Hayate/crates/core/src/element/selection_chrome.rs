//! Floating selection toolbar — core-drawn selection chrome (ADR-0097, #272).
//!
//! Decision 3 of ADR-0097: the selection chrome (highlight, handles, floating
//! toolbar) is drawn **once by core** into the SceneGraph and only its *style*
//! is theme-switchable; OS-native toolbar widgets are not re-implemented per
//! Platform Adapter. This module holds the style-agnostic toolbar **model** —
//! which actions appear, how the buttons are laid out, and which button a tap
//! lands on — plus the [`SelectionChromeStyle`] switch whose first member is the
//! Material flavor (Cupertino arrives with the iOS adapter, additively).

/// The chrome theme for selection highlight, handles and the floating toolbar.
/// Switchable so adding Cupertino (with the iOS Platform Adapter) is additive,
/// never a rewrite (ADR-0097, decision 3). Material is implemented first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SelectionChromeStyle {
    /// Material Design flavored chrome (the initial, default theme).
    #[default]
    Material,
    /// Cupertino (iOS) flavored chrome — added with the iOS Platform Adapter.
    Cupertino,
}

/// A button shown on the floating selection toolbar. The available set depends
/// on the selection: a read-only SelectionArea offers read actions (Copy /
/// Select All); an editable text-input adds the mutating ones (Cut / Paste).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ToolbarAction {
    Cut,
    Copy,
    Paste,
    SelectAll,
}

impl ToolbarAction {
    /// The button label drawn on the toolbar.
    pub fn label(self) -> &'static str {
        match self {
            ToolbarAction::Cut => "Cut",
            ToolbarAction::Copy => "Copy",
            ToolbarAction::Paste => "Paste",
            ToolbarAction::SelectAll => "Select All",
        }
    }
}

/// An axis-aligned rectangle in canvas coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToolbarRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl ToolbarRect {
    fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }
}

/// One tappable button on the floating toolbar, with its canvas-space rect.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToolbarButton {
    pub action: ToolbarAction,
    pub bounds: ToolbarRect,
}

/// The laid-out floating selection toolbar: the ordered buttons and the overall
/// bar rect, positioned over the selection. Built by [`layout`] and consumed by
/// both hit-testing (input) and scene emission (drawing).
#[derive(Clone, Debug, PartialEq)]
pub struct SelectionToolbar {
    pub style: SelectionChromeStyle,
    pub bounds: ToolbarRect,
    pub buttons: Vec<ToolbarButton>,
}

impl SelectionToolbar {
    /// The toolbar's actions in display order.
    pub fn actions(&self) -> Vec<ToolbarAction> {
        self.buttons.iter().map(|b| b.action).collect()
    }

    /// The action whose button contains `(x, y)`, or `None` for a tap that
    /// misses every button (the runtime then treats the press normally).
    pub fn action_at(&self, x: f32, y: f32) -> Option<ToolbarAction> {
        self.buttons
            .iter()
            .find(|b| b.bounds.contains(x, y))
            .map(|b| b.action)
    }
}

/// Which end of the range a drag handle controls. The `Start` handle adjusts the
/// document-earlier endpoint, `End` the later one (ADR-0097, #273).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionHandleEnd {
    Start,
    End,
}

/// One Material teardrop drag handle: a circular knob hanging just below the
/// selection's caret edge at one end, which the user drags to adjust that
/// endpoint (ADR-0097, #273). Style-agnostic geometry; the theme only colors it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelectionHandle {
    pub end: SelectionHandleEnd,
    /// Knob center in canvas coords — the circular grab target.
    pub knob_x: f32,
    pub knob_y: f32,
    /// Visible knob radius.
    pub radius: f32,
}

/// The pair of Material drag handles flanking the active selection (ADR-0097,
/// #273): one at each end of the range. Built by [`layout_handles`] and consumed
/// by both hit-testing (handle drag) and scene emission (drawing).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelectionHandles {
    pub style: SelectionChromeStyle,
    pub start: SelectionHandle,
    pub end: SelectionHandle,
}

impl SelectionHandles {
    /// The handle end whose knob `(x, y)` grabs, or `None` for a point clear of
    /// both. When both knobs are in reach (a very short selection) the nearer
    /// one wins, so the user can still target either end.
    pub fn handle_at(&self, x: f32, y: f32) -> Option<SelectionHandleEnd> {
        let d2 = |h: &SelectionHandle| {
            let dx = x - h.knob_x;
            let dy = y - h.knob_y;
            dx * dx + dy * dy
        };
        let reach = HANDLE_HIT_RADIUS * HANDLE_HIT_RADIUS;
        let candidates = [(d2(&self.start), self.start.end), (d2(&self.end), self.end.end)];
        candidates
            .into_iter()
            .filter(|&(dist, _)| dist <= reach)
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, end)| end)
    }
}

/// Visible radius of a Material selection handle's knob.
pub(crate) const HANDLE_RADIUS: f32 = 8.0;
/// Hit radius for grabbing a handle — larger than the knob so a finger can land
/// it (a Material handle's touch target is far bigger than its visible dot).
pub(crate) const HANDLE_HIT_RADIUS: f32 = 22.0;

/// Lay out the two Material drag handles from the caret edges at each end of the
/// selection (`(x, baseline_bottom_y)` in canvas coords). Each knob hangs one
/// radius below the text edge so the teardrop kisses the baseline (ADR-0097,
/// #273).
pub(crate) fn layout_handles(
    style: SelectionChromeStyle,
    start_caret: (f32, f32),
    end_caret: (f32, f32),
) -> SelectionHandles {
    let handle = |end: SelectionHandleEnd, (cx, cy): (f32, f32)| SelectionHandle {
        end,
        knob_x: cx,
        knob_y: cy + HANDLE_RADIUS,
        radius: HANDLE_RADIUS,
    };
    SelectionHandles {
        style,
        start: handle(SelectionHandleEnd::Start, start_caret),
        end: handle(SelectionHandleEnd::End, end_caret),
    }
}

impl SelectionChromeStyle {
    /// The fill color of a selection drag handle (RGBA, 0..1).
    pub(crate) fn handle_color(self) -> [f32; 4] {
        match self {
            // Material: the primary selection blue, matching the highlight.
            SelectionChromeStyle::Material => [0.20, 0.45, 0.95, 1.0],
            // Cupertino placeholder — refined with the iOS adapter (additive).
            SelectionChromeStyle::Cupertino => [0.0, 0.48, 1.0, 1.0],
        }
    }
}

/// Material toolbar metrics. A single core-drawn chrome whose values are
/// theme-switchable (ADR-0097); Material is the initial theme.
pub(crate) const TOOLBAR_HEIGHT: f32 = 40.0;
pub(crate) const TOOLBAR_LABEL_FONT_SIZE: f32 = 14.0;
pub(crate) const TOOLBAR_CORNER_RADIUS: f32 = 4.0;

impl SelectionChromeStyle {
    /// The toolbar panel background color (premultiplied-free RGBA, 0..1).
    pub(crate) fn toolbar_background(self) -> [f32; 4] {
        match self {
            // Material: a near-opaque dark surface.
            SelectionChromeStyle::Material => [0.20, 0.20, 0.22, 0.98],
            // Cupertino placeholder — refined with the iOS adapter (additive).
            SelectionChromeStyle::Cupertino => [0.18, 0.18, 0.18, 0.96],
        }
    }

    /// The toolbar label text color (RGBA, 0..1).
    pub(crate) fn toolbar_label(self) -> [f32; 4] {
        match self {
            SelectionChromeStyle::Material => [0.98, 0.98, 0.98, 1.0],
            SelectionChromeStyle::Cupertino => [1.0, 1.0, 1.0, 1.0],
        }
    }
}
/// Approximate horizontal advance per label character. Core draws the labels
/// itself, so this estimate is self-consistent between layout and rendering.
const LABEL_CHAR_ADVANCE: f32 = 8.0;
/// Horizontal padding on each side of a button's label.
const BUTTON_PAD_X: f32 = 12.0;
/// Vertical gap between the toolbar and the selection it floats over.
const TOOLBAR_GAP: f32 = 8.0;

fn button_width(action: ToolbarAction) -> f32 {
    action.label().chars().count() as f32 * LABEL_CHAR_ADVANCE + 2.0 * BUTTON_PAD_X
}

/// Lay the toolbar out over a selection bounding box `sel` (canvas coords),
/// centered horizontally and floating just above the selection — flipping below
/// when there is no room above the top viewport edge. The bar is clamped to stay
/// within the `viewport` horizontally. Returns `None` when `actions` is empty.
pub(crate) fn layout(
    style: SelectionChromeStyle,
    actions: &[ToolbarAction],
    sel: ToolbarRect,
    viewport: (f32, f32),
) -> Option<SelectionToolbar> {
    if actions.is_empty() {
        return None;
    }
    let total_width: f32 = actions.iter().map(|&a| button_width(a)).sum();

    // Centered over the selection, then clamped into the viewport horizontally.
    let center_x = sel.x + sel.width / 2.0;
    let max_x = (viewport.0 - total_width).max(0.0);
    let x = (center_x - total_width / 2.0).clamp(0.0, max_x);

    // Prefer floating above the selection; flip below when it would clip the top.
    let above_y = sel.y - TOOLBAR_GAP - TOOLBAR_HEIGHT;
    let y = if above_y >= 0.0 {
        above_y
    } else {
        sel.y + sel.height + TOOLBAR_GAP
    };

    let mut buttons = Vec::with_capacity(actions.len());
    let mut bx = x;
    for &action in actions {
        let w = button_width(action);
        buttons.push(ToolbarButton {
            action,
            bounds: ToolbarRect {
                x: bx,
                y,
                width: w,
                height: TOOLBAR_HEIGHT,
            },
        });
        bx += w;
    }

    Some(SelectionToolbar {
        style,
        bounds: ToolbarRect {
            x,
            y,
            width: total_width,
            height: TOOLBAR_HEIGHT,
        },
        buttons,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sel(x: f32, y: f32, w: f32, h: f32) -> ToolbarRect {
        ToolbarRect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    #[test]
    fn buttons_are_laid_out_left_to_right_in_action_order() {
        let actions = [ToolbarAction::Copy, ToolbarAction::SelectAll];
        let tb = layout(SelectionChromeStyle::Material, &actions, sel(100.0, 80.0, 60.0, 20.0), (400.0, 200.0))
            .expect("non-empty actions produce a toolbar");
        assert_eq!(tb.actions(), actions.to_vec());
        // Each button sits immediately right of the previous one, no overlap.
        let a = tb.buttons[0].bounds;
        let b = tb.buttons[1].bounds;
        assert_eq!(b.x, a.x + a.width);
    }

    #[test]
    fn toolbar_floats_above_the_selection_with_a_gap() {
        let tb = layout(
            SelectionChromeStyle::Material,
            &[ToolbarAction::Copy],
            sel(100.0, 80.0, 60.0, 20.0),
            (400.0, 200.0),
        )
        .unwrap();
        assert_eq!(tb.bounds.y, 80.0 - TOOLBAR_GAP - TOOLBAR_HEIGHT);
    }

    #[test]
    fn toolbar_flips_below_when_there_is_no_room_above() {
        // Selection hugging the top edge: above would be negative, so flip below.
        let tb = layout(
            SelectionChromeStyle::Material,
            &[ToolbarAction::Copy],
            sel(100.0, 2.0, 60.0, 20.0),
            (400.0, 200.0),
        )
        .unwrap();
        assert_eq!(tb.bounds.y, 2.0 + 20.0 + TOOLBAR_GAP);
    }

    #[test]
    fn toolbar_is_clamped_within_the_viewport_horizontally() {
        // Selection near the right edge: the bar must not overflow the viewport.
        let tb = layout(
            SelectionChromeStyle::Material,
            &[ToolbarAction::Cut, ToolbarAction::Copy, ToolbarAction::Paste, ToolbarAction::SelectAll],
            sel(390.0, 80.0, 8.0, 20.0),
            (400.0, 200.0),
        )
        .unwrap();
        assert!(tb.bounds.x >= 0.0);
        assert!(tb.bounds.x + tb.bounds.width <= 400.0 + 0.01);
    }

    #[test]
    fn action_at_hits_the_button_under_the_point() {
        let tb = layout(
            SelectionChromeStyle::Material,
            &[ToolbarAction::Copy, ToolbarAction::SelectAll],
            sel(100.0, 80.0, 60.0, 20.0),
            (400.0, 200.0),
        )
        .unwrap();
        let copy = tb.buttons[0].bounds;
        assert_eq!(
            tb.action_at(copy.x + 1.0, copy.y + 1.0),
            Some(ToolbarAction::Copy),
        );
        // A point above the bar hits nothing.
        assert_eq!(tb.action_at(copy.x + 1.0, copy.y - 5.0), None);
    }

    #[test]
    fn empty_actions_produce_no_toolbar() {
        assert!(layout(SelectionChromeStyle::Material, &[], sel(0.0, 0.0, 0.0, 0.0), (400.0, 200.0)).is_none());
    }

    #[test]
    fn handles_hang_below_both_selection_ends() {
        // Caret edges at the two ends of a one-line range share a baseline; the
        // teardrop knobs hang just below it, anchored at each end's x.
        let h = layout_handles(SelectionChromeStyle::Material, (10.0, 20.0), (80.0, 20.0));
        assert_eq!(h.start.end, SelectionHandleEnd::Start);
        assert_eq!(h.end.end, SelectionHandleEnd::End);
        assert_eq!(h.start.knob_x, 10.0);
        assert_eq!(h.end.knob_x, 80.0);
        assert!(h.start.knob_y > 20.0, "knob hangs below the text edge");
        assert_eq!(h.start.knob_y, h.end.knob_y);
    }

    #[test]
    fn handle_at_picks_the_end_under_the_point() {
        let h = layout_handles(SelectionChromeStyle::Material, (10.0, 20.0), (80.0, 20.0));
        assert_eq!(
            h.handle_at(h.start.knob_x, h.start.knob_y),
            Some(SelectionHandleEnd::Start),
        );
        assert_eq!(
            h.handle_at(h.end.knob_x, h.end.knob_y),
            Some(SelectionHandleEnd::End),
        );
        // A point far from both knobs grabs neither.
        assert_eq!(h.handle_at(45.0, 400.0), None);
    }
}

