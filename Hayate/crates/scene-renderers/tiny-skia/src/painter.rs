use hayate_core::{RenderImage, RenderImageAlphaType, ScenePainter, TextRunData};
use skrifa::{
    GlyphId, MetadataProvider,
    instance::{LocationRef, Size},
    outline::{DrawSettings, OutlinePen},
    raw::FontRef,
};
use tiny_skia::{
    BlendMode, Color, FillRule, Mask, Paint, Path, PathBuilder, Pixmap, PixmapPaint, PixmapRef, Transform,
};

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
    pub fn new(pixmap: &'a mut Pixmap) -> Self {
        Self {
            pixmap,
            state: PainterState {
                transform: Transform::identity(),
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

        let paint = color_to_paint(color);
        if let Some(outer) = rounded_rect_path(x, y, width, height, outer_radius) {
            pixmap.fill_path(&outer, &paint, FillRule::Winding, transform, mask);
        }
        let inner_r = (outer_radius - bw).max(0.0);
        if let Some(inner) = rounded_rect_path(x + bw, y + bw, inner_w, inner_h, inner_r) {
            let mut clear = Paint::default();
            clear.set_color(Color::TRANSPARENT);
            clear.blend_mode = BlendMode::Clear;
            clear.anti_alias = true;
            pixmap.fill_path(&inner, &clear, FillRule::Winding, transform, mask);
        }
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

    fn push_clip_rect(&mut self, x: f32, y: f32, width: f32, height: f32) {
        if let Some(rect) = tiny_skia::Rect::from_xywh(x, y, width, height) {
            let mut pb = PathBuilder::new();
            pb.push_rect(rect);
            if let Some(path) = pb.finish() {
                if let Some(mut clip_mask) = Mask::new(self.pixmap.width(), self.pixmap.height()) {
                    clip_mask.fill_path(&path, FillRule::Winding, true, Transform::identity());
                    self.state.clip_masks.push(clip_mask);
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

fn rounded_rect_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
    let r = r.min(w / 2.0).min(h / 2.0);
    let kappa = 0.5522848;
    let k = r * kappa;

    let mut pb = PathBuilder::new();
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

    for glyph in &data.glyphs {
        let outline = match outlines.get(GlyphId::new(glyph.id)) {
            Some(o) => o,
            None => continue,
        };

        let mut pen = TinySkiaPen {
            pb: PathBuilder::new(),
        };
        let settings = DrawSettings::unhinted(size, LocationRef::default());
        if outline.draw(settings, &mut pen).is_err() {
            continue;
        }
        let path = match pen.pb.finish() {
            Some(p) => p,
            None => continue,
        };

        let glyph_transform = transform
            .pre_translate(run_x + glyph.x, run_y + glyph.y)
            .pre_scale(1.0, -1.0);

        pixmap.fill_path(&path, &paint, FillRule::Winding, glyph_transform, mask);
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
