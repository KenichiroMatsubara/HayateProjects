use hayate_core::{
    build_draw_path, is_notdef, missing_glyph_placeholder, DrawFillRule, DrawLineCap, DrawLineJoin,
    PathSink, PathVerb, RenderImage, RenderImageAlphaType, ScenePainter, StrokeStyle, TextRunData,
};
use skrifa::{
    instance::{LocationRef, NormalizedCoord, Size},
    outline::{DrawSettings, OutlinePen},
    raw::FontRef,
    GlyphId, MetadataProvider,
};
use std::sync::Arc;
use vello_cpu::color::PremulRgba8;
use vello_cpu::kurbo::{Affine, BezPath, Cap, Join, Point, Rect, Stroke, Vec2};
use vello_cpu::peniko::Fill;
use vello_cpu::{Image, ImageSource, Pixmap, RenderContext};

fn normalized_coords_ref(coords: &[i16]) -> &[NormalizedCoord] {
    // Parley は harfrust/skrifa の正規化座標を i16(F2Dot14)で保持する。
    unsafe { std::slice::from_raw_parts(coords.as_ptr().cast(), coords.len()) }
}

use crate::straight_to_premultiplied;

struct PainterState {
    transform: Affine,
    transform_stack: Vec<Affine>,
}

pub struct VelloCpuPainter<'a> {
    context: &'a mut RenderContext,
    state: PainterState,
}

impl<'a> VelloCpuPainter<'a> {
    pub fn new(context: &'a mut RenderContext, content_scale: f32) -> Self {
        let transform = if content_scale == 1.0 {
            Affine::IDENTITY
        } else {
            Affine::scale(content_scale as f64)
        };
        Self {
            context,
            state: PainterState {
                transform,
                transform_stack: Vec::new(),
            },
        }
    }
}

impl ScenePainter for VelloCpuPainter<'_> {
    fn fill_rect(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: [f32; 4],
        corner_radius: f32,
    ) {
        self.context.set_transform(self.state.transform);
        self.context.set_paint(to_color(color));
        if corner_radius == 0.0 {
            self.context.fill_rect(&Rect::new(
                f64::from(x),
                f64::from(y),
                f64::from(x + width),
                f64::from(y + height),
            ));
        } else {
            self.context.set_fill_rule(Fill::NonZero);
            let path = rounded_rect_path(x, y, width, height, corner_radius);
            self.context.fill_path(&path);
        }
    }

    fn fill_rounded_ring(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        outer_radius: f32,
        border_width: f32,
        color: [f32; 4],
    ) {
        let bw = border_width.max(0.0);
        let inner_w = (width - 2.0 * bw).max(0.0);
        let inner_h = (height - 2.0 * bw).max(0.0);
        if inner_w <= 0.0 || inner_h <= 0.0 {
            self.fill_rect(x, y, width, height, color, outer_radius);
            return;
        }

        // リング帯だけを 1 回の even-odd フィル(外側マイナス内側)で塗る。tiny-skia backend と
        // 同じ理由（内側を Clear でくり抜くと下の不透明コンテンツを消してしまう）。
        let inner_r = (outer_radius - bw).max(0.0);
        let mut path = BezPath::new();
        push_rounded_rect(&mut path, x, y, width, height, outer_radius);
        push_rounded_rect(&mut path, x + bw, y + bw, inner_w, inner_h, inner_r);

        self.context.set_transform(self.state.transform);
        self.context.set_paint(to_color(color));
        self.context.set_fill_rule(Fill::EvenOdd);
        self.context.fill_path(&path);
        self.context.set_fill_rule(Fill::NonZero);
    }

    fn fill_path(
        &mut self,
        x: f32,
        y: f32,
        verbs: &[PathVerb],
        fill_rule: DrawFillRule,
        color: [f32; 4],
    ) {
        let mut sink = CpuKurboPathSink { path: BezPath::new() };
        build_draw_path(verbs, &mut sink);
        if sink.path.is_empty() {
            return;
        }
        let rule = match fill_rule {
            DrawFillRule::NonZero => Fill::NonZero,
            DrawFillRule::EvenOdd => Fill::EvenOdd,
        };
        // verbs はボーダーボックス相対。原点 `(x, y)` は transform の平行移動で与える。
        self.context
            .set_transform(self.state.transform * Affine::translate((f64::from(x), f64::from(y))));
        self.context.set_paint(to_color(color));
        self.context.set_fill_rule(rule);
        self.context.fill_path(&sink.path);
    }

    fn stroke_path(
        &mut self,
        x: f32,
        y: f32,
        verbs: &[PathVerb],
        stroke: &StrokeStyle,
        color: [f32; 4],
    ) {
        if stroke.width <= 0.0 {
            return;
        }
        let mut sink = CpuKurboPathSink { path: BezPath::new() };
        build_draw_path(verbs, &mut sink);
        if sink.path.is_empty() {
            return;
        }
        let mut style = Stroke::new(f64::from(stroke.width));
        style.miter_limit = f64::from(stroke.miter_limit);
        style.join = match stroke.join {
            DrawLineJoin::Miter => Join::Miter,
            DrawLineJoin::Round => Join::Round,
            DrawLineJoin::Bevel => Join::Bevel,
        };
        let cap = match stroke.cap {
            DrawLineCap::Butt => Cap::Butt,
            DrawLineCap::Round => Cap::Round,
            DrawLineCap::Square => Cap::Square,
        };
        style.start_cap = cap;
        style.end_cap = cap;
        if !stroke.dash.is_empty() {
            style.dash_pattern = stroke.dash.iter().map(|d| f64::from(*d)).collect();
            style.dash_offset = f64::from(stroke.dash_offset);
        }
        self.context
            .set_transform(self.state.transform * Affine::translate((f64::from(x), f64::from(y))));
        self.context.set_paint(to_color(color));
        self.context.set_stroke(style);
        self.context.stroke_path(&sink.path);
    }

    fn stroke_dashed_border(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        outer_radius: f32,
        border_width: f32,
        color: [f32; 4],
    ) {
        let bw = border_width.max(0.0);
        if bw <= 0.0 || width <= 0.0 || height <= 0.0 {
            return;
        }
        let half = bw / 2.0;
        let inset_w = width - bw;
        let inset_h = height - bw;
        // ボックスより太いボーダーはソリッドフィルに退化する。
        if inset_w <= 0.0 || inset_h <= 0.0 {
            self.fill_rect(x, y, width, height, color, outer_radius);
            return;
        }

        let inner_r = (outer_radius - half).max(0.0);
        let path = rounded_rect_path(x + half, y + half, inset_w, inset_h, inner_r);

        let dash = f64::from(bw) * 2.0;
        let mut stroke = Stroke::new(f64::from(bw));
        stroke.join = Join::Miter;
        stroke.start_cap = Cap::Butt;
        stroke.end_cap = Cap::Butt;
        stroke.dash_pattern = vec![dash, dash].into();
        stroke.dash_offset = 0.0;

        self.context.set_transform(self.state.transform);
        self.context.set_paint(to_color(color));
        self.context.set_stroke(stroke);
        self.context.stroke_path(&path);
    }

    fn draw_text_run(&mut self, x: f32, y: f32, color: [f32; 4], data: &TextRunData) {
        let base_transform = self.state.transform;
        let paint = to_color(color);

        let font_data = data.font.data.as_ref();
        let font = match FontRef::from_index(font_data, data.font.index) {
            Ok(f) => f,
            Err(_) => return,
        };
        let outlines = font.outline_glyphs();
        let font_size = data.font_size;
        let size = Size::new(font_size);
        let location = if data.normalized_coords.is_empty() {
            LocationRef::default()
        } else {
            LocationRef::new(normalized_coords_ref(&data.normalized_coords))
        };
        let skew = data.synthesis.skew_tangent;
        let embolden_width = data.synthesis.embolden;

        for glyph in &data.glyphs {
            // `.notdef` グリフはフォントがこのコードポイントを持たないことを意味する。
            // フォント任せの無音ボックスではなく意図的なプレースホルダ箱を描き、欠落が
            // 消えずに見えるようにする。
            if is_notdef(glyph) {
                self.draw_missing_glyph(base_transform, x, y, &paint, glyph, font_size);
                continue;
            }
            let outline = match outlines.get(GlyphId::new(glyph.id)) {
                Some(o) => o,
                None => continue,
            };

            let mut pen = GlyphPen { path: BezPath::new() };
            let settings = DrawSettings::unhinted(size, location);
            if outline.draw(settings, &mut pen).is_err() {
                continue;
            }
            let path = pen.path;
            if path.elements().is_empty() {
                continue;
            }

            let mut glyph_transform = base_transform
                .pre_translate(Vec2::new(f64::from(x + glyph.x), f64::from(y + glyph.y)))
                .pre_scale_non_uniform(1.0, -1.0);
            if let Some(tangent) = skew {
                glyph_transform = glyph_transform.pre_skew(f64::from(tangent), 0.0);
            }

            self.context.set_transform(glyph_transform);
            self.context.set_paint(paint);
            self.context.set_fill_rule(Fill::NonZero);
            self.context.fill_path(&path);
            if let Some(stroke_width) = embolden_width {
                let mut stroke = Stroke::new(f64::from(stroke_width));
                stroke.join = Join::Round;
                stroke.start_cap = Cap::Round;
                stroke.end_cap = Cap::Round;
                self.context.set_stroke(stroke);
                self.context.stroke_path(&path);
            }
        }

        self.context.set_transform(base_transform);
        for deco in &data.decorations {
            let rect = Rect::new(
                f64::from(x + deco.x0),
                f64::from(y + deco.y - deco.thickness * 0.5),
                f64::from(x + deco.x0 + (deco.x1 - deco.x0).max(0.0)),
                f64::from(y + deco.y - deco.thickness * 0.5 + deco.thickness),
            );
            self.context.set_paint(paint);
            self.context.fill_rect(&rect);
        }
    }

    fn draw_image(&mut self, x: f32, y: f32, width: f32, height: f32, data: &RenderImage) {
        if data.width == 0 || data.height == 0 {
            return;
        }

        let mut bytes = data.data.data().to_vec();
        if !matches!(data.alpha_type, RenderImageAlphaType::Premultiplied) {
            straight_to_premultiplied(&mut bytes);
        }
        let pixels: Vec<PremulRgba8> = bytes
            .chunks_exact(4)
            .map(|p| PremulRgba8 {
                r: p[0],
                g: p[1],
                b: p[2],
                a: p[3],
            })
            .collect();
        let (img_w, img_h) = (data.width as u16, data.height as u16);
        if pixels.len() != usize::from(img_w) * usize::from(img_h) {
            return;
        }
        let src_pixmap = Pixmap::from_parts(pixels, img_w, img_h);
        let brush = Image {
            image: ImageSource::Pixmap(Arc::new(src_pixmap)),
            sampler: Default::default(),
        };

        let sx = f64::from(width) / f64::from(data.width);
        let sy = f64::from(height) / f64::from(data.height);
        let paint_transform =
            Affine::translate(Vec2::new(f64::from(x), f64::from(y))).pre_scale_non_uniform(sx, sy);

        self.context.set_transform(self.state.transform);
        self.context.set_paint_transform(paint_transform);
        self.context.set_paint(brush);
        self.context.fill_rect(&Rect::new(
            f64::from(x),
            f64::from(y),
            f64::from(x + width),
            f64::from(y + height),
        ));
        self.context.reset_paint_transform();
    }

    fn push_transform(&mut self, transform: [f64; 6]) {
        self.state.transform_stack.push(self.state.transform);
        let group_ts = Affine::new(transform);
        self.state.transform = self.state.transform * group_ts;
    }

    fn pop_transform(&mut self) {
        if let Some(previous) = self.state.transform_stack.pop() {
            self.state.transform = previous;
        }
    }

    fn push_clip_rect(&mut self, x: f32, y: f32, width: f32, height: f32, corner_radii: [f32; 4]) {
        // 一様な半径(現状 Hayate が出す唯一の形状)。0 なら矩形クリップ。
        let radius = corner_radii.iter().copied().fold(0.0_f32, f32::max);
        let mut path = BezPath::new();
        if radius > 0.0 {
            push_rounded_rect(&mut path, x, y, width, height, radius);
        } else {
            push_rect(&mut path, x, y, width, height);
        }
        self.context.set_transform(self.state.transform);
        self.context.set_fill_rule(Fill::NonZero);
        self.context.push_clip_path(&path);
    }

    fn push_clip_draw_path(&mut self, verbs: &[PathVerb]) {
        let mut sink = CpuKurboPathSink { path: BezPath::new() };
        build_draw_path(verbs, &mut sink);
        // verbs は walk が原点 + draw CTM を焼き込み済み（state 変換の元空間）。
        self.context.set_transform(self.state.transform);
        self.context.set_fill_rule(Fill::NonZero);
        self.context.push_clip_path(&sink.path);
    }

    fn pop_clip(&mut self) {
        self.context.pop_clip_path();
    }
}

impl VelloCpuPainter<'_> {
    /// `.notdef` グリフ用の意図的なプレースホルダ箱を、テキスト色の中空ストローク矩形
    /// として、ベースライン上の cap-height 帯に描く。ジオメトリは
    /// `missing_glyph_placeholder` 経由で他backendと共有する。
    fn draw_missing_glyph(
        &mut self,
        transform: Affine,
        run_x: f32,
        run_y: f32,
        paint: &vello_cpu::peniko::Color,
        glyph: &hayate_core::RenderGlyph,
        font_size: f32,
    ) {
        let ph = missing_glyph_placeholder(glyph, font_size);
        if ph.width <= 0.0 || ph.height <= 0.0 {
            return;
        }
        let mut path = BezPath::new();
        push_rect(&mut path, run_x + ph.x, run_y + ph.y, ph.width, ph.height);

        let mut stroke = Stroke::new(f64::from(ph.stroke_width));
        stroke.join = Join::Miter;
        stroke.start_cap = Cap::Butt;
        stroke.end_cap = Cap::Butt;

        self.context.set_transform(transform);
        self.context.set_paint(*paint);
        self.context.set_stroke(stroke);
        self.context.stroke_path(&path);
    }
}

fn to_color(color: [f32; 4]) -> vello_cpu::peniko::Color {
    let [r, g, b, a] = color;
    vello_cpu::peniko::Color::new([
        r.clamp(0.0, 1.0),
        g.clamp(0.0, 1.0),
        b.clamp(0.0, 1.0),
        a.clamp(0.0, 1.0),
    ])
}

/// kurbo `BezPath` を [`PathSink`] として橋渡しする（曲線・便宜形状・arcTo の展開は
/// 共有 [`build_draw_path`] が行う）。
struct CpuKurboPathSink {
    path: BezPath,
}

impl PathSink for CpuKurboPathSink {
    fn move_to(&mut self, x: f32, y: f32) {
        self.path.move_to((f64::from(x), f64::from(y)));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.path.line_to((f64::from(x), f64::from(y)));
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        self.path
            .quad_to((f64::from(cx), f64::from(cy)), (f64::from(x), f64::from(y)));
    }
    fn cubic_to(&mut self, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32) {
        self.path.curve_to(
            (f64::from(c1x), f64::from(c1y)),
            (f64::from(c2x), f64::from(c2y)),
            (f64::from(x), f64::from(y)),
        );
    }
    fn close(&mut self) {
        self.path.close_path();
    }
}

fn push_rect(path: &mut BezPath, x: f32, y: f32, w: f32, h: f32) {
    path.move_to(Point::new(f64::from(x), f64::from(y)));
    path.line_to(Point::new(f64::from(x + w), f64::from(y)));
    path.line_to(Point::new(f64::from(x + w), f64::from(y + h)));
    path.line_to(Point::new(f64::from(x), f64::from(y + h)));
    path.close_path();
}

fn push_rounded_rect(path: &mut BezPath, x: f32, y: f32, w: f32, h: f32, r: f32) {
    let r = r.min(w / 2.0).min(h / 2.0);
    let kappa = 0.5522848_f32;
    let k = r * kappa;
    let p = |px: f32, py: f32| Point::new(f64::from(px), f64::from(py));

    path.move_to(p(x + r, y));
    path.line_to(p(x + w - r, y));
    path.curve_to(p(x + w - r + k, y), p(x + w, y + r - k), p(x + w, y + r));
    path.line_to(p(x + w, y + h - r));
    path.curve_to(p(x + w, y + h - r + k), p(x + w - r + k, y + h), p(x + w - r, y + h));
    path.line_to(p(x + r, y + h));
    path.curve_to(p(x + r - k, y + h), p(x, y + h - r + k), p(x, y + h - r));
    path.line_to(p(x, y + r));
    path.curve_to(p(x, y + r - k), p(x + r - k, y), p(x + r, y));
    path.close_path();
}

fn rounded_rect_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> BezPath {
    let mut path = BezPath::new();
    push_rounded_rect(&mut path, x, y, w, h, r);
    path
}

struct GlyphPen {
    path: BezPath,
}

impl OutlinePen for GlyphPen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.path.move_to(Point::new(f64::from(x), f64::from(y)));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.path.line_to(Point::new(f64::from(x), f64::from(y)));
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.path.quad_to(
            Point::new(f64::from(cx0), f64::from(cy0)),
            Point::new(f64::from(x), f64::from(y)),
        );
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.path.curve_to(
            Point::new(f64::from(cx0), f64::from(cy0)),
            Point::new(f64::from(cx1), f64::from(cy1)),
            Point::new(f64::from(x), f64::from(y)),
        );
    }

    fn close(&mut self) {
        self.path.close_path();
    }
}
