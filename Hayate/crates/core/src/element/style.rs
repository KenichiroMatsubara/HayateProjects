use crate::color::Color;

/// CSS box-shadow の1レイヤー（ADR-0095）。オフセット・blur・spread は CSS px、
/// `inset` は内側シャドウを選ぶ。`box-shadow` 値はこれの順序付きリスト
/// （CSS の描画順に合わせて先頭が最前面）。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shadow {
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur: f32,
    pub spread: f32,
    pub color: Color,
    pub inset: bool,
}

/// `element_unset_style` で解除するスタイルプロパティの指定。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StylePropKind {
    Color,
    FontSize,
    FontFamily,
    FontWeight,
}

/// スタイルバリアントのビューポート条件（ADR-0081）。
///
/// 各軸は px で AND 結合。`min_*` は `actual >= min_*`、`max_*` は
/// `actual <= max_*` の包含判定で、CSS `@media (min-width: ...)` /
/// `(max-width: ...)` 等に対応する。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewportCondition {
    pub min_width: Option<f32>,
    pub max_width: Option<f32>,
    pub min_height: Option<f32>,
    pub max_height: Option<f32>,
}

impl ViewportCondition {
    /// この条件が指定ビューポートサイズに一致するか。
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

/// ボーダーの線種（ADR-0083）。
///
/// `None` が既定で、明示的に style を設定したときだけボーダーを描く。
/// CSS の `border-style` が `none` 既定なのに対応する。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BorderStyleValue {
    None,
    Solid,
    Dashed,
}

/// ボックスの位置指定方式（ADR-0091）。
///
/// `Relative` が既定で Taffy の既定に一致する（既定が `static` の CSS とは異なる）。
/// `Absolute` は要素を通常フローから外し、`top`/`left`/`right`/`bottom` の
/// inset で位置決めする。`sticky` / `fixed` は対象外（Taffy 未対応）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PositionValue {
    Relative,
    Absolute,
}

/// ポインタカーソルの見た目（ADR-0088）。ポインタ下の要素から解決し、
/// `on_pointer_move` 経由で Platform Adapter に渡す。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorValue {
    Default,
    Pointer,
    Text,
    Crosshair,
    NotAllowed,
    Grab,
    Grabbing,
}

/// 要素テキストの選択可否。CSS `user-select` をモデル化（ADR-0108）。
///
/// `Text` はテキストが文書選択に参加する。`None` は自身（と部分木）を除外する。
/// `Contains` は選択可能だが、選択が越えられない包含境界を作る。明示的な
/// `user-select` 値から解決し、無ければ要素種別の UA 既定
/// （`default_user_select`）にフォールバックする。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UserSelectValue {
    Text,
    None,
    Contains,
}

/// 子要素のオーバーフロー処理（ADR-0090）。
///
/// `Visible` が既定で、子は要素ボックスの外側にも描画されうる
/// （CSS の `overflow` が `visible` 既定なのに対応）。`Hidden` は子を要素の
/// （角丸も含む）ボーダーボックスでクリップする。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverflowValue {
    Visible,
    Hidden,
}

/// 疑似状態トランジション補間のイージング関数（ADR-0089）。
///
/// `Ease` が CSS 既定。各バリアントは HTML モードでは対応する CSS
/// `transition-timing-function` キーワードに対応し、Canvas モードでは
/// 描画層の補間カーブを駆動する。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransitionTimingValue {
    Ease,
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
}

/// `max-lines` ブロックの最終可視行のテキスト切り詰め挙動（ADR-0090）。
///
/// `Clip` が既定で、`max-lines` を超えるテキストは無言で切られる。`Ellipsis`
/// は最終可視行に `…` を付ける。`text-overflow` は `max-lines` が設定されない
/// 限り効果がない。切り詰めの唯一のトリガーは `max-lines`。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextOverflowValue {
    Clip,
    Ellipsis,
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
    // 視覚
    BackgroundColor(Color),
    Opacity(f32),
    BorderRadius(f32),
    BorderWidth(f32),
    BorderColor(Color),
    BorderStyle(BorderStyleValue),
    BoxShadow(Vec<Shadow>),
    Overflow(OverflowValue),
    // サイズ
    Width(Dimension),
    Height(Dimension),
    MinWidth(Dimension),
    MinHeight(Dimension),
    MaxWidth(Dimension),
    MaxHeight(Dimension),
    AspectRatio(f32),
    // レイアウト
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
    // 位置指定
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
    // テキスト
    FontSize(f32),
    FontFamily(String),
    FontWeight(f32),
    Color(Color),
    FontStyle(FontStyleValue),
    TextDecoration(TextDecorationValue),
    // テキスト切り詰め（ADR-0090）
    MaxLines(u32),
    TextOverflow(TextOverflowValue),
    // ポインタ
    Cursor(CursorValue),
    // 既定テキストスタイル（ブロックを貫通する環境値）
    DefaultColor(Color),
    DefaultFontFamily(String),
    DefaultFontSize(f32),
    DefaultFontWeight(f32),
    // grid
    GridTemplateColumns(Vec<Dimension>),
    GridTemplateRows(Vec<Dimension>),
    // 重なり順
    ZIndex(i32),
    // トランジション（ADR-0089）
    TransitionDuration(f32),
    TransitionTiming(TransitionTimingValue),
}

impl StyleProp {
    /// レイアウトに影響するプロパティは Taffy へ、visual/text プロパティは Visual へ。
    pub fn is_layout(&self) -> bool {
        matches!(
            self,
            Self::Width(_)
                | Self::Height(_)
                | Self::MinWidth(_)
                | Self::MinHeight(_)
                | Self::MaxWidth(_)
                | Self::MaxHeight(_)
                | Self::AspectRatio(_)
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
