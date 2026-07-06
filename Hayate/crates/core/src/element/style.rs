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

/// 寸法（`width`/`min`/`max`）が指す箱（CSS `box-sizing`、ADR は #491 で確立）。
///
/// `BorderBox` は寸法に padding/border を含める（外形 = 指定寸法）。`ContentBox`
/// はコンテンツ箱を指し、padding/border は外側に足される（外形 = 寸法 + padding +
/// border）。レイアウト系なので Taffy の `box_sizing` へ流れる。CSS 既定は
/// `content-box` だが、明示宣言が無いときの UA 既定は要素種別側で決まる。
#[derive(Clone, Copy, Debug)]
pub enum BoxSizingValue {
    BorderBox,
    ContentBox,
}

/// Grid の自動配置方向と詰め方（CSS `grid-auto-flow`、ADR は #493 で確立）。
///
/// `Row`/`Column` は暗黙配置の主軸（行を端から埋めるか列を端から埋めるか）を選ぶ。
/// `*Dense` は dense 詰めを有効にし、後から来る小さいアイテムで前方の穴を埋め直す
/// （疎配置では穴を残す）。レイアウト系なので Taffy の `grid_auto_flow` へ流れる。
#[derive(Clone, Copy, Debug)]
pub enum GridAutoFlowValue {
    Row,
    Column,
    RowDense,
    ColumnDense,
}

/// Grid アイテムの1軸ぶんの配置端（CSS `grid-column` / `grid-row` の start/end の
/// 片側、ADR は #495 で確立）。
///
/// `Auto` は自動配置に委ねる。`Line(i)` は明示グリッド線 `i`（1 始まり、負値は
/// 末尾から数える）に置く。`Span(n)` は `n` トラックぶん占有する。レイアウト系の
/// 値型なので解決後の `Visual` には入らず Taffy の `GridPlacement` へ流れる。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridLineValue {
    Auto,
    Line(i32),
    Span(u32),
}

impl GridLineValue {
    /// 種別タグの wire 値（`0=auto` / `1=line` / `2=span`）。
    pub fn wire_kind(self) -> f32 {
        match self {
            GridLineValue::Auto => 0.0,
            GridLineValue::Line(_) => 1.0,
            GridLineValue::Span(_) => 2.0,
        }
    }

    /// 整数ペイロードの wire 値（`auto` は 0）。
    pub fn wire_value(self) -> f32 {
        match self {
            GridLineValue::Auto => 0.0,
            GridLineValue::Line(n) => n as f32,
            GridLineValue::Span(n) => n as f32,
        }
    }

    /// wire の `(種別タグ, 整数)` ペアから復元する。未知タグは `Auto`。
    pub fn from_wire(kind: f32, value: f32) -> Self {
        match kind as u32 {
            1 => GridLineValue::Line(value as i32),
            2 => GridLineValue::Span(value as u32),
            _ => GridLineValue::Auto,
        }
    }

    /// CSS `grid-column` / `grid-row` 端の文字列形（`auto` / `2` / `span 2`）。
    pub fn to_css(self) -> String {
        match self {
            GridLineValue::Auto => "auto".to_string(),
            GridLineValue::Line(n) => n.to_string(),
            GridLineValue::Span(n) => format!("span {n}"),
        }
    }
}

/// Grid アイテムの1軸ぶんの配置（CSS `grid-column` / `grid-row`、ADR は #495 で
/// 確立）。`start` / `end` の2スロットを持ち、各々 `auto` / `line` / `span`。
/// レイアウト系なので Taffy の `grid_column` / `grid_row`（`Line`）へ流れる。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GridPlacementValue {
    pub start: GridLineValue,
    pub end: GridLineValue,
}

impl GridPlacementValue {
    /// `start` だけを指定し `end` は `auto` にする（CSS `grid-column: <start>`）。
    pub fn start(start: GridLineValue) -> Self {
        Self {
            start,
            end: GridLineValue::Auto,
        }
    }

    /// `<start> / <end>` の両端指定。
    pub fn new(start: GridLineValue, end: GridLineValue) -> Self {
        Self { start, end }
    }

    /// 明示線 `i` から開始（CSS `grid-column: i`）。
    pub fn line(i: i32) -> Self {
        Self::start(GridLineValue::Line(i))
    }

    /// `n` トラックぶん占有（CSS `grid-column: span n`）。
    pub fn span(n: u32) -> Self {
        Self::start(GridLineValue::Span(n))
    }
}

/// Grid セル内のインライン軸（既定では水平）でのアイテム整列のコンテナ既定
/// （CSS `justify-items`、ADR は #494 で確立）。
///
/// `Start`/`End`/`Center` はセル内の該当端へ寄せ、`Stretch` はセル幅いっぱいに
/// 伸ばす（明示幅があれば伸びない）。レイアウト系なので Taffy の `justify_items`
/// （`Option`）へ流れる。flex 用の `align_items` と違い grid 専用の語彙で、
/// `flex-start`/`flex-end` ではなく `start`/`end` を使う。
#[derive(Clone, Copy, Debug)]
pub enum JustifyItemsValue {
    Start,
    End,
    Center,
    Stretch,
}

/// Grid アイテム個別のインライン軸整列。コンテナの `justify-items` を上書きする
/// （CSS `justify-self`、ADR は #494 で確立）。
///
/// `Auto` はコンテナ既定に従う（Taffy では `None`）。それ以外は `justify-items`
/// と同じ意味。レイアウト系なので Taffy の `justify_self`（`Option`）へ流れる。
#[derive(Clone, Copy, Debug)]
pub enum JustifySelfValue {
    Auto,
    Start,
    End,
    Center,
    Stretch,
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
    /// draw display list（decode 済みコマンド列・#724 / ADR-0141）。`view` 限定
    /// （`element_kinds.carriesDraw`）。空リストは「描画なし」。`Arc` 共有により
    /// `Visual` の clone / effective 解決を通じてコピーしない。
    Draw(std::sync::Arc<Vec<crate::wire::protocol::DrawCommand>>),
    // サイズ
    Width(Dimension),
    Height(Dimension),
    MinWidth(Dimension),
    MinHeight(Dimension),
    MaxWidth(Dimension),
    MaxHeight(Dimension),
    AspectRatio(f32),
    BoxSizing(BoxSizingValue),
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
    // grid の暗黙トラックサイズ
    GridAutoRows(Vec<Dimension>),
    GridAutoColumns(Vec<Dimension>),
    // grid の自動配置方向・詰め方
    GridAutoFlow(GridAutoFlowValue),
    // grid アイテムの明示配置（CSS `grid-column` / `grid-row`）
    GridColumn(GridPlacementValue),
    GridRow(GridPlacementValue),
    // grid セル内のインライン軸整列（コンテナ既定／アイテム上書き）
    JustifyItems(JustifyItemsValue),
    JustifySelf(JustifySelfValue),
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
                | Self::BoxSizing(_)
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
                | Self::GridAutoRows(_)
                | Self::GridAutoColumns(_)
                | Self::GridAutoFlow(_)
                | Self::GridColumn(_)
                | Self::GridRow(_)
                | Self::JustifyItems(_)
                | Self::JustifySelf(_)
        )
    }
}
