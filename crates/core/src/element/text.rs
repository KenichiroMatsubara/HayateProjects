use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::Arc;

use parley::{
    FontContext, FontFamily, Layout, LayoutContext, PositionedLayoutItem, StyleProperty,
};
use vello::Glyph;

use crate::node::TextRunData;

/// Brush type stored in Parley styles; color is applied at draw time.
pub type TextBrush = [u8; 4];

/// The bundled default font family. Always available in canvas (WASM) mode where
/// system fonts are absent. Unknown font names fall back to this via CSS font stack.
pub const DEFAULT_FONT_FAMILY: &str = "Noto Sans";

/// A Parley layout cached on an Element, plus the lowered Vello glyph runs.
pub struct TextLayout {
    pub layout: Layout<TextBrush>,
    pub runs: Vec<Arc<TextRunData>>,
    pub font_size: f32,
    pub text: Arc<str>,
    /// Width constraint last used; if None, single-line.
    pub width_constraint: Option<f32>,
    /// Font family names with .notdef glyphs detected during shaping.
    /// Each entry indicates a font that should be dynamically loaded.
    pub missing_families: Vec<&'static str>,
}

/// Map a Unicode codepoint to the font family name best suited to render it,
/// for use when .notdef is detected. Returns `None` for codepoints the
/// bundled default font is expected to cover.
///
/// Family names here are the keys each platform adapter uses in its own
/// family-name → font-source table (ADR-0043).
fn codepoint_font_family(cp: u32) -> Option<&'static str> {
    match cp {
        // ── CJK (Japanese, Chinese ideographs) ───────────────────────────
        0x2E80..=0x2EFF   // CJK Radicals Supplement
        | 0x2F00..=0x2FDF // Kangxi Radicals
        | 0x3000..=0x303F // CJK Symbols and Punctuation
        | 0x3040..=0x309F // Hiragana
        | 0x30A0..=0x30FF // Katakana
        | 0x31F0..=0x31FF // Katakana Phonetic Extensions
        | 0x3400..=0x4DBF // CJK Unified Ideographs Extension A
        | 0x4E00..=0x9FFF // CJK Unified Ideographs
        | 0xF900..=0xFAFF // CJK Compatibility Ideographs
        | 0x20000..=0x2A6DF // CJK Unified Ideographs Extension B
        | 0x2A700..=0x2B73F // CJK Unified Ideographs Extension C
        | 0x2B740..=0x2B81F // CJK Unified Ideographs Extension D
        | 0x2B820..=0x2CEAF // CJK Unified Ideographs Extension E
        | 0x2CEB0..=0x2EBEF // CJK Unified Ideographs Extension F
        => Some("Noto Sans JP"),

        // ── Korean ───────────────────────────────────────────────────────
        0x1100..=0x11FF   // Hangul Jamo
        | 0x3130..=0x318F // Hangul Compatibility Jamo
        | 0xA960..=0xA97F // Hangul Jamo Extended-A
        | 0xAC00..=0xD7AF // Hangul Syllables
        | 0xD7B0..=0xD7FF // Hangul Jamo Extended-B
        => Some("Noto Sans KR"),

        // ── Arabic ───────────────────────────────────────────────────────
        0x0600..=0x06FF   // Arabic
        | 0x0750..=0x077F // Arabic Supplement
        | 0x08A0..=0x08FF // Arabic Extended-A
        | 0xFB50..=0xFDFF // Arabic Presentation Forms-A
        | 0xFE70..=0xFEFF // Arabic Presentation Forms-B
        => Some("Noto Sans Arabic"),

        // ── Thai ─────────────────────────────────────────────────────────
        0x0E00..=0x0E7F => Some("Noto Sans Thai"),

        // ── Devanagari (Hindi, Marathi, Sanskrit …) ──────────────────────
        0x0900..=0x097F   // Devanagari
        | 0xA8E0..=0xA8FF // Devanagari Extended
        => Some("Noto Sans Devanagari"),

        // ── Hebrew ───────────────────────────────────────────────────────
        0x0590..=0x05FF   // Hebrew
        | 0xFB1D..=0xFB4F // Hebrew Presentation Forms
        => Some("Noto Sans Hebrew"),

        _ => None,
    }
}

/// Resolve CSS generic family keywords to concrete font names for Canvas Mode.
///
/// HTML Mode passes the value straight to the browser, which resolves generics
/// natively. Canvas Mode (Parley/Vello) has no system-font access in WASM, so
/// generic keywords are mapped to bundled or on-demand-fetched Noto fonts.
pub(crate) fn resolve_generic_family(name: &str) -> &str {
    match name {
        // sans-serif generics → default (Noto Sans, already bundled)
        "sans-serif"
        | "system-ui"
        | "ui-sans-serif"
        | "-apple-system"
        | "BlinkMacSystemFont"
        | "cursive"
        | "fantasy"
        | "ui-rounded" => DEFAULT_FONT_FAMILY,
        // serif → Noto Serif (fetched on demand via builtin_font_url)
        "serif" | "ui-serif" => "Noto Serif",
        // monospace → Noto Sans Mono (fetched on demand)
        "monospace" | "ui-monospace" => "Noto Sans Mono",
        // named or already-resolved family — pass through unchanged
        other => other,
    }
}

/// Build a Parley layout, break lines, and lower its glyph runs into
/// `TextRunData` instances ready for the Raw Layer.
pub fn build_text_layout(
    font_cx: &mut FontContext,
    layout_cx: &mut LayoutContext<TextBrush>,
    text: &str,
    font_size: f32,
    max_advance: Option<f32>,
    font_family: Option<&str>,
) -> TextLayout {
    let mut builder = layout_cx.ranged_builder(font_cx, text, 1.0, true);
    builder.push_default(StyleProperty::FontSize(font_size));
    // Resolve generic keywords, then build a CSS font stack so unknown names
    // fall back to the bundled default. Parley resolves left-to-right and
    // silently skips unregistered names, triggering FetchFont for missing ones.
    let stack = match font_family {
        Some(f) if !f.is_empty() => {
            let resolved = resolve_generic_family(f);
            if resolved == DEFAULT_FONT_FAMILY {
                Cow::Borrowed(DEFAULT_FONT_FAMILY)
            } else {
                Cow::Owned(format!("{resolved}, {DEFAULT_FONT_FAMILY}"))
            }
        }
        _ => Cow::Borrowed(DEFAULT_FONT_FAMILY),
    };
    builder.push_default(StyleProperty::FontFamily(FontFamily::Source(stack)));
    let mut layout: Layout<TextBrush> = builder.build(text);
    layout.break_all_lines(max_advance);

    let (runs, missing_families) = lower_glyph_runs(&layout, font_size, text);
    TextLayout {
        layout,
        runs,
        font_size,
        text: Arc::<str>::from(text),
        width_constraint: max_advance,
        missing_families,
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
            let PositionedLayoutItem::GlyphRun(grun) = item else { continue };
            let run = grun.run();
            let baseline = grun.baseline();
            let offset = grun.offset();
            let font = run.font().clone();
            let positioned: Vec<Glyph> = grun
                .glyphs()
                .scan(offset, |x, g| {
                    let glyph = Glyph { id: g.id, x: *x + g.x, y: baseline + g.y };
                    *x += g.advance;
                    Some(glyph)
                })
                .collect();
            if positioned.is_empty() {
                continue;
            }
            if positioned.iter().any(|g| g.id == 0) {
                let range = run.text_range();
                let end = range.end.min(text.len());
                if range.start < end {
                    for ch in text[range.start..end].chars() {
                        if let Some(fam) = codepoint_font_family(ch as u32) {
                            missing.insert(fam);
                        }
                    }
                }
            }
            out.push(Arc::new(TextRunData {
                font,
                font_size: run.font_size().max(font_size),
                glyphs: positioned,
                text: Arc::<str>::from(""),
            }));
        }
    }
    (out, missing.into_iter().collect())
}
