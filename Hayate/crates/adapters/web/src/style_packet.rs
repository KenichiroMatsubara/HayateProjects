use hayate_core::StyleProp;
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use web_sys::CssStyleDeclaration;

/// スタイルパケットのスライスを `StyleProp` 値へデコードする（proto/spec から生成）。
pub(crate) fn decode(packed: &[f32]) -> Result<Vec<StyleProp>, JsValue> {
    // 中立化した decode_style_packet（ADR-0112）は `String` エラーを返すため、
    // Web 境界で `JsValue` へ写す。
    crate::generated::decode_style_packet(packed).map_err(|e| JsValue::from_str(&e))
}

// ── Hayate CSS → ブラウザ CSS マッピング（HTML モード、ADR-0029） ───────────────

/// Hayate CSS プロパティ群を DOM 要素の style 宣言へ直接適用する。
/// レイアウトプロパティ（`display`、`gap`、`flex-direction` …）はブラウザ CSS と 1:1 対応し、
/// レイアウトはブラウザエンジンが行う — Taffy は介在しない（ADR-0029）。
#[cfg(target_arch = "wasm32")]
pub(crate) fn apply_props_to_dom(
    style: &CssStyleDeclaration,
    props: &[StyleProp],
) -> Result<(), JsValue> {
    for p in props {
        crate::generated::apply_style_prop_to_dom(style, p)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hayate_core::{DimensionUnit, DisplayValue, FlexDirectionValue, AlignValue, JustifyValue};

    fn ok(packed: &[f32]) -> Vec<StyleProp> {
        decode(packed).expect("decode should not fail")
    }

    // ── 色プロパティ ──────────────────────────────────────────────────────

    #[test]
    fn background_color_rgba() {
        // TAG_BACKGROUND_COLOR=0, r=1.0, g=0.5, b=0.25, a=1.0
        let props = ok(&[0.0, 1.0, 0.5, 0.25, 1.0]);
        assert_eq!(props.len(), 1);
        match &props[0] {
            StyleProp::BackgroundColor(c) => {
                assert!((c.r - 1.0).abs() < 1e-6);
                assert!((c.g - 0.5).abs() < 1e-6);
                assert!((c.b - 0.25).abs() < 1e-6);
                assert!((c.a - 1.0).abs() < 1e-6);
            }
            other => panic!("expected BackgroundColor, got {:?}", other),
        }
    }

    #[test]
    fn border_color_rgba() {
        // TAG_BORDER_COLOR=4
        let props = ok(&[4.0, 0.0, 0.0, 1.0, 0.5]);
        assert_eq!(props.len(), 1);
        match &props[0] {
            StyleProp::BorderColor(c) => {
                assert!((c.r - 0.0).abs() < 1e-6);
                assert!((c.b - 1.0).abs() < 1e-6);
                assert!((c.a - 0.5).abs() < 1e-6);
            }
            other => panic!("expected BorderColor, got {:?}", other),
        }
    }

    #[test]
    fn color_rgba() {
        // TAG_COLOR=27
        let props = ok(&[27.0, 1.0, 0.0, 0.0, 1.0]);
        assert_eq!(props.len(), 1);
        match &props[0] {
            StyleProp::Color(c) => {
                assert!((c.r - 1.0).abs() < 1e-6);
                assert!((c.g - 0.0).abs() < 1e-6);
                assert!((c.b - 0.0).abs() < 1e-6);
                assert!((c.a - 1.0).abs() < 1e-6);
            }
            other => panic!("expected Color, got {:?}", other),
        }
    }

    // ── 寸法プロパティ ──────────────────────────────────────────────────

    #[test]
    fn width_px() {
        // TAG_WIDTH=5, value=100.0, unit=0 (px)
        let props = ok(&[5.0, 100.0, 0.0]);
        assert_eq!(props.len(), 1);
        match &props[0] {
            StyleProp::Width(d) => {
                assert!((d.value - 100.0).abs() < 1e-6);
                assert!(matches!(d.unit, DimensionUnit::Px));
            }
            other => panic!("expected Width, got {:?}", other),
        }
    }

    #[test]
    fn width_percent() {
        // TAG_WIDTH=5, value=50.0, unit=1 (percent)
        let props = ok(&[5.0, 50.0, 1.0]);
        assert_eq!(props.len(), 1);
        match &props[0] {
            StyleProp::Width(d) => {
                assert!((d.value - 50.0).abs() < 1e-6);
                assert!(matches!(d.unit, DimensionUnit::Percent));
            }
            other => panic!("expected Width, got {:?}", other),
        }
    }

    #[test]
    fn width_auto() {
        // TAG_WIDTH=5, value=0.0, unit=2 (auto)
        let props = ok(&[5.0, 0.0, 2.0]);
        assert_eq!(props.len(), 1);
        match &props[0] {
            StyleProp::Width(d) => {
                assert!(matches!(d.unit, DimensionUnit::Auto));
            }
            other => panic!("expected Width, got {:?}", other),
        }
    }

    #[test]
    fn height_px() {
        // TAG_HEIGHT=6
        let props = ok(&[6.0, 200.0, 0.0]);
        assert_eq!(props.len(), 1);
        match &props[0] {
            StyleProp::Height(d) => {
                assert!((d.value - 200.0).abs() < 1e-6);
                assert!(matches!(d.unit, DimensionUnit::Px));
            }
            other => panic!("expected Height, got {:?}", other),
        }
    }

    // ── Display enum ──────────────────────────────────────────────────

    #[test]
    fn display_flex() {
        // TAG_DISPLAY=11, code=0 (flex)
        let props = ok(&[11.0, 0.0]);
        assert_eq!(props.len(), 1);
        match &props[0] {
            StyleProp::Display(v) => assert!(matches!(v, DisplayValue::Flex)),
            other => panic!("expected Display(Flex), got {:?}", other),
        }
    }

    #[test]
    fn display_grid() {
        let props = ok(&[11.0, 1.0]);
        match &props[0] {
            StyleProp::Display(v) => assert!(matches!(v, DisplayValue::Grid)),
            other => panic!("expected Display(Grid), got {:?}", other),
        }
    }

    #[test]
    fn display_none() {
        let props = ok(&[11.0, 3.0]);
        match &props[0] {
            StyleProp::Display(v) => assert!(matches!(v, DisplayValue::None)),
            other => panic!("expected Display(None), got {:?}", other),
        }
    }

    // ── FlexDirection enum ────────────────────────────────────────────

    #[test]
    fn flex_direction_row() {
        // TAG_FLEX_DIRECTION=12, code=0 (row)
        let props = ok(&[12.0, 0.0]);
        match &props[0] {
            StyleProp::FlexDirection(v) => assert!(matches!(v, FlexDirectionValue::Row)),
            other => panic!("expected FlexDirection(Row), got {:?}", other),
        }
    }

    #[test]
    fn flex_direction_column() {
        let props = ok(&[12.0, 1.0]);
        match &props[0] {
            StyleProp::FlexDirection(v) => assert!(matches!(v, FlexDirectionValue::Column)),
            other => panic!("expected FlexDirection(Column), got {:?}", other),
        }
    }

    #[test]
    fn flex_direction_row_reverse() {
        let props = ok(&[12.0, 2.0]);
        match &props[0] {
            StyleProp::FlexDirection(v) => assert!(matches!(v, FlexDirectionValue::RowReverse)),
            other => panic!("{:?}", other),
        }
    }

    // ── AlignItems enum ───────────────────────────────────────────────

    #[test]
    fn align_items_flex_start() {
        // TAG_ALIGN_ITEMS=13, code=0 (flex-start)
        let props = ok(&[13.0, 0.0]);
        match &props[0] {
            StyleProp::AlignItems(v) => assert!(matches!(v, AlignValue::FlexStart)),
            other => panic!("{:?}", other),
        }
    }

    #[test]
    fn align_items_center() {
        let props = ok(&[13.0, 2.0]);
        match &props[0] {
            StyleProp::AlignItems(v) => assert!(matches!(v, AlignValue::Center)),
            other => panic!("{:?}", other),
        }
    }

    // ── JustifyContent enum ───────────────────────────────────────────

    #[test]
    fn justify_content_flex_start() {
        // TAG_JUSTIFY_CONTENT=14, code=0
        let props = ok(&[14.0, 0.0]);
        match &props[0] {
            StyleProp::JustifyContent(v) => assert!(matches!(v, JustifyValue::FlexStart)),
            other => panic!("{:?}", other),
        }
    }

    #[test]
    fn justify_content_space_between() {
        let props = ok(&[14.0, 3.0]);
        match &props[0] {
            StyleProp::JustifyContent(v) => assert!(matches!(v, JustifyValue::SpaceBetween)),
            other => panic!("{:?}", other),
        }
    }

    // ── Overflow enum ─────────────────────────────────────────────────

    #[test]
    fn overflow_hidden() {
        use hayate_core::OverflowValue;
        // TAG_OVERFLOW=52, code=1 (hidden)
        let props = ok(&[52.0, 1.0]);
        match &props[0] {
            StyleProp::Overflow(v) => assert!(matches!(v, OverflowValue::Hidden)),
            other => panic!("expected Overflow(Hidden), got {:?}", other),
        }
    }

    #[test]
    fn overflow_visible() {
        use hayate_core::OverflowValue;
        let props = ok(&[52.0, 0.0]);
        match &props[0] {
            StyleProp::Overflow(v) => assert!(matches!(v, OverflowValue::Visible)),
            other => panic!("expected Overflow(Visible), got {:?}", other),
        }
    }

    // ── MAX_LINES (u32) + TEXT_OVERFLOW enum ─────────────────────

    #[test]
    fn max_lines_u32() {
        // TAG_MAX_LINES=53, value=3
        let props = ok(&[53.0, 3.0]);
        assert_eq!(props.len(), 1);
        match &props[0] {
            StyleProp::MaxLines(v) => assert_eq!(*v, 3),
            other => panic!("expected MaxLines(3), got {:?}", other),
        }
    }

    #[test]
    fn text_overflow_clip() {
        use hayate_core::TextOverflowValue;
        // TAG_TEXT_OVERFLOW=54, code=0 (clip)
        let props = ok(&[54.0, 0.0]);
        match &props[0] {
            StyleProp::TextOverflow(v) => assert!(matches!(v, TextOverflowValue::Clip)),
            other => panic!("expected TextOverflow(Clip), got {:?}", other),
        }
    }

    #[test]
    fn text_overflow_ellipsis() {
        use hayate_core::TextOverflowValue;
        // TAG_TEXT_OVERFLOW=54, code=1 (ellipsis)
        let props = ok(&[54.0, 1.0]);
        match &props[0] {
            StyleProp::TextOverflow(v) => assert!(matches!(v, TextOverflowValue::Ellipsis)),
            other => panic!("expected TextOverflow(Ellipsis), got {:?}", other),
        }
    }

    // ── FontFamily ────────────────────────────────────────────────────

    #[test]
    fn font_family_inter() {
        // TAG_FONT_FAMILY=29, byte_len=5, 'I'=73,'n'=110,'t'=116,'e'=101,'r'=114
        let family = "Inter";
        let bytes: Vec<f32> = family.bytes().map(|b| b as f32).collect();
        let mut packet = vec![29.0_f32, bytes.len() as f32];
        packet.extend_from_slice(&bytes);
        let props = ok(&packet);
        assert_eq!(props.len(), 1);
        match &props[0] {
            StyleProp::FontFamily(f) => assert_eq!(f, "Inter"),
            other => panic!("expected FontFamily, got {:?}", other),
        }
    }

    #[test]
    fn font_family_noto_sans_jp() {
        let family = "Noto Sans JP";
        let bytes: Vec<f32> = family.bytes().map(|b| b as f32).collect();
        let mut packet = vec![29.0_f32, bytes.len() as f32];
        packet.extend_from_slice(&bytes);
        let props = ok(&packet);
        match &props[0] {
            StyleProp::FontFamily(f) => assert_eq!(f, "Noto Sans JP"),
            other => panic!("{:?}", other),
        }
    }

    // ── スカラープロパティ ─────────────────────────────────────────────────────

    #[test]
    fn opacity() {
        // TAG_OPACITY=1
        let props = ok(&[1.0, 0.5]);
        match &props[0] {
            StyleProp::Opacity(v) => assert!((v - 0.5).abs() < 1e-6),
            other => panic!("{:?}", other),
        }
    }

    #[test]
    fn border_radius() {
        // TAG_BORDER_RADIUS=2
        let props = ok(&[2.0, 8.0]);
        match &props[0] {
            StyleProp::BorderRadius(v) => assert!((v - 8.0).abs() < 1e-6),
            other => panic!("{:?}", other),
        }
    }

    #[test]
    fn font_size() {
        // TAG_FONT_SIZE=26
        let props = ok(&[26.0, 16.0]);
        match &props[0] {
            StyleProp::FontSize(v) => assert!((v - 16.0).abs() < 1e-6),
            other => panic!("{:?}", other),
        }
    }

    #[test]
    fn font_weight() {
        // TAG_FONT_WEIGHT=31
        let props = ok(&[31.0, 700.0]);
        match &props[0] {
            StyleProp::FontWeight(v) => assert!((v - 700.0).abs() < 1e-6),
            other => panic!("{:?}", other),
        }
    }

    #[test]
    fn z_index() {
        // TAG_Z_INDEX=28
        let props = ok(&[28.0, 10.0]);
        match &props[0] {
            StyleProp::ZIndex(v) => assert_eq!(*v, 10),
            other => panic!("{:?}", other),
        }
    }

    // ── 複数プロパティのパケット ─────────────────────────────────────────────────────

    #[test]
    fn multiple_props_in_sequence() {
        // background_color + width + display の組み合わせ
        let packet = [
            0.0_f32, 1.0, 0.0, 0.0, 1.0,  // TAG_BACKGROUND_COLOR=0 + rgba
            5.0, 100.0, 0.0,               // TAG_WIDTH=5 + px
            11.0, 0.0,                     // TAG_DISPLAY=11 + flex
        ];
        let props = ok(&packet);
        assert_eq!(props.len(), 3);
        assert!(matches!(&props[0], StyleProp::BackgroundColor(_)));
        assert!(matches!(&props[1], StyleProp::Width(_)));
        assert!(matches!(&props[2], StyleProp::Display(_)));
    }

    #[test]
    fn empty_packet_returns_empty_vec() {
        let props = ok(&[]);
        assert!(props.is_empty());
    }

    // エラー経路のテスト（切り詰め／未知タグ）はネイティブモードでは実行しない。
    // wasm32 外では JsValue::from_str() が process::abort() を呼ぶスタブのため。
    // これらの経路は wasm-pack テストでカバーする。
}

