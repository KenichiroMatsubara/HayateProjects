use taffy::{
    AlignItems, Dimension as TaffyDim, Display, FlexDirection, JustifyContent,
    LengthPercentage, LengthPercentageAuto, Rect as TaffyRect, Size, Style,
};

use crate::element::id::ElementId;
use crate::element::style::{
    AlignValue, Dimension, DimensionUnit, DisplayValue, FlexDirectionValue,
    JustifyValue, StyleProp,
};

/// Context attached to each Taffy leaf so the measure closure can dispatch.
#[derive(Clone, Copy, Debug)]
pub enum MeasureCtx {
    Text(ElementId),
    None,
}

fn to_taffy_dim(d: Dimension) -> TaffyDim {
    match d.unit {
        DimensionUnit::Px => TaffyDim::Length(d.value),
        // Hayate accepts percent as 0..100; Taffy expects 0..1.
        DimensionUnit::Percent => TaffyDim::Percent(d.value / 100.0),
        DimensionUnit::Auto => TaffyDim::Auto,
        // Taffy `Dimension` has no `Fr` representation outside grid track sizing;
        // gracefully fall back to Auto here.
        DimensionUnit::Fr => TaffyDim::Auto,
    }
}

fn to_taffy_lp(d: Dimension) -> LengthPercentage {
    match d.unit {
        DimensionUnit::Px => LengthPercentage::Length(d.value),
        DimensionUnit::Percent => LengthPercentage::Percent(d.value / 100.0),
        // Padding/gap don't accept Auto — clamp to 0.
        DimensionUnit::Auto | DimensionUnit::Fr => LengthPercentage::Length(0.0),
    }
}

fn to_taffy_lp_auto(d: Dimension) -> LengthPercentageAuto {
    match d.unit {
        DimensionUnit::Px => LengthPercentageAuto::Length(d.value),
        DimensionUnit::Percent => LengthPercentageAuto::Percent(d.value / 100.0),
        DimensionUnit::Auto => LengthPercentageAuto::Auto,
        DimensionUnit::Fr => LengthPercentageAuto::Auto,
    }
}

/// Apply a single Hayate style prop into a mutable taffy::Style. Returns true
/// if the prop was a layout prop and was applied; false otherwise (caller
/// should route to Visual instead).
pub fn apply_to_style(style: &mut Style, prop: &StyleProp) -> bool {
    match prop {
        StyleProp::Width(d) => style.size.width = to_taffy_dim(*d),
        StyleProp::Height(d) => style.size.height = to_taffy_dim(*d),
        StyleProp::MinWidth(d) => style.min_size.width = to_taffy_dim(*d),
        StyleProp::MinHeight(d) => style.min_size.height = to_taffy_dim(*d),
        StyleProp::MaxWidth(d) => style.max_size.width = to_taffy_dim(*d),
        StyleProp::MaxHeight(d) => style.max_size.height = to_taffy_dim(*d),
        StyleProp::Display(v) => {
            style.display = match v {
                DisplayValue::Flex => Display::Flex,
                DisplayValue::Grid => Display::Grid,
                DisplayValue::Block => Display::Block,
                DisplayValue::None => Display::None,
            };
        }
        StyleProp::FlexDirection(v) => {
            style.flex_direction = match v {
                FlexDirectionValue::Row => FlexDirection::Row,
                FlexDirectionValue::Column => FlexDirection::Column,
                FlexDirectionValue::RowReverse => FlexDirection::RowReverse,
                FlexDirectionValue::ColumnReverse => FlexDirection::ColumnReverse,
            };
        }
        StyleProp::AlignItems(v) => {
            style.align_items = Some(match v {
                AlignValue::FlexStart => AlignItems::FlexStart,
                AlignValue::FlexEnd => AlignItems::FlexEnd,
                AlignValue::Center => AlignItems::Center,
                AlignValue::Stretch => AlignItems::Stretch,
                AlignValue::Baseline => AlignItems::Baseline,
            });
        }
        StyleProp::JustifyContent(v) => {
            style.justify_content = Some(match v {
                JustifyValue::FlexStart => JustifyContent::FlexStart,
                JustifyValue::FlexEnd => JustifyContent::FlexEnd,
                JustifyValue::Center => JustifyContent::Center,
                JustifyValue::SpaceBetween => JustifyContent::SpaceBetween,
                JustifyValue::SpaceAround => JustifyContent::SpaceAround,
                JustifyValue::SpaceEvenly => JustifyContent::SpaceEvenly,
            });
        }
        StyleProp::Gap(d) => {
            let lp = to_taffy_lp(*d);
            style.gap = Size { width: lp, height: lp };
        }
        StyleProp::Padding(d) => {
            let lp = to_taffy_lp(*d);
            style.padding = TaffyRect { left: lp, right: lp, top: lp, bottom: lp };
        }
        StyleProp::PaddingTop(d) => style.padding.top = to_taffy_lp(*d),
        StyleProp::PaddingRight(d) => style.padding.right = to_taffy_lp(*d),
        StyleProp::PaddingBottom(d) => style.padding.bottom = to_taffy_lp(*d),
        StyleProp::PaddingLeft(d) => style.padding.left = to_taffy_lp(*d),
        StyleProp::Margin(d) => {
            let lpa = to_taffy_lp_auto(*d);
            style.margin = TaffyRect { left: lpa, right: lpa, top: lpa, bottom: lpa };
        }
        StyleProp::MarginTop(d) => style.margin.top = to_taffy_lp_auto(*d),
        StyleProp::MarginRight(d) => style.margin.right = to_taffy_lp_auto(*d),
        StyleProp::MarginBottom(d) => style.margin.bottom = to_taffy_lp_auto(*d),
        StyleProp::MarginLeft(d) => style.margin.left = to_taffy_lp_auto(*d),
        _ => return false,
    }
    true
}
