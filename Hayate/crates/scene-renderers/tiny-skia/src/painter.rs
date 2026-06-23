use hayate_core::{
    RenderImage, RenderImageAlphaType, ScenePainter, TextRunData, is_notdef,
    missing_glyph_placeholder,
};
use skrifa::{
    GlyphId, MetadataProvider,
    instance::{LocationRef, NormalizedCoord, Size},
    outline::{DrawSettings, OutlinePen},
    raw::FontRef,
};
use tiny_skia::{
    Color, FillRule, LineCap, LineJoin, Mask, Paint, Path, PathBuilder, Pixmap,
    PixmapPaint, PixmapRef, Stroke, Transform,
};

fn normalized_coords_ref(coords: &[i16]) -> &[NormalizedCoord] {
    // Parley は harfrust/skrifa の正規化座標を i16(F2Dot14)で保持する。
    unsafe { std::slice::from_raw_parts(coords.as_ptr().cast(), coords.len()) }
}

use crate::straight_to_premultiplied;

struct PainterState {
    transform: Transform,
    transform_stack: Vec<Transform>,
    clip_masks: Vec<Mask>,
}

pub struct TinySkiaPainter<'a> {
    pixmap: &'a mut Pixmap,
    state: PainterState,
}

impl<'a> TinySkiaPainter<'a> {
    pub fn new(pixmap: &'a mut Pixmap, content_scale: f32) -> Self {
        let transform = if content_scale == 1.0 {
            Transform::identity()
        } else {
            Transform::from_scale(content_scale, content_scale)
        };
        Self {
            pixmap,
            state: PainterState {
                transform,
                transform_stack: Vec::new(),
                clip_masks: Vec::new(),
            },
        }
    }

}

impl ScenePainter for TinySkiaPainter<'_> {
    fn fill_rect(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: [f32; 4],
        corner_radius: f32,
    ) {
        let transform = self.state.transform;
        let mask = self.state.clip_masks.last();
        let pixmap = &mut self.pixmap;
        let paint = color_to_paint(color);
        if corner_radius == 0.0 {
            if let Some(rect) = tiny_skia::Rect::from_xywh(x, y, width, height) {
                pixmap.fill_rect(rect, &paint, transform, mask);
            }
        } else if let Some(path) = rounded_rect_path(x, y, width, height, corner_radius) {
            pixmap.fill_path(&path, &paint, FillRule::Winding, transform, mask);
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
        let transform = self.state.transform;
        let mask = self.state.clip_masks.last();
        let pixmap = &mut self.pixmap;
        let bw = border_width.max(0.0);
        let inner_w = (width - 2.0 * bw).max(0.0);
        let inner_h = (height - 2.0 * bw).max(0.0);
        if inner_w <= 0.0 || inner_h <= 0.0 {
            self.fill_rect(x, y, width, height, color, outer_radius);
            return;
        }

        // リング帯だけを 1 回の even-odd フィル(外側マイナス内側)で塗る。内側を
        // `BlendMode::Clear` でくり抜くと下の不透明コンテンツを消してしまう — 例えば
        // ネイティブフォーカスリングは塗り済み input の上に乗るため、透明化しては
        // ならない。vello バックエンドの even-odd 帯フィルと同じ。
        let paint = color_to_paint(color);
        let inner_r = (outer_radius - bw).max(0.0);
        let mut pb = PathBuilder::new();
        push_rounded_rect(&mut pb, x, y, width, height, outer_radius);
        push_rounded_rect(&mut pb, x + bw, y + bw, inner_w, inner_h, inner_r);
        if let Some(path) = pb.finish() {
            pixmap.fill_path(&path, &paint, FillRule::EvenOdd, transform, mask);
        }
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

        let transform = self.state.transform;
        let mask = self.state.clip_masks.last();
        let inner_r = (outer_radius - half).max(0.0);
        let Some(path) = rounded_rect_path(x + half, y + half, inset_w, inset_h, inner_r) else {
            return;
        };
        let paint = color_to_paint(color);
        let dash = bw * 2.0;
        let mut stroke = Stroke {
            width: bw,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            ..Stroke::default()
        };
        stroke.dash = tiny_skia::StrokeDash::new(vec![dash, dash], 0.0);
        self.pixmap
            .stroke_path(&path, &paint, &stroke, transform, mask);
    }

    fn draw_text_run(&mut self, x: f32, y: f32, color: [f32; 4], data: &TextRunData) {
        let transform = self.state.transform;
        let mask = self.state.clip_masks.last();
        draw_text_run(&mut self.pixmap, x, y, color, data, transform, mask);
    }

    fn draw_image(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        data: &RenderImage,
    ) {
        let transform = self.state.transform;
        let mask = self.state.clip_masks.last();
        draw_image(
            &mut self.pixmap,
            x,
            y,
            width,
            height,
            data,
            transform,
            mask,
        );
    }

    fn push_transform(&mut self, transform: [f64; 6]) {
        self.state.transform_stack.push(self.state.transform);
        let [a, b, c, d, e, f] = transform;
        let group_ts =
            Transform::from_row(a as f32, b as f32, c as f32, d as f32, e as f32, f as f32);
        self.state.transform = self.state.transform.pre_concat(group_ts);
    }

    fn pop_transform(&mut self) {
        if let Some(previous) = self.state.transform_stack.pop() {
            self.state.transform = previous;
        }
    }

    fn push_clip_rect(&mut self, x: f32, y: f32, width: f32, height: f32, corner_radii: [f32; 4]) {
        let transform = self.state.transform;
        // 一様な半径(現状 Hayate が出す唯一の形状)。0 なら矩形クリップ。
        let radius = corner_radii.iter().copied().fold(0.0_f32, f32::max);
        let path = if radius > 0.0 {
            rounded_rect_path(x, y, width, height, radius)
        } else if let Some(rect) = tiny_skia::Rect::from_xywh(x, y, width, height) {
            let mut pb = PathBuilder::new();
            pb.push_rect(rect);
            pb.finish()
        } else {
            None
        };
        if let Some(path) = path {
            match self.state.clip_masks.last() {
                Some(parent) => {
                    let mut clip_mask = parent.clone();
                    clip_mask.intersect_path(&path, FillRule::Winding, true, transform);
                    self.state.clip_masks.push(clip_mask);
                }
                None => {
                    if let Some(mut clip_mask) =
                        Mask::new(self.pixmap.width(), self.pixmap.height())
                    {
                        clip_mask.fill_path(&path, FillRule::Winding, true, transform);
                        self.state.clip_masks.push(clip_mask);
                    }
                }
            }
        }
    }

    fn pop_clip(&mut self) {
        self.state.clip_masks.pop();
    }
}

fn color_to_paint(color: [f32; 4]) -> Paint<'static> {
    let [r, g, b, a] = color;
    let mut paint = Paint::default();
    paint.set_color(
        Color::from_rgba(
            r.clamp(0.0, 1.0),
            g.clamp(0.0, 1.0),
            b.clamp(0.0, 1.0),
            a.clamp(0.0, 1.0),
        )
        .unwrap_or(Color::TRANSPARENT),
    );
    paint.anti_alias = true;
    paint
}

fn push_rounded_rect(pb: &mut PathBuilder, x: f32, y: f32, w: f32, h: f32, r: f32) {
    let r = r.min(w / 2.0).min(h / 2.0);
    let kappa = 0.5522848;
    let k = r * kappa;

    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    pb.close();
}

fn rounded_rect_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
    let mut pb = PathBuilder::new();
    push_rounded_rect(&mut pb, x, y, w, h, r);
    pb.finish()
}

fn draw_text_run(
    pixmap: &mut Pixmap,
    run_x: f32,
    run_y: f32,
    color: [f32; 4],
    data: &TextRunData,
    transform: Transform,
    mask: Option<&Mask>,
) {
    let paint = color_to_paint(color);
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
            draw_missing_glyph(pixmap, run_x, run_y, &paint, glyph, font_size, transform, mask);
            continue;
        }
        let outline = match outlines.get(GlyphId::new(glyph.id)) {
            Some(o) => o,
            None => continue,
        };

        let mut pen = TinySkiaPen {
            pb: PathBuilder::new(),
        };
        let settings = DrawSettings::unhinted(size, location);
        if outline.draw(settings, &mut pen).is_err() {
            continue;
        }
        let path = match pen.pb.finish() {
            Some(p) => p,
            None => continue,
        };

        let mut glyph_transform = transform
            .pre_translate(run_x + glyph.x, run_y + glyph.y)
            .pre_scale(1.0, -1.0);
        if let Some(tangent) = skew {
            glyph_transform = glyph_transform
                .pre_concat(Transform::from_row(1.0, 0.0, tangent, 1.0, 0.0, 0.0));
        }

        pixmap.fill_path(&path, &paint, FillRule::Winding, glyph_transform, mask);
        if let Some(stroke_width) = embolden_width {
            let stroke = Stroke {
                width: stroke_width,
                line_join: LineJoin::Round,
                line_cap: LineCap::Round,
                ..Stroke::default()
            };
            pixmap.stroke_path(&path, &paint, &stroke, glyph_transform, mask);
        }
    }

    for deco in &data.decorations {
        if let Some(rect) = tiny_skia::Rect::from_xywh(
            run_x + deco.x0,
            run_y + deco.y - deco.thickness * 0.5,
            (deco.x1 - deco.x0).max(0.0),
            deco.thickness,
        ) {
            let mut pb = PathBuilder::new();
            pb.push_rect(rect);
            if let Some(path) = pb.finish() {
                pixmap.fill_path(&path, &paint, FillRule::Winding, transform, mask);
            }
        }
    }
}

/// `.notdef` グリフ用の意図的なプレースホルダ箱を、テキスト色の中空ストローク矩形
/// として、ベースライン上の cap-height 帯に描く。ジオメトリは
/// `missing_glyph_placeholder` 経由で vello バックエンドと共有する。
fn draw_missing_glyph(
    pixmap: &mut Pixmap,
    run_x: f32,
    run_y: f32,
    paint: &Paint<'static>,
    glyph: &hayate_core::RenderGlyph,
    font_size: f32,
    transform: Transform,
    mask: Option<&Mask>,
) {
    let ph = missing_glyph_placeholder(glyph, font_size);
    if ph.width <= 0.0 || ph.height <= 0.0 {
        return;
    }
    let Some(rect) = tiny_skia::Rect::from_xywh(run_x + ph.x, run_y + ph.y, ph.width, ph.height)
    else {
        return;
    };
    let mut pb = PathBuilder::new();
    pb.push_rect(rect);
    let Some(path) = pb.finish() else {
        return;
    };
    let stroke = Stroke {
        width: ph.stroke_width,
        line_join: LineJoin::Miter,
        line_cap: LineCap::Butt,
        ..Stroke::default()
    };
    pixmap.stroke_path(&path, paint, &stroke, transform, mask);
}

fn draw_image(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    image: &RenderImage,
    transform: Transform,
    mask: Option<&Mask>,
) {
    if image.width == 0 || image.height == 0 {
        return;
    }

    let src_data = match image.alpha_type {
        RenderImageAlphaType::Premultiplied => image.data.to_vec(),
        _ => {
            let mut buf = image.data.to_vec();
            straight_to_premultiplied(&mut buf);
            buf
        }
    };

    let src_pixmap = match PixmapRef::from_bytes(&src_data, image.width, image.height) {
        Some(p) => p,
        None => return,
    };

    let sx = width / image.width as f32;
    let sy = height / image.height as f32;
    let img_transform = transform.pre_translate(x, y).pre_scale(sx, sy);

    pixmap.draw_pixmap(
        0,
        0,
        src_pixmap,
        &PixmapPaint::default(),
        img_transform,
        mask,
    );
}

struct TinySkiaPen {
    pb: PathBuilder,
}

impl OutlinePen for TinySkiaPen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.pb.move_to(x, y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.pb.line_to(x, y);
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.pb.quad_to(cx0, cy0, x, y);
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.pb.cubic_to(cx0, cy0, cx1, cy1, x, y);
    }

    fn close(&mut self) {
        self.pb.close();
    }
}
