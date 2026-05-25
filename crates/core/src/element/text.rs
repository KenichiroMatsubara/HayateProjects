use std::sync::Arc;

use parley::{
    FontContext, Layout, LayoutContext, PositionedLayoutItem, StyleProperty,
};
use vello::Glyph;

use crate::node::TextRunData;

/// Brush type stored in Parley styles; color is applied at draw time.
pub type TextBrush = [u8; 4];

/// A Parley layout cached on an Element, plus the lowered Vello glyph runs.
pub struct TextLayout {
    pub layout: Layout<TextBrush>,
    pub runs: Vec<Arc<TextRunData>>,
    pub font_size: f32,
    pub text: Arc<str>,
    /// Width constraint last used; if None, single-line.
    pub width_constraint: Option<f32>,
}

/// Build a Parley layout, break lines, and lower its glyph runs into
/// `TextRunData` instances ready for the Raw Layer.
pub fn build_text_layout(
    font_cx: &mut FontContext,
    layout_cx: &mut LayoutContext<TextBrush>,
    text: &str,
    font_size: f32,
    max_advance: Option<f32>,
) -> TextLayout {
    let mut builder = layout_cx.ranged_builder(font_cx, text, 1.0, true);
    builder.push_default(StyleProperty::FontSize(font_size));
    let mut layout: Layout<TextBrush> = builder.build(text);
    layout.break_all_lines(max_advance);

    let runs = lower_glyph_runs(&layout, font_size);
    TextLayout {
        layout,
        runs,
        font_size,
        text: Arc::<str>::from(text),
        width_constraint: max_advance,
    }
}

fn lower_glyph_runs(layout: &Layout<TextBrush>, font_size: f32) -> Vec<Arc<TextRunData>> {
    let mut out: Vec<Arc<TextRunData>> = Vec::new();
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
            out.push(Arc::new(TextRunData {
                font,
                font_size: run.font_size().max(font_size),
                glyphs: positioned,
                text: Arc::<str>::from(""),
            }));
        }
    }
    out
}
