//! core scene lowering 向けのフォント合成ヘルパー（ADR-0054 / ADR-0085）。
//!
//! 生の `fontique::Synthesis` と font メトリクスを、painter がそのまま適用できる
//! [`TextSynthesis`] へ解決する。合成の数式（度→tangent、フォント単位の太らせ量）は
//! ここに閉じ込め、各バックエンドの painter には漏らさない（ADR-0054: painter = leaf op）。

use fontique::Synthesis;

use crate::node::TextSynthesis;

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

/// 生の fontique synthesis を、painter がそのまま適用できる ready-to-apply 値へ解決する。
/// `units_per_em` はフォントフェイスの静的メトリクスで、太らせ量の算出に用いる。
pub fn resolve_synthesis(synthesis: &Synthesis, units_per_em: u16) -> TextSynthesis {
    TextSynthesis {
        skew_tangent: synthesis.skew().map(italic_skew_tangent),
        embolden: synthesis
            .embolden()
            .then(|| embolden_amount_font_units(units_per_em) as f32),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embolden_amount_scales_with_units_per_em() {
        // ブラウザ流の合成ボールド: units_per_em / 32。
        assert_eq!(embolden_amount_font_units(1000), 31.25);
        assert_eq!(embolden_amount_font_units(2048), 64.0);
    }

    #[test]
    fn italic_skew_tangent_converts_degrees_to_tangent() {
        let tangent = italic_skew_tangent(14.0);
        assert!((tangent - 14.0_f32.to_radians().tan()).abs() < 1e-6);
        assert!(tangent > 0.0, "positive degrees skew ink to the right");
        assert_eq!(italic_skew_tangent(0.0), 0.0);
    }

    #[test]
    fn resolve_synthesis_passes_through_no_synthesis() {
        // 既定の Synthesis（skew=0 / embolden=false）は ready-to-apply 値を持たない。
        assert_eq!(
            resolve_synthesis(&Synthesis::default(), 1000),
            TextSynthesis::default()
        );
    }
}
