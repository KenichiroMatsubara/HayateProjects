use crate::color::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DimensionUnit {
    Px,
    Percent,
    Auto,
    Fr,
}

#[derive(Clone, Copy, Debug)]
pub struct Dimension {
    pub value: f32,
    pub unit: DimensionUnit,
}

impl Dimension {
    pub const AUTO: Self = Self { value: 0.0, unit: DimensionUnit::Auto };

    pub const fn px(value: f32) -> Self {
        Self { value, unit: DimensionUnit::Px }
    }

    pub const fn percent(value: f32) -> Self {
        Self { value, unit: DimensionUnit::Percent }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum DisplayValue {
    Flex,
    Grid,
    Block,
    None,
}

#[derive(Clone, Copy, Debug)]
pub enum FlexDirectionValue {
    Row,
    Column,
    RowReverse,
    ColumnReverse,
}

#[derive(Clone, Copy, Debug)]
pub enum AlignValue {
    FlexStart,
    FlexEnd,
    Center,
    Stretch,
    Baseline,
}

#[derive(Clone, Copy, Debug)]
pub enum JustifyValue {
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

#[derive(Clone, Copy, Debug)]
pub enum StyleProp {
    // visual
    BackgroundColor(Color),
    Opacity(f32),
    BorderRadius(f32),
    BorderWidth(f32),
    BorderColor(Color),
    // sizing
    Width(Dimension),
    Height(Dimension),
    MinWidth(Dimension),
    MinHeight(Dimension),
    MaxWidth(Dimension),
    MaxHeight(Dimension),
    // layout
    Display(DisplayValue),
    FlexDirection(FlexDirectionValue),
    AlignItems(AlignValue),
    JustifyContent(JustifyValue),
    Gap(Dimension),
    Padding(Dimension),
    PaddingTop(Dimension),
    PaddingRight(Dimension),
    PaddingBottom(Dimension),
    PaddingLeft(Dimension),
    Margin(Dimension),
    MarginTop(Dimension),
    MarginRight(Dimension),
    MarginBottom(Dimension),
    MarginLeft(Dimension),
    // text
    FontSize(f32),
    Color(Color),
    // stacking
    ZIndex(i32),
}

impl StyleProp {
    /// Layout-affecting props go to Taffy; visual/text props go to Visual.
    pub fn is_layout(self) -> bool {
        matches!(
            self,
            Self::Width(_)
                | Self::Height(_)
                | Self::MinWidth(_)
                | Self::MinHeight(_)
                | Self::MaxWidth(_)
                | Self::MaxHeight(_)
                | Self::Display(_)
                | Self::FlexDirection(_)
                | Self::AlignItems(_)
                | Self::JustifyContent(_)
                | Self::Gap(_)
                | Self::Padding(_)
                | Self::PaddingTop(_)
                | Self::PaddingRight(_)
                | Self::PaddingBottom(_)
                | Self::PaddingLeft(_)
                | Self::Margin(_)
                | Self::MarginTop(_)
                | Self::MarginRight(_)
                | Self::MarginBottom(_)
                | Self::MarginLeft(_)
        )
    }
}
