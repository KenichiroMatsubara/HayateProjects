use hayate_core::{
    AlignValue, Color, Dimension, DimensionUnit, DisplayValue, FlexDirectionValue, JustifyValue,
    StyleProp,
};
use wasm_bindgen::prelude::*;

// Tag IDs — keep in sync with style-encoding.js in the demo.
pub(crate) const TAG_BACKGROUND_COLOR: u32 = 0;
pub(crate) const TAG_OPACITY: u32 = 1;
pub(crate) const TAG_BORDER_RADIUS: u32 = 2;
pub(crate) const TAG_BORDER_WIDTH: u32 = 3;
pub(crate) const TAG_BORDER_COLOR: u32 = 4;
pub(crate) const TAG_WIDTH: u32 = 5;
pub(crate) const TAG_HEIGHT: u32 = 6;
pub(crate) const TAG_MIN_WIDTH: u32 = 7;
pub(crate) const TAG_MIN_HEIGHT: u32 = 8;
pub(crate) const TAG_MAX_WIDTH: u32 = 9;
pub(crate) const TAG_MAX_HEIGHT: u32 = 10;
pub(crate) const TAG_DISPLAY: u32 = 11;
pub(crate) const TAG_FLEX_DIRECTION: u32 = 12;
pub(crate) const TAG_ALIGN_ITEMS: u32 = 13;
pub(crate) const TAG_JUSTIFY_CONTENT: u32 = 14;
pub(crate) const TAG_GAP: u32 = 15;
pub(crate) const TAG_PADDING: u32 = 16;
pub(crate) const TAG_PADDING_TOP: u32 = 17;
pub(crate) const TAG_PADDING_RIGHT: u32 = 18;
pub(crate) const TAG_PADDING_BOTTOM: u32 = 19;
pub(crate) const TAG_PADDING_LEFT: u32 = 20;
pub(crate) const TAG_MARGIN: u32 = 21;
pub(crate) const TAG_MARGIN_TOP: u32 = 22;
pub(crate) const TAG_MARGIN_RIGHT: u32 = 23;
pub(crate) const TAG_MARGIN_BOTTOM: u32 = 24;
pub(crate) const TAG_MARGIN_LEFT: u32 = 25;
pub(crate) const TAG_FONT_SIZE: u32 = 26;
pub(crate) const TAG_COLOR: u32 = 27;
pub(crate) const TAG_Z_INDEX: u32 = 28;

fn dim(value: f32, unit_raw: f32) -> Dimension {
    let unit = match unit_raw as u32 {
        0 => DimensionUnit::Px,
        1 => DimensionUnit::Percent,
        2 => DimensionUnit::Auto,
        3 => DimensionUnit::Fr,
        _ => DimensionUnit::Px,
    };
    Dimension { value, unit }
}

fn color(r: f32, g: f32, b: f32, a: f32) -> Color {
    Color::new(r as f64, g as f64, b as f64, a as f64)
}

pub(crate) fn decode(packed: &[f32]) -> Result<Vec<StyleProp>, JsValue> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < packed.len() {
        let tag = packed[i] as u32;
        i += 1;
        let need = |n: usize, tag: u32| -> Result<(), JsValue> {
            if i + n > packed.len() {
                Err(JsValue::from_str(&format!(
                    "style packet truncated at tag {tag}"
                )))
            } else {
                Ok(())
            }
        };
        match tag {
            TAG_BACKGROUND_COLOR => {
                need(4, tag)?;
                out.push(StyleProp::BackgroundColor(color(
                    packed[i], packed[i + 1], packed[i + 2], packed[i + 3],
                )));
                i += 4;
            }
            TAG_OPACITY => {
                need(1, tag)?;
                out.push(StyleProp::Opacity(packed[i]));
                i += 1;
            }
            TAG_BORDER_RADIUS => {
                need(1, tag)?;
                out.push(StyleProp::BorderRadius(packed[i]));
                i += 1;
            }
            TAG_BORDER_WIDTH => {
                need(1, tag)?;
                out.push(StyleProp::BorderWidth(packed[i]));
                i += 1;
            }
            TAG_BORDER_COLOR => {
                need(4, tag)?;
                out.push(StyleProp::BorderColor(color(
                    packed[i], packed[i + 1], packed[i + 2], packed[i + 3],
                )));
                i += 4;
            }
            TAG_WIDTH => {
                need(2, tag)?;
                out.push(StyleProp::Width(dim(packed[i], packed[i + 1])));
                i += 2;
            }
            TAG_HEIGHT => {
                need(2, tag)?;
                out.push(StyleProp::Height(dim(packed[i], packed[i + 1])));
                i += 2;
            }
            TAG_MIN_WIDTH => {
                need(2, tag)?;
                out.push(StyleProp::MinWidth(dim(packed[i], packed[i + 1])));
                i += 2;
            }
            TAG_MIN_HEIGHT => {
                need(2, tag)?;
                out.push(StyleProp::MinHeight(dim(packed[i], packed[i + 1])));
                i += 2;
            }
            TAG_MAX_WIDTH => {
                need(2, tag)?;
                out.push(StyleProp::MaxWidth(dim(packed[i], packed[i + 1])));
                i += 2;
            }
            TAG_MAX_HEIGHT => {
                need(2, tag)?;
                out.push(StyleProp::MaxHeight(dim(packed[i], packed[i + 1])));
                i += 2;
            }
            TAG_DISPLAY => {
                need(1, tag)?;
                let v = match packed[i] as u32 {
                    0 => DisplayValue::Flex,
                    1 => DisplayValue::Grid,
                    2 => DisplayValue::Block,
                    3 => DisplayValue::None,
                    _ => DisplayValue::Flex,
                };
                out.push(StyleProp::Display(v));
                i += 1;
            }
            TAG_FLEX_DIRECTION => {
                need(1, tag)?;
                let v = match packed[i] as u32 {
                    0 => FlexDirectionValue::Row,
                    1 => FlexDirectionValue::Column,
                    2 => FlexDirectionValue::RowReverse,
                    3 => FlexDirectionValue::ColumnReverse,
                    _ => FlexDirectionValue::Row,
                };
                out.push(StyleProp::FlexDirection(v));
                i += 1;
            }
            TAG_ALIGN_ITEMS => {
                need(1, tag)?;
                let v = match packed[i] as u32 {
                    0 => AlignValue::FlexStart,
                    1 => AlignValue::FlexEnd,
                    2 => AlignValue::Center,
                    3 => AlignValue::Stretch,
                    4 => AlignValue::Baseline,
                    _ => AlignValue::FlexStart,
                };
                out.push(StyleProp::AlignItems(v));
                i += 1;
            }
            TAG_JUSTIFY_CONTENT => {
                need(1, tag)?;
                let v = match packed[i] as u32 {
                    0 => JustifyValue::FlexStart,
                    1 => JustifyValue::FlexEnd,
                    2 => JustifyValue::Center,
                    3 => JustifyValue::SpaceBetween,
                    4 => JustifyValue::SpaceAround,
                    5 => JustifyValue::SpaceEvenly,
                    _ => JustifyValue::FlexStart,
                };
                out.push(StyleProp::JustifyContent(v));
                i += 1;
            }
            TAG_GAP => {
                need(2, tag)?;
                out.push(StyleProp::Gap(dim(packed[i], packed[i + 1])));
                i += 2;
            }
            TAG_PADDING => {
                need(2, tag)?;
                out.push(StyleProp::Padding(dim(packed[i], packed[i + 1])));
                i += 2;
            }
            TAG_PADDING_TOP => {
                need(2, tag)?;
                out.push(StyleProp::PaddingTop(dim(packed[i], packed[i + 1])));
                i += 2;
            }
            TAG_PADDING_RIGHT => {
                need(2, tag)?;
                out.push(StyleProp::PaddingRight(dim(packed[i], packed[i + 1])));
                i += 2;
            }
            TAG_PADDING_BOTTOM => {
                need(2, tag)?;
                out.push(StyleProp::PaddingBottom(dim(packed[i], packed[i + 1])));
                i += 2;
            }
            TAG_PADDING_LEFT => {
                need(2, tag)?;
                out.push(StyleProp::PaddingLeft(dim(packed[i], packed[i + 1])));
                i += 2;
            }
            TAG_MARGIN => {
                need(2, tag)?;
                out.push(StyleProp::Margin(dim(packed[i], packed[i + 1])));
                i += 2;
            }
            TAG_MARGIN_TOP => {
                need(2, tag)?;
                out.push(StyleProp::MarginTop(dim(packed[i], packed[i + 1])));
                i += 2;
            }
            TAG_MARGIN_RIGHT => {
                need(2, tag)?;
                out.push(StyleProp::MarginRight(dim(packed[i], packed[i + 1])));
                i += 2;
            }
            TAG_MARGIN_BOTTOM => {
                need(2, tag)?;
                out.push(StyleProp::MarginBottom(dim(packed[i], packed[i + 1])));
                i += 2;
            }
            TAG_MARGIN_LEFT => {
                need(2, tag)?;
                out.push(StyleProp::MarginLeft(dim(packed[i], packed[i + 1])));
                i += 2;
            }
            TAG_FONT_SIZE => {
                need(1, tag)?;
                out.push(StyleProp::FontSize(packed[i]));
                i += 1;
            }
            TAG_COLOR => {
                need(4, tag)?;
                out.push(StyleProp::Color(color(
                    packed[i], packed[i + 1], packed[i + 2], packed[i + 3],
                )));
                i += 4;
            }
            TAG_Z_INDEX => {
                need(1, tag)?;
                out.push(StyleProp::ZIndex(packed[i] as i32));
                i += 1;
            }
            other => {
                return Err(JsValue::from_str(&format!("unknown style tag {other}")));
            }
        }
    }
    Ok(out)
}
