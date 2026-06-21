//! Runtime-overridable copy of the Canvas-Mode "chrome" taste constants
//! (scrollbar overlay, touch indicator, selection highlight/handle/toolbar,
//! composition underlines, placeholder alpha).
//!
//! The named `const`s in [`scene_build`](crate::element::scene_build) and
//! [`selection_chrome`](crate::element::selection_chrome) remain the
//! authoritative defaults — [`Default`] reads them, so the numbers are never
//! restated here. A dev build may overlay values at runtime (a `tuning.json`
//! parsed by the Platform Adapter, which owns serde) so a human can calibrate
//! against Chromium/Android by editing JSON and pressing F5, with no recompile.
//! Production ships with no override, so every field equals its const and each
//! read is a plain struct-field load off the live [`ElementTree`](crate::element::tree::ElementTree)
//! (no perf cost over the old `const` reference).
//!
//! Scope note (v1): only paint-time *visual* values are overridable. Layout /
//! hit-test geometry (handle hit radius, toolbar gap, label advance) and the
//! indicator fade *timing* stay on their consts — they are read by functions
//! that do not receive the tree — and still require a recompile to change.

use crate::element::scene_build;
use crate::element::selection_chrome;
use crate::Color;

/// Live, overridable chrome constants. See the module docs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChromeTuning {
    // ── Scrollbar overlay (Mouse/Pen), ADR-0110 ──
    pub scrollbar_thickness: f32,
    pub scrollbar_track_margin: f32,
    pub scrollbar_min_thumb_length: f32,
    pub scrollbar_thumb_color: Color,
    pub scrollbar_thumb_opacity: f32,
    // ── Touch transient indicator ──
    pub scrollbar_indicator_thickness: f32,
    pub scrollbar_indicator_color: Color,
    pub scrollbar_indicator_opacity: f32,
    // ── Selection highlight tint (Chromium `::selection`), ADR-0097 ──
    pub selection_highlight_color: [f32; 4],
    // ── Composition (IME) underlines, ADR-0102 ──
    pub composition_underline_thin: f32,
    pub composition_underline_thick: f32,
    // ── Placeholder text muting, ADR-0102 ──
    pub placeholder_alpha: f64,
    // ── Floating selection toolbar (Android-native), ADR-0097 ──
    //
    // Only the non-themed panel corner radius is a tuning knob. The toolbar /
    // handle *colours* and the toolbar height / label font size are owned by the
    // switchable `SelectionChromeStyle` theme (Material vs Cupertino) or by the
    // selection layout pass, neither of which this override touches — they stay
    // on their consts (recompile to change). See the module-level scope note.
    pub toolbar_corner_radius: f32,
}

impl Default for ChromeTuning {
    fn default() -> Self {
        // Mirror the authoritative consts — do not restate the literals, so the
        // const blocks stay the single source of the default values.
        Self {
            scrollbar_thickness: scene_build::SCROLLBAR_THICKNESS,
            scrollbar_track_margin: scene_build::SCROLLBAR_TRACK_MARGIN,
            scrollbar_min_thumb_length: scene_build::SCROLLBAR_MIN_THUMB_LENGTH,
            scrollbar_thumb_color: scene_build::SCROLLBAR_THUMB_COLOR,
            scrollbar_thumb_opacity: scene_build::SCROLLBAR_THUMB_OPACITY,
            scrollbar_indicator_thickness: scene_build::SCROLLBAR_INDICATOR_THICKNESS,
            scrollbar_indicator_color: scene_build::SCROLLBAR_INDICATOR_COLOR,
            scrollbar_indicator_opacity: scene_build::SCROLLBAR_INDICATOR_OPACITY,
            selection_highlight_color: scene_build::SELECTION_HIGHLIGHT_COLOR,
            composition_underline_thin: scene_build::COMPOSITION_UNDERLINE_THIN,
            composition_underline_thick: scene_build::COMPOSITION_UNDERLINE_THICK,
            placeholder_alpha: scene_build::PLACEHOLDER_ALPHA,
            toolbar_corner_radius: selection_chrome::TOOLBAR_CORNER_RADIUS,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_mirrors_the_authoritative_consts() {
        // Locks the "Default reflects the consts" invariant so a future const
        // edit that forgets this struct is caught at test time.
        let d = ChromeTuning::default();
        assert_eq!(d.scrollbar_thickness, scene_build::SCROLLBAR_THICKNESS);
        assert_eq!(d.scrollbar_thumb_color, scene_build::SCROLLBAR_THUMB_COLOR);
        assert_eq!(d.selection_highlight_color, scene_build::SELECTION_HIGHLIGHT_COLOR);
        assert_eq!(d.placeholder_alpha, scene_build::PLACEHOLDER_ALPHA);
        assert_eq!(d.composition_underline_thick, scene_build::COMPOSITION_UNDERLINE_THICK);
        assert_eq!(d.toolbar_corner_radius, selection_chrome::TOOLBAR_CORNER_RADIUS);
    }
}
