//! Hayabusa の static style モデル（ADR-0010）。
//!
//! 初回デモは **static style のみ**（reactive style 束縛は禁止・pending-decisions P3）。要素に
//! 一度だけ適用するスタイルプロパティの閉じた集合で、`sink` の `set_style` op（`bind_text` の
//! ような binding にはしない）で `hayate_core` の要素ローカルインラインスタイル（Hayate CSS）へ
//! 落ちる。型は `ElementKind` と同じく**閉じた Hayabusa 語彙**で、`HayateSink` が core の
//! `StyleProp` へ写す（既定の self-contained ビルドは外部依存ゼロ・ADR-0006）。
//!
//! 範囲（tracer bullet）：レイアウト（flex・サイズ・余白・gap）と視覚（背景色・文字色・
//! フォントサイズ）の小さな部分集合。reactive style・`<style>` ブロックのセレクタ・scoped
//! style・border / shadow 等は後続。

/// 長さ（px / % / auto）。core の `Dimension` に写る。
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Length {
    Px(f32),
    Percent(f32),
    Auto,
}

/// 0..1 正規化の RGBA。core の `Color` に写る。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Rgba {
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Rgba { r, g, b, a }
    }
}

// `Display` / `FlexDirection` / `Align` / `Justify` は `hayabusa-style-vocab`（Hayate の
// proto/spec が正本・ADR-0011）から `build.rs` が生成する。手書きすると codegen 側の
// キーワード変換と語彙が二重管理になる。
include!(concat!(env!("OUT_DIR"), "/style_enums_generated.rs"));

/// static なスタイルプロパティ 1 件。要素へ一度だけ適用される（ADR-0010）。
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StyleProp {
    // サイズ・余白
    Width(Length),
    Height(Length),
    Padding(Length),
    Margin(Length),
    Gap(Length),
    // レイアウト
    Display(Display),
    FlexDirection(FlexDirection),
    AlignItems(Align),
    JustifyContent(Justify),
    // 視覚・テキスト
    BackgroundColor(Rgba),
    TextColor(Rgba),
    FontSize(f32),
}
