use hayate_core::{
    NodeId, NodeKind, RenderImage, RenderImageAlphaType, SceneGraph, TextRunData,
};
use skrifa::{
    instance::{LocationRef, Size},
    outline::{DrawSettings, OutlinePen},
    raw::FontRef,
    GlyphId, MetadataProvider,
};
use tiny_skia::{
    Color, FillRule, Mask, Paint, Path, PathBuilder, Pixmap, PixmapPaint, PixmapRef, Transform,
};
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use super::{CanvasBackend, ClearColor};

pub(crate) struct SelectedBackend {
    ctx: web_sys::CanvasRenderingContext2d,
    pixmap: Pixmap,
    width: u32,
    height: u32,
}

impl SelectedBackend {
    pub(crate) async fn init(canvas: HtmlCanvasElement) -> Result<Self, JsValue> {
        let width = canvas.width();
        let height = canvas.height();

        let ctx = canvas
            .get_context("2d")
            .map_err(|e| JsValue::from_str(&format!("get_context(\"2d\"): {e:?}")))?
            .ok_or_else(|| JsValue::from_str("canvas 2d context unavailable"))?
            .dyn_into::<web_sys::CanvasRenderingContext2d>()
            .map_err(|_| JsValue::from_str("failed to cast to CanvasRenderingContext2d"))?;

        let pixmap = Pixmap::new(width, height)
            .ok_or_else(|| JsValue::from_str("failed to create Pixmap (zero size?)"))?;

        Ok(Self {
            ctx,
            pixmap,
            width,
            height,
        })
    }
}

impl CanvasBackend for SelectedBackend {
    fn render_scene(
        &mut self,
        scene: &SceneGraph,
        clear_color: ClearColor,
    ) -> Result<(), JsValue> {
        let bg = to_premultiplied_color(clear_color);
        self.pixmap.fill(bg);

        for &root_id in scene.roots() {
            draw_node(scene, root_id, &mut self.pixmap, Transform::identity(), None);
        }

        blit_to_canvas(&self.ctx, &self.pixmap, self.width, self.height)
    }

    fn clear(&mut self, clear_color: ClearColor) -> Result<(), JsValue> {
        self.pixmap.fill(to_premultiplied_color(clear_color));
        blit_to_canvas(&self.ctx, &self.pixmap, self.width, self.height)
    }

    fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 || (width == self.width && height == self.height) {
            return;
        }
        if let Some(pixmap) = Pixmap::new(width, height) {
            self.pixmap = pixmap;
            self.width = width;
            self.height = height;
        }
    }
}

fn blit_to_canvas(
    ctx: &web_sys::CanvasRenderingContext2d,
    pixmap: &Pixmap,
    width: u32,
    height: u32,
) -> Result<(), JsValue> {
    let mut straight = pixmap.data().to_vec();
    premultiplied_to_straight(&mut straight);

    let image_data = web_sys::ImageData::new_with_u8_clamped_array_and_sh(
        wasm_bindgen::Clamped(&straight),
        width,
        height,
    )?;
    ctx.put_image_data(&image_data, 0.0, 0.0)
}

fn premultiplied_to_straight(data: &mut [u8]) {
    for pixel in data.chunks_exact_mut(4) {
        let a = pixel[3] as u32;
        if a == 0 {
            continue;
        }
        pixel[0] = ((pixel[0] as u32 * 255 + a / 2) / a).min(255) as u8;
        pixel[1] = ((pixel[1] as u32 * 255 + a / 2) / a).min(255) as u8;
        pixel[2] = ((pixel[2] as u32 * 255 + a / 2) / a).min(255) as u8;
    }
}

fn to_premultiplied_color(c: ClearColor) -> Color {
    let [r, g, b, a] = c;
    Color::from_rgba(r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0), a.clamp(0.0, 1.0))
        .unwrap_or(Color::TRANSPARENT)
}

fn draw_node(
    graph: &SceneGraph,
    id: NodeId,
    pixmap: &mut Pixmap,
    transform: Transform,
    mask: Option<&Mask>,
) {
    let node = match graph.get(id) {
        Some(n) => n,
        None => return,
    };
    match &node.kind {
        NodeKind::Rect {
            x,
            y,
            width,
            height,
            color,
            corner_radius,
        } => {
            let paint = color_to_paint(*color);
            if *corner_radius == 0.0 {
                if let Some(rect) = tiny_skia::Rect::from_xywh(*x, *y, *width, *height) {
                    pixmap.fill_rect(rect, &paint, transform, mask);
                }
            } else if let Some(path) = rounded_rect_path(*x, *y, *width, *height, *corner_radius) {
                pixmap.fill_path(&path, &paint, FillRule::Winding, transform, mask);
            }
        }
        NodeKind::TextRun { x, y, color, data } => {
            draw_text_run(pixmap, *x, *y, *color, data, transform, mask);
        }
        NodeKind::Image {
            x,
            y,
            width,
            height,
            data,
        } => {
            draw_image(pixmap, *x, *y, *width, *height, data, transform, mask);
        }
        NodeKind::Group { transform: t } => {
            let [a, b, c, d, e, f] = *t;
            let group_ts = Transform::from_row(
                a as f32, b as f32, c as f32, d as f32, e as f32, f as f32,
            );
            let combined = transform.pre_concat(group_ts);
            for &child_id in &node.children {
                draw_node(graph, child_id, pixmap, combined, mask);
            }
        }
        NodeKind::Clip {
            x,
            y,
            width,
            height,
        } => {
            let clip_rect = tiny_skia::Rect::from_xywh(*x, *y, *width, *height);
            if let Some(rect) = clip_rect {
                let mut pb = PathBuilder::new();
                pb.push_rect(rect);
                if let Some(path) = pb.finish() {
                    if let Some(mut clip_mask) = Mask::new(pixmap.width(), pixmap.height()) {
                        clip_mask.fill_path(&path, FillRule::Winding, true, Transform::identity());
                        for &child_id in &node.children {
                            draw_node(graph, child_id, pixmap, transform, Some(&clip_mask));
                        }
                        return;
                    }
                }
            }
            for &child_id in &node.children {
                draw_node(graph, child_id, pixmap, transform, mask);
            }
        }
    }
}

fn color_to_paint(color: [f32; 4]) -> Paint<'static> {
    let [r, g, b, a] = color;
    let mut paint = Paint::default();
    paint.set_color(
        Color::from_rgba(r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0), a.clamp(0.0, 1.0))
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

        // Font outlines use Y-up coordinates; screen is Y-down.
        // DrawSettings with Size handles scaling from font units to pixels,
        // but we need to flip Y and position at the glyph's absolute location.
        let glyph_transform = transform
            .pre_translate(run_x + glyph.x, run_y + glyph.y)
            .pre_scale(1.0, -1.0);

        pixmap.fill_path(&path, &paint, FillRule::Winding, glyph_transform, mask);
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

fn straight_to_premultiplied(data: &mut [u8]) {
    for pixel in data.chunks_exact_mut(4) {
        let a = pixel[3] as u32;
        if a == 255 {
            continue;
        }
        pixel[0] = ((pixel[0] as u32 * a + 127) / 255) as u8;
        pixel[1] = ((pixel[1] as u32 * a + 127) / 255) as u8;
        pixel[2] = ((pixel[2] as u32 * a + 127) / 255) as u8;
    }
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
