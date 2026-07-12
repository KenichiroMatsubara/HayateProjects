//! Skia キャンバスへ描く [`ScenePainter`] 実装（ADR-0054 crate 内部 seam・非公開）。
//!
//! Skia の `Canvas` は save/restore が matrix と clip を一体で積むスタックを持つため、
//! tiny-skia 実装のような手動の `transform_stack` / `clip_masks` は不要——
//! `push_transform`/`push_clip_rect`/`push_clip_draw_path` はすべて `canvas.save()`、
//! 対応する `pop_transform`/`pop_clip` はすべて `canvas.restore()` で対称に閉じる
//! （walk が push/pop を必ず対で呼ぶため、単一スタックでも取り違えない）。
//! 描画メソッドはローカル座標をそのまま Skia へ渡し、CTM の適用は Canvas に任せる。

use hayate_core::{
    DrawFillRule, DrawLineCap, DrawLineJoin, PathSink, PathVerb, RenderGlyph, RenderImage,
    RenderImageAlphaType, ScenePainter, StrokeStyle, TextRunData, build_draw_path, is_notdef,
    missing_glyph_placeholder,
};
use skia_safe::{
    AlphaType, Canvas, Color4f, ColorType, Data, Font, FontMgr, ImageInfo, Paint,
    Path, PathBuilder as SkPathBuilder, PathFillType, Point, RRect, Rect, SamplingOptions,
    TextBlobBuilder,
    canvas::SrcRectConstraint,
    dash_path_effect,
    images,
    paint::{Cap as PaintCap, Join as PaintJoin, Style as PaintStyle},
};

pub struct SkiaPainter<'a> {
    canvas: &'a Canvas,
}

impl<'a> SkiaPainter<'a> {
    pub fn new(canvas: &'a Canvas) -> Self {
        Self { canvas }
    }
}

fn paint_for(color: [f32; 4]) -> Paint {
    let [r, g, b, a] = color;
    let mut paint = Paint::new(Color4f::new(r, g, b, a), None);
    paint.set_anti_alias(true);
    paint
}

fn rrect_uniform(x: f32, y: f32, width: f32, height: f32, radius: f32) -> RRect {
    let r = radius.max(0.0);
    RRect::new_rect_xy(Rect::from_xywh(x, y, width, height), r, r)
}

impl ScenePainter for SkiaPainter<'_> {
    fn fill_rect(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: [f32; 4],
        corner_radius: f32,
    ) {
        let paint = paint_for(color);
        if corner_radius <= 0.0 {
            self.canvas.draw_rect(Rect::from_xywh(x, y, width, height), &paint);
        } else {
            self.canvas
                .draw_rrect(rrect_uniform(x, y, width, height, corner_radius), &paint);
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
        let inner_r = (outer_radius - bw).max(0.0);
        let mut pb = SkPathBuilder::new_with_fill_type(PathFillType::EvenOdd);
        pb.add_rrect(rrect_uniform(x, y, width, height, outer_radius), None, None);
        pb.add_rrect(
            rrect_uniform(x + bw, y + bw, inner_w, inner_h, inner_r),
            None,
            None,
        );
        let path = pb.detach();
        self.canvas.draw_path(&path, &paint_for(color));
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
        if inset_w <= 0.0 || inset_h <= 0.0 {
            self.fill_rect(x, y, width, height, color, outer_radius);
            return;
        }
        let inner_r = (outer_radius - half).max(0.0);
        let rrect = rrect_uniform(x + half, y + half, inset_w, inset_h, inner_r);
        let mut paint = paint_for(color);
        paint.set_style(PaintStyle::Stroke);
        paint.set_stroke_width(bw);
        paint.set_stroke_cap(PaintCap::Butt);
        paint.set_stroke_join(PaintJoin::Miter);
        let dash = bw * 2.0;
        if let Some(effect) = dash_path_effect::new(&[dash, dash], 0.0) {
            paint.set_path_effect(effect);
        }
        self.canvas.draw_rrect(rrect, &paint);
    }

    fn fill_path(
        &mut self,
        x: f32,
        y: f32,
        verbs: &[PathVerb],
        fill_rule: DrawFillRule,
        color: [f32; 4],
    ) {
        let Some(path) = verbs_to_path(verbs, fill_rule) else {
            return;
        };
        self.canvas.save();
        self.canvas.translate((x, y));
        self.canvas.draw_path(&path, &paint_for(color));
        self.canvas.restore();
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
        let Some(path) = verbs_to_path(verbs, DrawFillRule::NonZero) else {
            return;
        };
        let mut paint = paint_for(color);
        paint.set_style(PaintStyle::Stroke);
        paint.set_stroke_width(stroke.width);
        paint.set_stroke_miter(stroke.miter_limit);
        paint.set_stroke_cap(match stroke.cap {
            DrawLineCap::Butt => PaintCap::Butt,
            DrawLineCap::Round => PaintCap::Round,
            DrawLineCap::Square => PaintCap::Square,
        });
        paint.set_stroke_join(match stroke.join {
            DrawLineJoin::Miter => PaintJoin::Miter,
            DrawLineJoin::Round => PaintJoin::Round,
            DrawLineJoin::Bevel => PaintJoin::Bevel,
        });
        if !stroke.dash.is_empty() {
            if let Some(effect) = dash_path_effect::new(&stroke.dash, stroke.dash_offset) {
                paint.set_path_effect(effect);
            }
        }
        self.canvas.save();
        self.canvas.translate((x, y));
        self.canvas.draw_path(&path, &paint);
        self.canvas.restore();
    }

    fn draw_text_run(&mut self, x: f32, y: f32, color: [f32; 4], data: &TextRunData) {
        draw_text_run(self.canvas, x, y, color, data);
    }

    fn draw_image(&mut self, x: f32, y: f32, width: f32, height: f32, data: &RenderImage) {
        draw_image(self.canvas, x, y, width, height, data);
    }

    fn push_transform(&mut self, transform: [f64; 6]) {
        let [a, b, c, d, e, f] = transform;
        self.canvas.save();
        let matrix = skia_safe::Matrix::new_all(
            a as f32, c as f32, e as f32, b as f32, d as f32, f as f32, 0.0, 0.0, 1.0,
        );
        self.canvas.concat(&matrix);
    }

    fn pop_transform(&mut self) {
        self.canvas.restore();
    }

    fn push_clip_rect(&mut self, x: f32, y: f32, width: f32, height: f32, corner_radii: [f32; 4]) {
        self.canvas.save();
        let radius = corner_radii.iter().copied().fold(0.0_f32, f32::max);
        if radius > 0.0 {
            self.canvas.clip_rrect(
                rrect_uniform(x, y, width, height, radius),
                None,
                Some(true),
            );
        } else {
            self.canvas
                .clip_rect(Rect::from_xywh(x, y, width, height), None, Some(true));
        }
    }

    fn push_clip_draw_path(&mut self, verbs: &[PathVerb]) {
        self.canvas.save();
        if let Some(path) = verbs_to_path(verbs, DrawFillRule::NonZero) {
            self.canvas.clip_path(&path, None, Some(true));
        } else {
            // 退化クリップ（空パス）は何も通さない。walk のクリップ計数は save() で
            // すでに一致しているので、空矩形クリップで無 op を保証する。
            self.canvas.clip_rect(Rect::from_xywh(0.0, 0.0, 0.0, 0.0), None, None);
        }
    }

    fn pop_clip(&mut self) {
        self.canvas.restore();
    }
}

fn verbs_to_path(verbs: &[PathVerb], fill_rule: DrawFillRule) -> Option<Path> {
    let fill_type = match fill_rule {
        DrawFillRule::NonZero => PathFillType::Winding,
        DrawFillRule::EvenOdd => PathFillType::EvenOdd,
    };
    let mut sink = SkiaPathSink {
        pb: SkPathBuilder::new_with_fill_type(fill_type),
        has_points: false,
    };
    build_draw_path(verbs, &mut sink);
    if !sink.has_points {
        return None;
    }
    Some(sink.pb.detach())
}

struct SkiaPathSink {
    pb: SkPathBuilder,
    has_points: bool,
}

impl PathSink for SkiaPathSink {
    fn move_to(&mut self, x: f32, y: f32) {
        self.has_points = true;
        self.pb.move_to(Point::new(x, y));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.has_points = true;
        self.pb.line_to(Point::new(x, y));
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        self.has_points = true;
        self.pb.quad_to(Point::new(cx, cy), Point::new(x, y));
    }
    fn cubic_to(&mut self, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32) {
        self.has_points = true;
        self.pb
            .cubic_to(Point::new(c1x, c1y), Point::new(c2x, c2y), Point::new(x, y));
    }
    fn close(&mut self) {
        self.pb.close();
    }
}

/// `RenderFont` のバイト列から SkTypeface を作る。フォントごとに毎回構築する
/// （呼び出し側でキャッシュしない）—— v1 はシンプルさを優先し、最適化は後日。
fn typeface_for(data: &TextRunData) -> Option<skia_safe::Typeface> {
    let bytes: &[u8] = data.font.data.as_ref();
    FontMgr::default().new_from_data(bytes, data.font.index as usize)
}

fn draw_text_run(canvas: &Canvas, run_x: f32, run_y: f32, color: [f32; 4], data: &TextRunData) {
    let Some(typeface) = typeface_for(data) else {
        return;
    };
    let mut font = Font::new(typeface, data.font_size);
    font.set_subpixel(true);
    font.set_hinting(skia_safe::FontHinting::None);
    // ビットマップ絵文字（CBDT/CBLC・sbix）が描かれるために必須（既定 false）。COLR/CPAL
    // ベクタカラーグリフはこのフラグと無関係に自動判定される（ADR-0146 §4）。
    font.set_embedded_bitmaps(true);
    if let Some(tangent) = data.synthesis.skew_tangent {
        font.set_skew_x(tangent);
    }
    if data.synthesis.embolden.is_some() {
        font.set_embolden(true);
    }

    let paint = paint_for(color);

    // notdef グリフはフォールバックの無音アウトラインではなく、意図的なプレースホルダ
    // 箱を描く（vello / tiny-skia と同じ geometry を共有 `missing_glyph_placeholder` から借りる）。
    let mut real_glyphs: Vec<&RenderGlyph> = Vec::with_capacity(data.glyphs.len());
    for glyph in &data.glyphs {
        if is_notdef(glyph) {
            draw_missing_glyph(canvas, run_x, run_y, &paint, glyph, data.font_size);
        } else {
            real_glyphs.push(glyph);
        }
    }

    if !real_glyphs.is_empty() {
        let mut builder = TextBlobBuilder::new();
        let (glyph_ids, positions) = builder.alloc_run_pos(&font, real_glyphs.len(), None);
        for (i, glyph) in real_glyphs.iter().enumerate() {
            glyph_ids[i] = glyph.id as u16;
            positions[i] = Point::new(glyph.x, glyph.y);
        }
        if let Some(blob) = builder.make() {
            canvas.draw_text_blob(&blob, (run_x, run_y), &paint);
        }
    }

    for deco in &data.decorations {
        let rect = Rect::from_xywh(
            run_x + deco.x0,
            run_y + deco.y - deco.thickness * 0.5,
            (deco.x1 - deco.x0).max(0.0),
            deco.thickness,
        );
        canvas.draw_rect(rect, &paint);
    }
}

fn draw_missing_glyph(
    canvas: &Canvas,
    run_x: f32,
    run_y: f32,
    paint: &Paint,
    glyph: &RenderGlyph,
    font_size: f32,
) {
    let ph = missing_glyph_placeholder(glyph, font_size);
    if ph.width <= 0.0 || ph.height <= 0.0 {
        return;
    }
    let mut stroke = paint.clone();
    stroke.set_style(PaintStyle::Stroke);
    stroke.set_stroke_width(ph.stroke_width);
    stroke.set_stroke_cap(PaintCap::Butt);
    stroke.set_stroke_join(PaintJoin::Miter);
    canvas.draw_rect(
        Rect::from_xywh(run_x + ph.x, run_y + ph.y, ph.width, ph.height),
        &stroke,
    );
}

fn draw_image(canvas: &Canvas, x: f32, y: f32, width: f32, height: f32, image: &RenderImage) {
    if image.width == 0 || image.height == 0 {
        return;
    }
    let alpha_type = match image.alpha_type {
        RenderImageAlphaType::Opaque => AlphaType::Opaque,
        RenderImageAlphaType::Alpha => AlphaType::Unpremul,
        RenderImageAlphaType::Premultiplied => AlphaType::Premul,
    };
    let info = ImageInfo::new(
        (image.width as i32, image.height as i32),
        ColorType::RGBA8888,
        alpha_type,
        None,
    );
    let row_bytes = info.min_row_bytes();
    let data = Data::new_copy(image.data.data());
    let Some(sk_image) = images::raster_from_data(&info, data, row_bytes) else {
        return;
    };
    let dst = Rect::from_xywh(x, y, width, height);
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    canvas.draw_image_rect_with_sampling_options(
        &sk_image,
        None::<(&Rect, SrcRectConstraint)>,
        dst,
        SamplingOptions::default(),
        &paint,
    );
}
