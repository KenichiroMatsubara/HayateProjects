//! Dev-only `tuning.json` parsing for the web Platform Adapter.
//!
//! The authoritative defaults are the Rust `const`s (in `scroll_drag::physics`
//! and `hayate_core`'s scene-build / selection-chrome blocks). This module lets
//! a developer overlay them at runtime: the host fetches `tuning.json` and hands
//! the text to [`HayateElementRenderer::set_tuning`](crate::canvas), which parses
//! it here. Every field is optional — only the keys present in the JSON override
//! their default, and a malformed file is rejected wholesale (the caller keeps
//! the defaults). Parsing lives in the adapter because `hayate-core` deliberately
//! carries no runtime serde dependency.

use hayate_core::{ChromeTuning, Color};
use serde::Deserialize;

use crate::scroll_drag::ScrollPhysicsTuning;

/// Top-level shape of `tuning.json`: two optional sections.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TuningJson {
    pub scroll: Option<ScrollJson>,
    pub chrome: Option<ChromeJson>,
}

impl TuningJson {
    /// Parse `tuning.json` text. `Err` on malformed JSON or unknown keys so the
    /// caller can fall back to the compiled defaults intact.
    pub fn parse(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }

    /// The merged scroll-physics knobs (defaults overlaid by any present keys).
    pub fn scroll_tuning(&self) -> ScrollPhysicsTuning {
        self.scroll.as_ref().map(ScrollJson::merged).unwrap_or_default()
    }

    /// The merged chrome knobs (defaults overlaid by any present keys).
    pub fn chrome_tuning(&self) -> ChromeTuning {
        self.chrome.as_ref().map(ChromeJson::merged).unwrap_or_default()
    }
}

/// Overlay one `Option` onto a mutable default field.
fn overlay<T>(slot: &mut T, value: Option<T>) {
    if let Some(v) = value {
        *slot = v;
    }
}

/// A `[r, g, b, a]` (0..1) JSON array converted to a core [`Color`].
fn color_from(rgba: [f64; 4]) -> Color {
    Color::new(rgba[0], rgba[1], rgba[2], rgba[3])
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ScrollJson {
    slop_px: Option<f32>,
    deceleration_rate: Option<f32>,
    max_release_velocity: Option<f32>,
    min_velocity: Option<f32>,
    sample_window_ms: Option<f64>,
    rubber_band_c: Option<f32>,
    spring_stiffness: Option<f32>,
    spring_damping: Option<f32>,
    spring_rest_offset: Option<f32>,
    spring_rest_velocity: Option<f32>,
}

impl ScrollJson {
    fn merged(&self) -> ScrollPhysicsTuning {
        let mut d = ScrollPhysicsTuning::default();
        overlay(&mut d.slop_px, self.slop_px);
        overlay(&mut d.deceleration_rate, self.deceleration_rate);
        overlay(&mut d.max_release_velocity, self.max_release_velocity);
        overlay(&mut d.min_velocity, self.min_velocity);
        overlay(&mut d.sample_window_ms, self.sample_window_ms);
        overlay(&mut d.rubber_band_c, self.rubber_band_c);
        overlay(&mut d.spring_stiffness, self.spring_stiffness);
        overlay(&mut d.spring_damping, self.spring_damping);
        overlay(&mut d.spring_rest_offset, self.spring_rest_offset);
        overlay(&mut d.spring_rest_velocity, self.spring_rest_velocity);
        d
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ChromeJson {
    scrollbar_thickness: Option<f32>,
    scrollbar_track_margin: Option<f32>,
    scrollbar_min_thumb_length: Option<f32>,
    scrollbar_thumb_color: Option<[f64; 4]>,
    scrollbar_thumb_opacity: Option<f32>,
    scrollbar_indicator_thickness: Option<f32>,
    scrollbar_indicator_color: Option<[f64; 4]>,
    scrollbar_indicator_opacity: Option<f32>,
    selection_highlight_color: Option<[f32; 4]>,
    composition_underline_thin: Option<f32>,
    composition_underline_thick: Option<f32>,
    placeholder_alpha: Option<f64>,
    toolbar_corner_radius: Option<f32>,
}

impl ChromeJson {
    fn merged(&self) -> ChromeTuning {
        let mut d = ChromeTuning::default();
        overlay(&mut d.scrollbar_thickness, self.scrollbar_thickness);
        overlay(&mut d.scrollbar_track_margin, self.scrollbar_track_margin);
        overlay(&mut d.scrollbar_min_thumb_length, self.scrollbar_min_thumb_length);
        overlay(&mut d.scrollbar_thumb_color, self.scrollbar_thumb_color.map(color_from));
        overlay(&mut d.scrollbar_thumb_opacity, self.scrollbar_thumb_opacity);
        overlay(&mut d.scrollbar_indicator_thickness, self.scrollbar_indicator_thickness);
        overlay(&mut d.scrollbar_indicator_color, self.scrollbar_indicator_color.map(color_from));
        overlay(&mut d.scrollbar_indicator_opacity, self.scrollbar_indicator_opacity);
        overlay(&mut d.selection_highlight_color, self.selection_highlight_color);
        overlay(&mut d.composition_underline_thin, self.composition_underline_thin);
        overlay(&mut d.composition_underline_thick, self.composition_underline_thick);
        overlay(&mut d.placeholder_alpha, self.placeholder_alpha);
        overlay(&mut d.toolbar_corner_radius, self.toolbar_corner_radius);
        d
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_json_yields_all_defaults() {
        let t = TuningJson::parse("{}").unwrap();
        assert_eq!(t.scroll_tuning(), ScrollPhysicsTuning::default());
        assert_eq!(t.chrome_tuning(), ChromeTuning::default());
    }

    #[test]
    fn only_present_keys_override_the_defaults() {
        let t = TuningJson::parse(r#"{ "scroll": { "deceleration_rate": 0.99 } }"#).unwrap();
        let s = t.scroll_tuning();
        // The one present key changed…
        assert_eq!(s.deceleration_rate, 0.99);
        // …and everything else stayed at its default const.
        assert_eq!(s.slop_px, ScrollPhysicsTuning::default().slop_px);
        assert_eq!(s.spring_damping, ScrollPhysicsTuning::default().spring_damping);
    }

    #[test]
    fn color_arrays_become_core_colors() {
        let t = TuningJson::parse(r#"{ "chrome": { "scrollbar_thumb_color": [1.0, 0.0, 0.0, 1.0] } }"#).unwrap();
        assert_eq!(t.chrome_tuning().scrollbar_thumb_color, Color::new(1.0, 0.0, 0.0, 1.0));
    }

    #[test]
    fn malformed_json_is_rejected_so_the_caller_keeps_defaults() {
        assert!(TuningJson::parse("{ not json").is_err());
        // Unknown keys are rejected too, surfacing typos instead of silently no-op'ing.
        assert!(TuningJson::parse(r#"{ "scroll": { "typoed_key": 1 } }"#).is_err());
    }
}
