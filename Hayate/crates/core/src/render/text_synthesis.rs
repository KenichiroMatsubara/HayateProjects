//! シーンレンダラ向けのフォント合成ヘルパー（ADR-0085）。

/// 疑似ボールドの太らせ量（フォント単位）の除数。ブラウザ流の合成ボールド。
pub const EMBOLDEN_UNITS_DIVISOR: f64 = 32.0;

/// 疑似ボールドのストローク/太らせ量（フォントデザイン単位）。
pub fn embolden_amount_font_units(units_per_em: u16) -> f64 {
    f64::from(units_per_em) / EMBOLDEN_UNITS_DIVISOR
}

/// 疑似イタリック/斜体のスキュー tangent（CSS は度数指定）。
pub fn italic_skew_tangent(degrees: f32) -> f32 {
    degrees.to_radians().tan()
}
