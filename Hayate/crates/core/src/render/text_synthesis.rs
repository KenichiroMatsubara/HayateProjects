//! Font synthesis helpers for scene renderers (ADR-0085).

/// Divisor for faux-bold expansion in font units (browser-style synthetic bold).
pub const EMBOLDEN_UNITS_DIVISOR: f64 = 32.0;

/// Faux-bold stroke/expansion amount in font design units.
pub fn embolden_amount_font_units(units_per_em: u16) -> f64 {
    f64::from(units_per_em) / EMBOLDEN_UNITS_DIVISOR
}

/// Skew tangent for faux italic/oblique (CSS uses degrees).
pub fn italic_skew_tangent(degrees: f32) -> f32 {
    degrees.to_radians().tan()
}
