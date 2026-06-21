//! Shared missing-glyph handling for the Canvas scene painters (issue #427).
//!
//! Glyph id 0 is `.notdef` in every OpenType/TrueType font: the value shaping
//! emits when a codepoint is absent from the chosen font. NotoSansJP's `.notdef`
//! has no outline, so a missing codepoint (e.g. `✕` U+2715) used to vanish into
//! a *silent* blank box on Canvas while the DOM renderer fell back to system
//! fonts. Both the vello and tiny-skia painters now intercept `.notdef` and draw
//! a deliberate, visible placeholder via [`missing_glyph_placeholder`] instead of
//! emitting the font's silent box — so a glyph the font can't supply degrades
//! into something the user can see rather than nothing.

use crate::render::RenderGlyph;

/// Glyph id 0 — `.notdef` — in every OpenType/TrueType font.
pub const NOTDEF_GLYPH_ID: u32 = 0;

/// Fallback font chain consulted when the primary font lacks a glyph.
///
/// This is a **placeholder** set of family names kept as a single named constant
/// so the painters and shaping never hard-code family literals inline; finalizing
/// the list (and wiring real font assets through it) is a follow-up to issue #427.
/// Until those faces are bundled, [`missing_glyph_placeholder`] is the visible
/// degradation path.
pub const FALLBACK_FONT_CHAIN: &[&str] = &[
    "Noto Sans Symbols 2",
    "Noto Sans Symbols",
    "Noto Sans Math",
    "Noto Color Emoji",
    "Noto Sans",
];

/// Whether `glyph` is the `.notdef` glyph (a codepoint the font cannot supply).
#[inline]
pub fn is_notdef(glyph: &RenderGlyph) -> bool {
    glyph.id == NOTDEF_GLYPH_ID
}

/// Geometry of the deliberate placeholder box drawn in place of a `.notdef`
/// glyph, in run-local coordinates (the same space as [`RenderGlyph::x`] /
/// [`RenderGlyph::y`]): painters add the run origin and stroke the rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MissingGlyphPlaceholder {
    /// Left edge, run-local.
    pub x: f32,
    /// Top edge, run-local (above the baseline at [`RenderGlyph::y`]).
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// Stroke width for the hollow box outline.
    pub stroke_width: f32,
}

/// Compute the placeholder box for a `.notdef` `glyph` at `font_size`.
///
/// [`RenderGlyph::y`] is the baseline; the box sits in the cap-height band above
/// it, inset from the pen origin so adjacent placeholders read as separate cells.
/// The advance width is not carried on [`RenderGlyph`], so the box is sized from
/// the em (`font_size`) — wide enough to be unmistakable, narrow enough to fit a
/// typical symbol/CJK advance.
pub fn missing_glyph_placeholder(glyph: &RenderGlyph, font_size: f32) -> MissingGlyphPlaceholder {
    let em = font_size.max(0.0);
    let inset = em * 0.08;
    let width = (em * 0.55 - inset).max(0.0);
    let height = (em * 0.62).max(0.0);
    // Bottom edge just above the baseline, box rising into the cap-height band.
    let top = glyph.y - height - em * 0.02;
    MissingGlyphPlaceholder {
        x: glyph.x + inset,
        y: top,
        width,
        height,
        stroke_width: (em * 0.06).max(1.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glyph(id: u32) -> RenderGlyph {
        RenderGlyph { id, x: 10.0, y: 30.0 }
    }

    #[test]
    fn notdef_is_glyph_zero() {
        assert_eq!(NOTDEF_GLYPH_ID, 0);
        assert!(is_notdef(&glyph(0)));
        assert!(!is_notdef(&glyph(1)));
    }

    #[test]
    fn fallback_chain_is_non_empty_named_constant() {
        assert!(!FALLBACK_FONT_CHAIN.is_empty());
        assert!(FALLBACK_FONT_CHAIN.iter().all(|f| !f.is_empty()));
    }

    #[test]
    fn placeholder_box_is_visible_and_above_baseline() {
        let g = glyph(0);
        let ph = missing_glyph_placeholder(&g, 40.0);
        assert!(ph.width > 0.0 && ph.height > 0.0, "placeholder must have area");
        assert!(ph.stroke_width >= 1.0, "stroke must be at least 1px");
        // The box sits above the baseline (smaller screen y) and to the right of
        // the pen origin.
        assert!(ph.y < g.y, "box top must be above the baseline");
        assert!(ph.y + ph.height <= g.y, "box must not dip below the baseline");
        assert!(ph.x >= g.x, "box must be inset from the pen origin");
    }

    #[test]
    fn placeholder_degrades_gracefully_at_zero_size() {
        let ph = missing_glyph_placeholder(&glyph(0), 0.0);
        assert_eq!(ph.width, 0.0);
        assert_eq!(ph.height, 0.0);
        assert!(ph.stroke_width >= 1.0);
    }
}
