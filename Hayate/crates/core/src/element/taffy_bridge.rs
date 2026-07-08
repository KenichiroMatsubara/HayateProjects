use taffy::{
    style_helpers::{fr, length, line, percent, FromFr, FromLength, FromPercent, TaffyAuto},
    AlignContent, AlignItems, BoxSizing, Dimension as TaffyDim, Display, FlexDirection, FlexWrap,
    GridAutoFlow, GridPlacement, JustifyContent, LengthPercentage, LengthPercentageAuto, Line,
    Position, Rect as TaffyRect, Size, Style,
};

use crate::element::id::ElementId;
use crate::element::style::{
    AlignContentValue, AlignSelfValue, AlignValue, BoxSizingValue, Dimension, DimensionUnit,
    DisplayValue, FlexDirectionValue, FlexWrapValue, GridAutoFlowValue, GridLineValue,
    GridPlacementValue, JustifyItemsValue, JustifySelfValue, JustifyValue, OverflowValue,
    PositionValue, StyleProp,
};

/// Hayate の grid 配置端を Taffy の `GridPlacement` へ写す。`Line(i)` は CSS の
/// 1 始まりグリッド線、`Span(n)` は占有トラック数。
fn to_taffy_grid_placement(value: GridLineValue) -> GridPlacement {
    match value {
        GridLineValue::Auto => GridPlacement::Auto,
        GridLineValue::Line(i) => line(i as i16),
        GridLineValue::Span(n) => GridPlacement::Span(n as u16),
    }
}

/// `grid-column` / `grid-row` 値を Taffy の `Line<GridPlacement>`（start/end）へ。
fn to_taffy_grid_line(placement: &GridPlacementValue) -> Line<GridPlacement> {
    Line {
        start: to_taffy_grid_placement(placement.start),
        end: to_taffy_grid_placement(placement.end),
    }
}

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
        DimensionUnit::Px => TaffyDim::length(d.value),
        // Hayate は percent を 0..100 で受けるが Taffy は 0..1 を期待する。
        DimensionUnit::Percent => TaffyDim::percent(d.value / 100.0),
        DimensionUnit::Auto => TaffyDim::AUTO,
        // Taffy `Dimension` はグリッドトラック以外で `Fr` 表現を持たないため
        // ここでは Auto にフォールバックする。
        DimensionUnit::Fr => TaffyDim::AUTO,
    }
}

fn to_taffy_lp(d: Dimension) -> LengthPercentage {
    match d.unit {
        DimensionUnit::Px => LengthPercentage::length(d.value),
        DimensionUnit::Percent => LengthPercentage::percent(d.value / 100.0),
        // Padding/gap は Auto を受け付けないため 0 にクランプする。
        DimensionUnit::Auto | DimensionUnit::Fr => LengthPercentage::length(0.0),
    }
}

/// Hayate の `dimension` を Taffy のトラックサイズ関数へ写す。明示トラック
/// (`grid-template-*`, `TrackSizingFunction`) と暗黙トラック (`grid-auto-*`,
/// `NonRepeatedTrackSizingFunction`) の双方を同じ語彙で賄えるよう、`fr` / `length`
/// / `percent` / `Auto` を出力型ジェネリックで構築する。
fn to_taffy_track<T: FromLength + FromPercent + FromFr + TaffyAuto>(d: Dimension) -> T {
    match d.unit {
        DimensionUnit::Px => length(d.value),
        DimensionUnit::Percent => percent(d.value / 100.0),
        DimensionUnit::Fr => fr(d.value),
        DimensionUnit::Auto => T::AUTO,
    }
}

fn to_taffy_lp_auto(d: Dimension) -> LengthPercentageAuto {
    match d.unit {
        DimensionUnit::Px => LengthPercentageAuto::length(d.value),
        DimensionUnit::Percent => LengthPercentageAuto::percent(d.value / 100.0),
        DimensionUnit::Auto => LengthPercentageAuto::AUTO,
        DimensionUnit::Fr => LengthPercentageAuto::AUTO,
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
        // aspect-ratio は width / height（Taffy も同じ向き）。負/非正は無効として無視。
        StyleProp::AspectRatio(v) => {
            style.aspect_ratio = if *v > 0.0 { Some(*v) } else { None };
        }
        // box-sizing は寸法が指す箱を選ぶ。Taffy の box_sizing へそのまま写す。
        StyleProp::BoxSizing(v) => {
            style.box_sizing = match v {
                BoxSizingValue::BorderBox => BoxSizing::BorderBox,
                BoxSizingValue::ContentBox => BoxSizing::ContentBox,
            };
        }
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
                AlignValue::FlexStart => AlignItems::FLEX_START,
                AlignValue::FlexEnd => AlignItems::FLEX_END,
                AlignValue::Center => AlignItems::CENTER,
                AlignValue::Stretch => AlignItems::STRETCH,
                AlignValue::Baseline => AlignItems::BASELINE,
            });
        }
        StyleProp::JustifyContent(v) => {
            style.justify_content = Some(match v {
                JustifyValue::FlexStart => JustifyContent::FLEX_START,
                JustifyValue::FlexEnd => JustifyContent::FLEX_END,
                JustifyValue::Center => JustifyContent::CENTER,
                JustifyValue::SpaceBetween => JustifyContent::SPACE_BETWEEN,
                JustifyValue::SpaceAround => JustifyContent::SPACE_AROUND,
                JustifyValue::SpaceEvenly => JustifyContent::SPACE_EVENLY,
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
                AlignSelfValue::FlexStart => Some(AlignItems::FLEX_START),
                AlignSelfValue::FlexEnd => Some(AlignItems::FLEX_END),
                AlignSelfValue::Center => Some(AlignItems::CENTER),
                AlignSelfValue::Stretch => Some(AlignItems::STRETCH),
                AlignSelfValue::Baseline => Some(AlignItems::BASELINE),
            };
        }
        StyleProp::AlignContent(v) => {
            style.align_content = Some(match v {
                AlignContentValue::FlexStart => AlignContent::FLEX_START,
                AlignContentValue::FlexEnd => AlignContent::FLEX_END,
                AlignContentValue::Center => AlignContent::CENTER,
                AlignContentValue::Stretch => AlignContent::STRETCH,
                AlignContentValue::SpaceBetween => AlignContent::SPACE_BETWEEN,
                AlignContentValue::SpaceAround => AlignContent::SPACE_AROUND,
                AlignContentValue::SpaceEvenly => AlignContent::SPACE_EVENLY,
            });
        }
        StyleProp::GridTemplateColumns(tracks) => {
            style.grid_template_columns = tracks.iter().copied().map(to_taffy_track).collect();
        }
        StyleProp::GridTemplateRows(tracks) => {
            style.grid_template_rows = tracks.iter().copied().map(to_taffy_track).collect();
        }
        // 明示トラックを超えて生成される暗黙の行/列のサイズ。Taffy の
        // `grid_auto_*`（`NonRepeatedTrackSizingFunction`）へ同じ語彙で写す。
        StyleProp::GridAutoRows(tracks) => {
            style.grid_auto_rows = tracks.iter().copied().map(to_taffy_track).collect();
        }
        StyleProp::GridAutoColumns(tracks) => {
            style.grid_auto_columns = tracks.iter().copied().map(to_taffy_track).collect();
        }
        // 自動配置の主軸と dense 詰め。Taffy の `grid_auto_flow` へそのまま写す。
        StyleProp::GridAutoFlow(v) => {
            style.grid_auto_flow = match v {
                GridAutoFlowValue::Row => GridAutoFlow::Row,
                GridAutoFlowValue::Column => GridAutoFlow::Column,
                GridAutoFlowValue::RowDense => GridAutoFlow::RowDense,
                GridAutoFlowValue::ColumnDense => GridAutoFlow::ColumnDense,
            };
        }
        // grid アイテムの明示配置。start/end を Taffy の Line<GridPlacement> へ写す。
        StyleProp::GridColumn(p) => {
            style.grid_column = to_taffy_grid_line(p);
        }
        StyleProp::GridRow(p) => {
            style.grid_row = to_taffy_grid_line(p);
        }
        // grid セル内インライン軸のコンテナ既定。grid 専用なので start/end を使う。
        StyleProp::JustifyItems(v) => {
            style.justify_items = Some(match v {
                JustifyItemsValue::Start => AlignItems::START,
                JustifyItemsValue::End => AlignItems::END,
                JustifyItemsValue::Center => AlignItems::CENTER,
                JustifyItemsValue::Stretch => AlignItems::STRETCH,
            });
        }
        // grid アイテム個別のインライン軸整列。`auto` は None（コンテナ既定に従う）。
        StyleProp::JustifySelf(v) => {
            style.justify_self = match v {
                JustifySelfValue::Auto => None,
                JustifySelfValue::Start => Some(AlignItems::START),
                JustifySelfValue::End => Some(AlignItems::END),
                JustifySelfValue::Center => Some(AlignItems::CENTER),
                JustifySelfValue::Stretch => Some(AlignItems::STRETCH),
            };
        }
        _ => return false,
    }
    true
}
