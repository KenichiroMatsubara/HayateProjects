use taffy::{
    style_helpers::{fr, length, percent, TaffyAuto},
    AlignContent, AlignItems, Dimension as TaffyDim, Display, FlexDirection, FlexWrap, JustifyContent,
    LengthPercentage, LengthPercentageAuto, Position, Rect as TaffyRect, Size, Style,
    TrackSizingFunction,
};

use crate::element::id::ElementId;
use crate::element::style::{
    AlignContentValue, AlignSelfValue, AlignValue, Dimension, DimensionUnit, DisplayValue,
    FlexDirectionValue, FlexWrapValue, JustifyValue, OverflowValue, PositionValue, StyleProp,
};

/// 各 Taffy リーフに付ける文脈。measure クロージャがこれで分岐する。
#[derive(Clone, Copy, Debug)]
pub enum MeasureCtx {
    Text(ElementId),
    /// `text-input` リーフ。明示的な `width` がないとき、フォント相対の UA 既定
    /// コンテンツ幅を供給する（ADR-0109 根本原因 A）。
    TextInput(ElementId),
    None,
}

fn to_taffy_dim(d: Dimension) -> TaffyDim {
    match d.unit {
        DimensionUnit::Px => TaffyDim::Length(d.value),
        // Hayate は percent を 0..100 で受けるが Taffy は 0..1 を期待する。
        DimensionUnit::Percent => TaffyDim::Percent(d.value / 100.0),
        DimensionUnit::Auto => TaffyDim::Auto,
        // Taffy `Dimension` はグリッドトラック以外で `Fr` 表現を持たないため
        // ここでは Auto にフォールバックする。
        DimensionUnit::Fr => TaffyDim::Auto,
    }
}

fn to_taffy_lp(d: Dimension) -> LengthPercentage {
    match d.unit {
        DimensionUnit::Px => LengthPercentage::Length(d.value),
        DimensionUnit::Percent => LengthPercentage::Percent(d.value / 100.0),
        // Padding/gap は Auto を受け付けないため 0 にクランプする。
        DimensionUnit::Auto | DimensionUnit::Fr => LengthPercentage::Length(0.0),
    }
}

fn to_taffy_track(d: Dimension) -> TrackSizingFunction {
    match d.unit {
        DimensionUnit::Px => length(d.value),
        DimensionUnit::Percent => percent(d.value / 100.0),
        DimensionUnit::Fr => fr(d.value),
        DimensionUnit::Auto => TrackSizingFunction::AUTO,
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

/// Hayate の `overflow` を Taffy へ写す。`Hidden` と（kind 既定の）`Scroll` は
/// どちらもスクロールコンテナで、Taffy は flex 自動最小サイズを 0 にするため、
/// コンテンツ/basis で溢れずに兄弟が残した空間まで縮む。`Visible` はコンテンツ
/// ベースの最小サイズ（CSS 既定）を保つ。`OverflowValue` に `Scroll` はなく、
/// scroll-view の kind 既定は `ElementKind::base_layout_style` で直接設定する。
fn to_taffy_overflow(v: OverflowValue) -> taffy::Overflow {
    match v {
        OverflowValue::Visible => taffy::Overflow::Visible,
        OverflowValue::Hidden => taffy::Overflow::Hidden,
    }
}

/// `overflow` を `taffy::Style` の両軸へ書く。二面性を持つ `overflow` プロップの
/// レイアウト側。視覚側（子のクリップ）は別途 `Visual` に適用され、ここは flex
/// スクロールコンテナの最小サイズだけを司る。汎用のレイアウト/視覚振り分けが
/// `overflow` を視覚として分類し続けられるよう `apply_to_style` から外し、
/// レイアウト効果は専用シーム（`LayoutPass::set_overflow`）経由で駆動する。
pub fn apply_overflow_to_style(style: &mut Style, v: OverflowValue) {
    let o = to_taffy_overflow(v);
    style.overflow = taffy::Point { x: o, y: o };
}

/// Hayate のスタイルプロップ1つを可変 taffy::Style に適用する。レイアウト
/// プロップで適用できたら true、そうでなければ false（呼び出し側は Visual へ回す）。
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
        StyleProp::FlexWrap(v) => {
            style.flex_wrap = match v {
                FlexWrapValue::Nowrap => FlexWrap::NoWrap,
                FlexWrapValue::Wrap => FlexWrap::Wrap,
                FlexWrapValue::WrapReverse => FlexWrap::WrapReverse,
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
            style.gap = Size {
                width: lp,
                height: lp,
            };
        }
        StyleProp::Padding(d) => {
            let lp = to_taffy_lp(*d);
            style.padding = TaffyRect {
                left: lp,
                right: lp,
                top: lp,
                bottom: lp,
            };
        }
        StyleProp::PaddingTop(d) => style.padding.top = to_taffy_lp(*d),
        StyleProp::PaddingRight(d) => style.padding.right = to_taffy_lp(*d),
        StyleProp::PaddingBottom(d) => style.padding.bottom = to_taffy_lp(*d),
        StyleProp::PaddingLeft(d) => style.padding.left = to_taffy_lp(*d),
        StyleProp::Margin(d) => {
            let lpa = to_taffy_lp_auto(*d);
            style.margin = TaffyRect {
                left: lpa,
                right: lpa,
                top: lpa,
                bottom: lpa,
            };
        }
        StyleProp::MarginTop(d) => style.margin.top = to_taffy_lp_auto(*d),
        StyleProp::MarginRight(d) => style.margin.right = to_taffy_lp_auto(*d),
        StyleProp::MarginBottom(d) => style.margin.bottom = to_taffy_lp_auto(*d),
        StyleProp::MarginLeft(d) => style.margin.left = to_taffy_lp_auto(*d),
        StyleProp::Position(v) => {
            style.position = match v {
                PositionValue::Relative => Position::Relative,
                PositionValue::Absolute => Position::Absolute,
            };
        }
        StyleProp::Top(d) => style.inset.top = to_taffy_lp_auto(*d),
        StyleProp::Left(d) => style.inset.left = to_taffy_lp_auto(*d),
        StyleProp::Right(d) => style.inset.right = to_taffy_lp_auto(*d),
        StyleProp::Bottom(d) => style.inset.bottom = to_taffy_lp_auto(*d),
        StyleProp::FlexGrow(v) => style.flex_grow = (*v).max(0.0),
        StyleProp::FlexShrink(v) => style.flex_shrink = (*v).max(0.0),
        StyleProp::FlexBasis(d) => style.flex_basis = to_taffy_dim(*d),
        StyleProp::AlignSelf(v) => {
            style.align_self = match v {
                AlignSelfValue::Auto => None,
                AlignSelfValue::FlexStart => Some(AlignItems::FlexStart),
                AlignSelfValue::FlexEnd => Some(AlignItems::FlexEnd),
                AlignSelfValue::Center => Some(AlignItems::Center),
                AlignSelfValue::Stretch => Some(AlignItems::Stretch),
                AlignSelfValue::Baseline => Some(AlignItems::Baseline),
            };
        }
        StyleProp::AlignContent(v) => {
            style.align_content = Some(match v {
                AlignContentValue::FlexStart => AlignContent::FlexStart,
                AlignContentValue::FlexEnd => AlignContent::FlexEnd,
                AlignContentValue::Center => AlignContent::Center,
                AlignContentValue::Stretch => AlignContent::Stretch,
                AlignContentValue::SpaceBetween => AlignContent::SpaceBetween,
                AlignContentValue::SpaceAround => AlignContent::SpaceAround,
                AlignContentValue::SpaceEvenly => AlignContent::SpaceEvenly,
            });
        }
        StyleProp::GridTemplateColumns(tracks) => {
            style.grid_template_columns = tracks.iter().copied().map(to_taffy_track).collect();
        }
        StyleProp::GridTemplateRows(tracks) => {
            style.grid_template_rows = tracks.iter().copied().map(to_taffy_track).collect();
        }
        _ => return false,
    }
    true
}
