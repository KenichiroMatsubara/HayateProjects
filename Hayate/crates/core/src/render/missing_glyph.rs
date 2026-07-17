//! Canvas シーンペインタ共通の欠落グリフ処理。
//!
//! グリフ id 0 はどの OpenType/TrueType フォントでも `.notdef`（フォントに存在
//! しないコードポイントに対してシェイピングが返す値）。NotoSansJP の `.notdef`
//! はアウトラインを持たないため、欠落コードポイント（例: `✕` U+2715）は Canvas
//! 上で無音の空白箱に消えてしまう。vello/tiny-skia の両ペインタは `.notdef` を
//! 検出し、フォントの無音箱の代わりに [`missing_glyph_placeholder`] で可視の
//! プレースホルダを描く。フォントが供給できないグリフを「見える形」に縮退させる。

use crate::render::RenderGlyph;

/// グリフ id 0（どの OpenType/TrueType フォントでも `.notdef`）。
pub const NOTDEF_GLYPH_ID: u32 = 0;

/// 主フォントにグリフが無いとき参照するフォールバックフォント連鎖。
///
/// ペインタやシェイピングがファミリ名リテラルをインラインで埋め込まないよう、
/// 単一の名前付き定数にまとめている。これらのフェイスが同梱されるまでは
/// [`missing_glyph_placeholder`] が可視の縮退経路となる。
pub const FALLBACK_FONT_CHAIN: &[&str] = &[
    "Noto Sans Symbols 2",
    "Noto Sans Symbols",
    "Noto Sans Math",
    "Noto Color Emoji",
    "Noto Sans",
];

/// `glyph` が `.notdef`（フォントが供給できないコードポイント）かどうか。
#[inline]
pub fn is_notdef(glyph: &RenderGlyph) -> bool {
    glyph.id == NOTDEF_GLYPH_ID
}

/// `.notdef` グリフの代わりに描くプレースホルダ箱のジオメトリ。座標は run ローカル
/// （[`RenderGlyph::x`] / [`RenderGlyph::y`] と同じ空間）で、ペインタが run 原点を
/// 加えて矩形をストロークする。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MissingGlyphPlaceholder {
    /// 左端（run ローカル）。
    pub x: f32,
    /// 上端（run ローカル、[`RenderGlyph::y`] のベースラインより上）。
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// 中空の箱の輪郭のストローク幅。
    pub stroke_width: f32,
}

/// `font_size` の `.notdef` `glyph` に対するプレースホルダ箱を計算する。
///
/// [`RenderGlyph::y`] がベースライン。箱はその上のキャップハイト帯に置き、ペン原点
/// からインセットして隣接プレースホルダが別セルと読めるようにする。advance 幅は
/// [`RenderGlyph`] に載らないため、箱は em（`font_size`）から寸法を取る。一般的な
/// シンボル/CJK の advance に収まりつつ、見間違えない大きさにしている。
pub fn missing_glyph_placeholder(glyph: &RenderGlyph, font_size: f32) -> MissingGlyphPlaceholder {
    let em = font_size.max(0.0);
    let inset = em * 0.08;
    let width = (em * 0.55 - inset).max(0.0);
    let height = (em * 0.62).max(0.0);
    // 下端をベースラインの直上に置き、箱はキャップハイト帯へ立ち上げる。
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
        RenderGlyph {
            id,
            x: 10.0,
            y: 30.0,
        }
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
        assert!(
            ph.width > 0.0 && ph.height > 0.0,
            "placeholder must have area"
        );
        assert!(ph.stroke_width >= 1.0, "stroke must be at least 1px");
        // 箱はベースラインより上（画面 y が小さい）かつペン原点より右にある。
        assert!(ph.y < g.y, "box top must be above the baseline");
        assert!(
            ph.y + ph.height <= g.y,
            "box must not dip below the baseline"
        );
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
