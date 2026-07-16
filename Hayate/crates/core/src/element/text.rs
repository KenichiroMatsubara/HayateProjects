use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::Arc;

use fontique::{Collection, FontInfoOverride, FontStyle, GenericFamily};
use linebender_resource_handle::Blob;
use parley::{
    FontContext, FontFamily, FontWeight, Layout, LayoutContext, PositionedLayoutItem, StyleProperty,
};

use crate::element::font_coverage;
use crate::element::style::{FontStyleValue, TextDecorationValue, TextOverflowValue};

use skrifa::raw::{FontRef, TableProvider};

use crate::node::{TextDecorationLine, TextFontAttributes, TextFontSlant, TextRunData};
use crate::render::{text_synthesis, RenderFont, RenderGlyph};

/// 合成ボールド量の算出に使うフォントの `units_per_em`。head テーブルが読めない場合の
/// フォールバックは典型的なアウトラインフォントの 1000。
fn font_units_per_em(font: &RenderFont) -> u16 {
    FontRef::from_index(font.data.as_ref(), font.index)
        .ok()
        .and_then(|f| f.head().ok())
        .map(|head| head.units_per_em())
        .unwrap_or(1000)
}

/// Parley スタイルに保持するブラシ型。色は描画時に適用する。
pub type TextBrush = [u8; 4];

/// バンドルされたデフォルトフォントファミリ。システムフォントのない canvas(WASM)
/// モードでも常に利用可能。未知のフォント名は CSS フォントスタックでこれにフォールバックする。
pub const DEFAULT_FONT_FAMILY: &str = "Noto Sans";

/// `data` を `family_name` として `collection` に登録し、バンドルデフォルト自身でない
/// 限り、`sans-serif` ジェネリックへバンドルデフォルトの後ろに追加してクラスタ単位の
/// フォールバックとして組み込む。
///
/// オンデマンドのフォント読み込み([`crate::element::tree::ElementTree::register_font`])を支える。
/// 取得したフォントは2通りで到達可能でなければならない: 自身の名前で(CSS スタック
/// `"Inter, …"` が選べるように)、およびバンドルフォントに無いグリフ(絵文字・他言語)の
/// フォールバックとして。
///
/// 重要なのはバンドルデフォルトを隠してはならない点。以前は取得フォントをすべて
/// `DEFAULT_FONT_FAMILY` の別名にしており、そのファミリに競合フェイスを追加していた。
/// すると fontique が新たに取得した(例: Latin のみの Inter)フェイスを丸ごとのランに選び、
/// フォールバックを取得した途端に全 CJK グリフが `.notdef` に落ちた(テキストが正しく描画され
/// た後、最初の取得で □ になる豆腐化現象)。ジェネリックへ追加すればバンドルフェイスが先頭に
/// 残り、クラスタ単位で常に先に試され、取得フォントは本当の欠落だけを埋める。
pub fn register_collection_font(collection: &mut Collection, family_name: &str, data: Arc<Vec<u8>>) {
    let override_info = FontInfoOverride {
        family_name: Some(family_name),
        ..Default::default()
    };
    let registered = collection.register_fonts(Blob::new(data), Some(override_info));
    if family_name != DEFAULT_FONT_FAMILY {
        let ids: Vec<_> = registered.into_iter().map(|(id, _)| id).collect();
        collection.append_generic_families(GenericFamily::SansSerif, ids.into_iter());
    }
}

/// バイト範囲 → それを所有するインラインテキスト要素(検索時は最深がち勝つ)。
#[derive(Clone, Debug, Default)]
pub struct RangeMap {
    pub(crate) entries: Vec<(usize, usize, crate::element::id::ElementId)>,
}

impl RangeMap {
    pub fn insert(
        &mut self,
        byte_start: usize,
        byte_end: usize,
        id: crate::element::id::ElementId,
    ) {
        if byte_start < byte_end {
            self.entries.push((byte_start, byte_end, id));
        }
    }

    pub fn lookup(&self, byte: usize) -> Option<crate::element::id::ElementId> {
        self.entries
            .iter()
            .rev()
            .find(|(start, end, _)| byte >= *start && byte < *end)
            .map(|(_, _, id)| *id)
    }
}

/// 範囲指定 Parley シェーピング用のスタイル付きバイト範囲(ADR-0063)。
#[derive(Clone)]
pub struct RangedTextSpan {
    pub byte_start: usize,
    pub byte_end: usize,
    pub font_size: f32,
    pub font_weight: Option<f32>,
    pub font_family: Option<String>,
    pub font_style: Option<FontStyleValue>,
    pub text_decoration: Option<TextDecorationValue>,
    pub brush: TextBrush,
}

/// Element にキャッシュした Parley レイアウトと、ローワリング済みの Vello グリフラン。
pub struct TextLayout {
    pub layout: Layout<TextBrush>,
    pub runs: Vec<Arc<TextRunData>>,
    pub font_size: f32,
    pub text: Arc<str>,
    /// シェーピング中に .notdef グリフが検出されたフォントファミリ名。
    /// 各エントリは動的に読み込むべきフォントを示す。
    pub missing_families: Vec<&'static str>,
    /// IFC のバイト範囲 → インラインテキスト要素の所有者(ADR-0063)。
    pub range_map: Option<RangeMap>,
}

fn parley_font_style(value: FontStyleValue) -> FontStyle {
    match value {
        FontStyleValue::Normal => FontStyle::Normal,
        FontStyleValue::Italic => FontStyle::Italic,
        FontStyleValue::Oblique => FontStyle::Oblique(None),
    }
}

/// CSS のジェネリックファミリキーワードを Canvas Mode 用の具体的なフォント名へ解決する。
///
/// HTML Mode は値をそのままブラウザに渡し、ブラウザがジェネリックをネイティブに解決する。
/// Canvas Mode(Parley/Vello)は WASM でシステムフォントにアクセスできないため、ジェネリック
/// キーワードをバンドルまたはオンデマンド取得の Noto フォントへ対応付ける。
pub(crate) fn resolve_generic_family(name: &str) -> &str {
    match name {
        // sans-serif 系ジェネリック → デフォルト(バンドル済みの Noto Sans)
        "sans-serif" | "system-ui" | "ui-sans-serif" | "-apple-system" | "BlinkMacSystemFont"
        | "cursive" | "fantasy" | "ui-rounded" => DEFAULT_FONT_FAMILY,
        // serif → Noto Serif(builtin_font_url 経由でオンデマンド取得)
        "serif" | "ui-serif" => "Noto Serif",
        // monospace → Noto Sans Mono(オンデマンド取得)
        "monospace" | "ui-monospace" => "Noto Sans Mono",
        // 名前付き、または解決済みのファミリはそのまま通す
        other => other,
    }
}

/// CSS の `font-family` 値を、順序を保った解決済みエントリへ分割する。
///
/// `font-family` はスタック(例: `"Inter, Segoe UI, system-ui, sans-serif"`)。
/// カンマ区切りの各エントリをトリム(引用符除去)し、[`resolve_generic_family`] を通して
/// ジェネリックキーワードを具体的なバンドル/Noto 名にする。空エントリは捨てる。返すスライスは
/// `value` を借用するか、解決済みジェネリックでは `'static`。
///
/// 呼び出し側は Parley フォントスタックの構築と、先読み取得すべき名前付きファミリの判定の
/// 両方に使う。URL に対応付けられないカンマ文字列全体ではなく、各エントリを個別に取得する。
pub(crate) fn parse_font_family_list(value: &str) -> Vec<&str> {
    value
        .split(',')
        .map(|entry| entry.trim().trim_matches(['"', '\'']).trim())
        .filter(|entry| !entry.is_empty())
        .map(resolve_generic_family)
        .collect()
}

/// CSS の `font-family` 値から Parley フォントスタックを構築する。各エントリを解決し、
/// 末尾にバンドルデフォルトを終端フォールバックとして追加するため、未登録の名前はこれに
/// 退化する。値が空、またはデフォルト単体に解決される場合は素のデフォルトを返す。
pub(crate) fn build_font_stack(font_family: Option<&str>) -> Cow<'static, str> {
    match font_family {
        Some(f) if !f.is_empty() => {
            let mut stack = parse_font_family_list(f);
            if stack.last() != Some(&DEFAULT_FONT_FAMILY) {
                stack.push(DEFAULT_FONT_FAMILY);
            }
            if stack.as_slice() == [DEFAULT_FONT_FAMILY] {
                Cow::Borrowed(DEFAULT_FONT_FAMILY)
            } else {
                Cow::Owned(stack.join(", "))
            }
        }
        _ => Cow::Borrowed(DEFAULT_FONT_FAMILY),
    }
}

/// Parley レイアウトを構築し、行分割して、グリフランを Raw Layer 用の `TextRunData` へ
/// ローワリングする。
pub fn build_text_layout(
    font_cx: &mut FontContext,
    layout_cx: &mut LayoutContext<TextBrush>,
    text: &str,
    font_size: f32,
    max_advance: Option<f32>,
    font_family: Option<&str>,
    font_weight: Option<f32>,
    font_style: Option<FontStyleValue>,
) -> TextLayout {
    let mut builder = layout_cx.ranged_builder(font_cx, text, 1.0, true);
    builder.push_default(StyleProperty::FontSize(font_size));
    if let Some(weight) = font_weight {
        builder.push_default(StyleProperty::FontWeight(FontWeight::new(weight)));
    }
    if let Some(style) = font_style {
        builder.push_default(StyleProperty::FontStyle(parley_font_style(style)));
    }
    // ジェネリックキーワードを解決し、未知の名前がバンドルデフォルトにフォールバックする
    // よう CSS フォントスタックを構築する。Parley は左から順に解決して未登録名を黙って
    // スキップし、欠落分には FetchFont を発火する。
    let stack = build_font_stack(font_family);
    builder.push_default(StyleProperty::FontFamily(FontFamily::Source(stack)));
    let mut layout: Layout<TextBrush> = builder.build(text);
    layout.break_all_lines(max_advance);

    let (runs, missing_families) = lower_glyph_runs(&layout, font_size, text);
    TextLayout {
        layout,
        runs,
        font_size,
        text: Arc::<str>::from(text),
        missing_families,
        range_map: None,
    }
}

/// ブラウザ `<input>` のデフォルト `size`(文字数)。明示的な `width` を持たない
/// `text-input` の UA デフォルトコンテンツ幅は、解決済みフォントで計ったこの文字数分の
/// 幅になる(ADR-0109)。インラインのマジックナンバーではない。
pub const TEXT_INPUT_DEFAULT_SIZE_CHARS: usize = 20;

/// 幅未指定の `text-input` の UA デフォルトコンテンツ幅を測る。解決済みフォントでの
/// [`TEXT_INPUT_DEFAULT_SIZE_CHARS`] 個の `"0"` グリフのアドバンス。フォントに追従し
/// (font-size に依存)、入力自身のテキスト/プレースホルダには依存しない。フィールド幅を
/// 固定し、内容に合わせて伸びずスクロールするブラウザ `<input size>` デフォルトを反映する。
pub(crate) fn text_input_default_width(
    font_cx: &mut FontContext,
    layout_cx: &mut LayoutContext<TextBrush>,
    font_size: f32,
    font_family: Option<&str>,
    font_weight: Option<f32>,
    font_style: Option<FontStyleValue>,
) -> f32 {
    let sample: String = "0".repeat(TEXT_INPUT_DEFAULT_SIZE_CHARS);
    let layout = build_text_layout(
        font_cx, layout_cx, &sample, font_size, None, font_family, font_weight, font_style,
    );
    layout.layout.width()
}

/// バイト範囲ごとのスタイルを持つ Parley レイアウトを構築し、行分割する。
/// 公開 IFC エントリポイントと `max-lines` の再シェーピングパスで共有する。
fn build_broken_ranged_layout(
    font_cx: &mut FontContext,
    layout_cx: &mut LayoutContext<TextBrush>,
    text: &str,
    spans: &[RangedTextSpan],
    max_advance: Option<f32>,
) -> Layout<TextBrush> {
    let default_font_size = spans.first().map(|s| s.font_size).unwrap_or(16.0);
    let mut builder = layout_cx.ranged_builder(font_cx, text, 1.0, true);
    builder.push_default(StyleProperty::FontSize(default_font_size));
    builder.push_default(StyleProperty::FontFamily(FontFamily::Source(
        std::borrow::Cow::Borrowed(DEFAULT_FONT_FAMILY),
    )));

    for span in spans {
        let range = span.byte_start..span.byte_end;
        builder.push(StyleProperty::FontSize(span.font_size), range.clone());
        if let Some(weight) = span.font_weight {
            builder.push(
                StyleProperty::FontWeight(FontWeight::new(weight)),
                range.clone(),
            );
        }
        if let Some(ref fam) = span.font_family {
            let stack = build_font_stack(Some(fam));
            builder.push(
                StyleProperty::FontFamily(FontFamily::Source(stack)),
                range.clone(),
            );
        }
        if let Some(style) = span.font_style {
            builder.push(
                StyleProperty::FontStyle(parley_font_style(style)),
                range.clone(),
            );
        }
        if let Some(dec) = span.text_decoration {
            match dec {
                TextDecorationValue::Underline => {
                    builder.push(StyleProperty::Underline(true), range.clone());
                }
                TextDecorationValue::LineThrough => {
                    builder.push(StyleProperty::Strikethrough(true), range.clone());
                }
                TextDecorationValue::None => {}
            }
        }
        builder.push(StyleProperty::Brush(span.brush), range);
    }

    let mut layout: Layout<TextBrush> = builder.build(text);
    layout.break_all_lines(max_advance);
    layout
}

/// バイト範囲ごとのスタイルを持つ Parley レイアウト(IFC / インラインテキスト)を構築し、
/// 必要なら `text-overflow` 処理付きで `max_lines` に切り詰める。
///
/// 切り詰めの唯一のトリガーは `max_lines`。これが無ければ `text_overflow` は無効。
/// `Ellipsis` は最終可視行に `…` を付ける(幅制約に収まるようトリム)、`Clip` は黙って切る。
pub fn build_ranged_text_layout(
    font_cx: &mut FontContext,
    layout_cx: &mut LayoutContext<TextBrush>,
    text: &str,
    spans: &[RangedTextSpan],
    max_advance: Option<f32>,
    max_lines: Option<u32>,
    text_overflow: TextOverflowValue,
) -> TextLayout {
    let default_font_size = spans.first().map(|s| s.font_size).unwrap_or(16.0);
    let mut layout = build_broken_ranged_layout(font_cx, layout_cx, text, spans, max_advance);

    let mut truncated_text: Option<String> = None;
    if let Some(max) = max_lines.map(|m| m as usize).filter(|m| *m >= 1) {
        if layout.lines().count() > max {
            let cut = layout
                .lines()
                .nth(max - 1)
                .map(|l| l.text_range().end)
                .unwrap_or(text.len())
                .min(text.len());
            let ellipsis = matches!(text_overflow, TextOverflowValue::Ellipsis);
            let (new_text, new_spans) = if ellipsis {
                fit_ellipsis(font_cx, layout_cx, text, spans, max_advance, max, cut)
            } else {
                let kept = text[..cut].trim_end();
                (kept.to_string(), clip_spans(spans, kept.len()))
            };
            layout = build_broken_ranged_layout(font_cx, layout_cx, &new_text, &new_spans, max_advance);
            truncated_text = Some(new_text);
        }
    }

    let final_text: &str = truncated_text.as_deref().unwrap_or(text);
    let (runs, missing_families) = lower_glyph_runs(&layout, default_font_size, final_text);
    TextLayout {
        layout,
        runs,
        font_size: default_font_size,
        text: Arc::<str>::from(final_text),
        missing_families,
        range_map: None,
    }
}

/// スタイル付きスパンを先頭 `len` バイトに切り詰める。完全に範囲外のものは捨てる。
fn clip_spans(spans: &[RangedTextSpan], len: usize) -> Vec<RangedTextSpan> {
    spans
        .iter()
        .filter(|s| s.byte_start < len)
        .map(|s| {
            let mut clipped = s.clone();
            clipped.byte_end = clipped.byte_end.min(len);
            clipped
        })
        .filter(|s| s.byte_start < s.byte_end)
        .collect()
}

/// バイトオフセット `at` に省略記号(`…`)スパンを追加する。末尾スパンのテキストスタイルを
/// 継承するが、装飾は継承しない。
fn push_ellipsis_span(spans: &mut Vec<RangedTextSpan>, at: usize) {
    let ellipsis_len = '…'.len_utf8();
    let mut span = spans.last().cloned().unwrap_or(RangedTextSpan {
        byte_start: at,
        byte_end: at,
        font_size: 16.0,
        font_weight: None,
        font_family: None,
        font_style: None,
        text_decoration: None,
        brush: [0, 0, 0, 255],
    });
    span.byte_start = at;
    span.byte_end = at + ellipsis_len;
    span.text_decoration = None;
    spans.push(span);
}

fn prev_char_boundary(s: &str, mut idx: usize) -> usize {
    if idx == 0 {
        return 0;
    }
    idx -= 1;
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

/// `text-overflow: ellipsis` の切り詰めを構築する。`prefix + …` が `max` 行に収まる、
/// `text` の(`cut` までの)最長プレフィックスを求める。
fn fit_ellipsis(
    font_cx: &mut FontContext,
    layout_cx: &mut LayoutContext<TextBrush>,
    text: &str,
    spans: &[RangedTextSpan],
    max_advance: Option<f32>,
    max: usize,
    cut: usize,
) -> (String, Vec<RangedTextSpan>) {
    let mut keep = text[..cut].trim_end().len();
    loop {
        let candidate = format!("{}…", &text[..keep]);
        let mut candidate_spans = clip_spans(spans, keep);
        push_ellipsis_span(&mut candidate_spans, keep);
        let probe = build_broken_ranged_layout(font_cx, layout_cx, &candidate, &candidate_spans, max_advance);
        if keep == 0 || probe.lines().count() <= max {
            return (candidate, candidate_spans);
        }
        keep = text[..prev_char_boundary(text, keep)].trim_end().len();
    }
}

fn lower_glyph_runs(
    layout: &Layout<TextBrush>,
    font_size: f32,
    text: &str,
) -> (Vec<Arc<TextRunData>>, Vec<&'static str>) {
    let mut out: Vec<Arc<TextRunData>> = Vec::new();
    let mut missing: HashSet<&'static str> = HashSet::new();

    for line in layout.lines() {
        for item in line.items() {
            let PositionedLayoutItem::GlyphRun(grun) = item else {
                continue;
            };
            let run = grun.run();
            let baseline = grun.baseline();
            let offset = grun.offset();
            let font = RenderFont::from(run.font().clone());
            let positioned: Vec<RenderGlyph> = grun
                .glyphs()
                .scan(offset, |x, g| {
                    let glyph = RenderGlyph {
                        id: g.id,
                        x: *x + g.x,
                        y: baseline + g.y,
                    };
                    *x += g.advance;
                    Some(glyph)
                })
                .collect();
            if positioned.is_empty() {
                continue;
            }
            let style = grun.style();
            let metrics = run.metrics();
            let mut decorations = Vec::new();
            if let Some(underline) = &style.underline {
                let deco_offset = underline.offset.unwrap_or(metrics.underline_offset);
                let size = underline.size.unwrap_or(metrics.underline_size);
                if size > 0.0 {
                    decorations.push(TextDecorationLine {
                        x0: grun.offset(),
                        x1: grun.offset() + grun.advance(),
                        // フォントメトリクスのオフセットは y-up 座標でベースライン相対。y-down へ反転する。
                        y: grun.baseline() - deco_offset + size * 0.5,
                        thickness: size.max(1.0),
                    });
                }
            }
            if let Some(strike) = &style.strikethrough {
                let offset = strike.offset.unwrap_or(metrics.strikethrough_offset);
                let size = strike.size.unwrap_or(metrics.strikethrough_size);
                if size > 0.0 {
                    decorations.push(TextDecorationLine {
                        x0: grun.offset(),
                        x1: grun.offset() + grun.advance(),
                        y: grun.baseline() - offset + size * 0.5,
                        thickness: size.max(1.0),
                    });
                }
            }
            if positioned.iter().any(|g| g.id == 0) {
                let range = run.text_range();
                let end = range.end.min(text.len());
                if range.start < end {
                    for ch in text[range.start..end].chars() {
                        if let Some(fam) = font_coverage::family_for_codepoint(ch as u32) {
                            missing.insert(fam);
                        }
                    }
                }
            }
            let synthesis =
                text_synthesis::resolve_synthesis(&run.synthesis(), font_units_per_em(&font));
            let font_attrs = run.font_attrs();
            let slant = match font_attrs.style {
                FontStyle::Normal => TextFontSlant::Upright,
                FontStyle::Italic => TextFontSlant::Italic,
                FontStyle::Oblique(_) => TextFontSlant::Oblique,
            };
            out.push(Arc::new(TextRunData {
                font,
                font_size: run.font_size().max(font_size),
                font_attributes: TextFontAttributes {
                    weight: font_attrs.weight.value(),
                    width: font_attrs.width.ratio(),
                    slant,
                },
                glyphs: positioned,
                decorations,
                text: Arc::<str>::from(""),
                synthesis,
                normalized_coords: run.normalized_coords().to_vec(),
            }));
        }
    }
    (out, missing.into_iter().collect())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use fontique::{FontInfoOverride, FontStyle, GenericFamily};
    use linebender_resource_handle::Blob;
    use parley::{FontContext, LayoutContext, PositionedLayoutItem};

    use super::*;

    fn test_font_context() -> FontContext {
        let mut font_cx = FontContext::new();
        static NOTO_SANS_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSansJP.ttf");
        let blob = Blob::new(Arc::new(NOTO_SANS_BYTES));
        let override_info = FontInfoOverride {
            family_name: Some(DEFAULT_FONT_FAMILY),
            ..Default::default()
        };
        let registered = font_cx.collection.register_fonts(blob, Some(override_info));
        let family_ids: Vec<_> = registered.into_iter().map(|(id, _)| id).collect();
        if !family_ids.is_empty() {
            font_cx
                .collection
                .set_generic_families(GenericFamily::SansSerif, family_ids.into_iter());
        }
        font_cx
    }

    fn text_run_syntheses(layout: &TextLayout) -> Vec<fontique::Synthesis> {
        // 生の fontique synthesis（variation_settings / embolden / skew）はシェーピング層に
        // ある。lowered TextRunData は解決済みの TextSynthesis を運ぶため、ここでは parley
        // layout から raw synthesis を読む。
        layout
            .layout
            .lines()
            .flat_map(|line| line.items())
            .filter_map(|item| match item {
                PositionedLayoutItem::GlyphRun(grun) => Some(grun.run().synthesis()),
                _ => None,
            })
            .collect()
    }

    fn glyph_run_font_styles(layout: &TextLayout) -> Vec<FontStyle> {
        layout
            .layout
            .lines()
            .flat_map(|line| line.items())
            .filter_map(|item| {
                let PositionedLayoutItem::GlyphRun(grun) = item else {
                    return None;
                };
                Some(grun.run().font_attrs().style)
            })
            .collect()
    }

    #[test]
    fn build_text_layout_pushes_font_style_to_parley() {
        let mut font_cx = test_font_context();
        let mut layout_cx = LayoutContext::new();
        let tl = build_text_layout(
            &mut font_cx,
            &mut layout_cx,
            "Hello",
            16.0,
            None,
            None,
            None,
            Some(FontStyleValue::Italic),
        );
        let styles = glyph_run_font_styles(&tl);
        assert!(!styles.is_empty(), "expected shaped glyph runs");
        assert!(
            styles.iter().all(|s| *s == FontStyle::Italic),
            "expected italic font style on all runs, got {styles:?}"
        );
    }

    #[test]
    fn build_text_layout_preserves_italic_synthesis_on_text_run() {
        let mut font_cx = test_font_context();
        let mut layout_cx = LayoutContext::new();
        let tl = build_text_layout(
            &mut font_cx,
            &mut layout_cx,
            "Hello",
            16.0,
            None,
            None,
            None,
            Some(FontStyleValue::Italic),
        );
        let synths = text_run_syntheses(&tl);
        assert!(!synths.is_empty(), "expected shaped text runs");
        assert!(
            synths.iter().any(|s| s.skew() == Some(14.0)),
            "expected faux italic skew on bundled font, got {synths:?}"
        );
    }

    #[test]
    fn build_text_layout_preserves_wght_axis_for_intermediate_weight() {
        let mut font_cx = test_font_context();
        let mut layout_cx = LayoutContext::new();
        let regular = build_text_layout(
            &mut font_cx,
            &mut layout_cx,
            "Hello",
            16.0,
            None,
            None,
            Some(400.0),
            None,
        );
        let semibold = build_text_layout(
            &mut font_cx,
            &mut layout_cx,
            "Hello",
            16.0,
            None,
            None,
            Some(600.0),
            None,
        );
        let regular_coords = regular.runs.first().map(|r| r.normalized_coords.as_slice());
        let semibold_coords = semibold.runs.first().map(|r| r.normalized_coords.as_slice());
        assert_eq!(regular.runs.first().unwrap().font_attributes.weight, 400.0);
        assert_eq!(semibold.runs.first().unwrap().font_attributes.weight, 600.0);
        assert!(
            regular_coords.is_some() && semibold_coords.is_some(),
            "expected shaped text runs"
        );
        assert_ne!(
            regular_coords,
            semibold_coords,
            "font-weight 600 should change variable font coordinates"
        );
        assert!(
            text_run_syntheses(&semibold)
                .iter()
                .any(|s| !s.variation_settings().is_empty()),
            "expected wght variation synthesis for semibold"
        );
    }

    #[test]
    fn build_text_layout_uses_wght_not_embolden_for_bold_variable_font() {
        let mut font_cx = test_font_context();
        let mut layout_cx = LayoutContext::new();
        let tl = build_text_layout(
            &mut font_cx,
            &mut layout_cx,
            "Hello",
            16.0,
            None,
            None,
            Some(700.0),
            None,
        );
        for synth in text_run_syntheses(&tl) {
            assert!(
                !synth.embolden(),
                "variable font bold should use wght axis, not faux embolden"
            );
            assert!(
                !synth.variation_settings().is_empty(),
                "expected wght variation for bold on variable font"
            );
        }
    }

    #[test]
    fn build_ranged_text_layout_pushes_font_style_to_parley() {
        let mut font_cx = test_font_context();
        let mut layout_cx = LayoutContext::new();
        let spans = [RangedTextSpan {
            byte_start: 0,
            byte_end: 5,
            font_size: 16.0,
            font_weight: None,
            font_family: None,
            font_style: Some(FontStyleValue::Italic),
            text_decoration: None,
            brush: [0, 0, 0, 255],
        }];
        let tl = build_ranged_text_layout(
            &mut font_cx,
            &mut layout_cx,
            "Hello",
            &spans,
            None,
            None,
            TextOverflowValue::Clip,
        );
        let styles = glyph_run_font_styles(&tl);
        assert!(!styles.is_empty(), "expected shaped glyph runs");
        assert!(
            styles.iter().all(|s| *s == FontStyle::Italic),
            "expected italic font style on all runs, got {styles:?}"
        );
    }

    #[test]
    fn parse_font_family_list_splits_resolves_and_unquotes_entries() {
        // 空白・引用符・末尾のジェネリックはすべてエントリ単位で処理される。
        assert_eq!(
            parse_font_family_list("Inter, \"Segoe UI\" , system-ui, sans-serif"),
            vec!["Inter", "Segoe UI", DEFAULT_FONT_FAMILY, DEFAULT_FONT_FAMILY],
        );
        // 空エントリ(例: 末尾のカンマ)は捨てる。
        assert_eq!(parse_font_family_list("Inter,,"), vec!["Inter"]);
        assert!(parse_font_family_list("  ").is_empty());
    }

    #[test]
    fn build_font_stack_appends_default_as_terminal_fallback() {
        // 名前付きファミリは最終フォールバックとしてバンドルデフォルトを得る。
        assert_eq!(build_font_stack(Some("Inter")), "Inter, Noto Sans");
        // すでにデフォルトで終わるリストは重複しない。
        assert_eq!(build_font_stack(Some("Inter, sans-serif")), "Inter, Noto Sans");
        // sans-serif 単体は素のデフォルトに収束する。
        assert_eq!(build_font_stack(Some("sans-serif")), DEFAULT_FONT_FAMILY);
        assert_eq!(build_font_stack(None), DEFAULT_FONT_FAMILY);
    }
}
