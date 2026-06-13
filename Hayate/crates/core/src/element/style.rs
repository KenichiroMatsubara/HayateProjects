use crate::color::Color;

/// Identifies which style property to unset via `element_unset_style`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StylePropKind {
    Color,
    FontSize,
    FontFamily,
    FontWeight,
}

/// Viewport-based condition for a style variant (ADR-0081).
///
/// All axes are in px and AND-combined; `min_*` match inclusively
/// (`actual >= min_*`) and `max_*` match inclusively (`actual <= max_*`),
/// mirroring CSS `@media (min-width: ...)` / `(max-width: ...)` etc.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewportCondition {
    pub min_width: Option<f32>,
    pub max_width: Option<f32>,
    pub min_height: Option<f32>,
    pub max_height: Option<f32>,
}

impl ViewportCondition {
    /// Whether this condition matches the given viewport size.
    pub fn matches(&self, viewport_width: f32, viewport_height: f32) -> bool {
        let min_width_ok = self.min_width.is_none_or(|v| viewport_width >= v);
        let max_width_ok = self.max_width.is_none_or(|v| viewport_width <= v);
        let min_height_ok = self.min_height.is_none_or(|v| viewport_height >= v);
        let max_height_ok = self.max_height.is_none_or(|v| viewport_height <= v);
        min_width_ok && max_width_ok && min_height_ok && max_height_ok
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontStyleValue {
    Normal,
    Italic,
    Oblique,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextDecorationValue {
    None,
    Underline,
    LineThrough,
}

/// Border line style (ADR-0083 module-complete border vocabulary, issue #204).
///
/// `None` is the default: a border is only drawn when an explicit style is set,
/// mirroring CSS where `border-style` defaults to `none`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BorderStyleValue {
    None,
    Solid,
    Dashed,
}

/// Box positioning scheme (ADR-0091, issue #205).
///
/// `Relative` is the default and matches Taffy's default (in contrast to CSS,
/// where `static` is the default). `Absolute` takes the element out of normal
/// flow; `top`/`left`/`right`/`bottom` insets then position it. `sticky` /
/// `fixed` are out of scope (Taffy has no support).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PositionValue {
    Relative,
    Absolute,
}

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
    pub const AUTO: Self = Self {
        value: 0.0,
        unit: DimensionUnit::Auto,
    };

    pub const fn px(value: f32) -> Self {
        Self {
            value,
            unit: DimensionUnit::Px,
        }
    }

    pub const fn percent(value: f32) -> Self {
        Self {
            value,
            unit: DimensionUnit::Percent,
        }
    }

    pub const fn fr(value: f32) -> Self {
        Self {
            value,
            unit: DimensionUnit::Fr,
        }
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
pub enum FlexWrapValue {
    Nowrap,
    Wrap,
    WrapReverse,
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
pub enum AlignSelfValue {
    Auto,
    FlexStart,
    FlexEnd,
    Center,
    Stretch,
    Baseline,
}

#[derive(Clone, Copy, Debug)]
pub enum AlignContentValue {
    FlexStart,
    FlexEnd,
    Center,
    Stretch,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

#[derive(Clone, Debug)]
pub enum StyleProp {
    // visual
    BackgroundColor(Color),
    Opacity(f32),
    BorderRadius(f32),
    BorderWidth(f32),
    BorderColor(Color),
    BorderStyle(BorderStyleValue),
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
    FlexWrap(FlexWrapValue),
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
    // positioning
    Position(PositionValue),
    Top(Dimension),
    Left(Dimension),
    Right(Dimension),
    Bottom(Dimension),
    // flex
    FlexGrow(f32),
    FlexShrink(f32),
    FlexBasis(Dimension),
    AlignSelf(AlignSelfValue),
    AlignContent(AlignContentValue),
    // text
    FontSize(f32),
    FontFamily(String),
    FontWeight(f32),
    Color(Color),
    FontStyle(FontStyleValue),
    TextDecoration(TextDecorationValue),
    // ambient default text style (block-penetrating)
    DefaultColor(Color),
    DefaultFontFamily(String),
    DefaultFontSize(f32),
    DefaultFontWeight(f32),
    // grid
    GridTemplateColumns(Vec<Dimension>),
    GridTemplateRows(Vec<Dimension>),
    // stacking
    ZIndex(i32),
}

impl StyleProp {
    /// Layout-affecting props go to Taffy; visual/text props go to Visual.
    pub fn is_layout(&self) -> bool {
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
                | Self::FlexWrap(_)
                | Self::AlignItems(_)
                | Self::JustifyContent(_)
                | Self::Gap(_)
                | Self::FlexGrow(_)
                | Self::FlexShrink(_)
                | Self::FlexBasis(_)
                | Self::AlignSelf(_)
                | Self::AlignContent(_)
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
                | Self::Position(_)
                | Self::Top(_)
                | Self::Left(_)
                | Self::Right(_)
                | Self::Bottom(_)
                | Self::GridTemplateColumns(_)
                | Self::GridTemplateRows(_)
        )
    }
}
