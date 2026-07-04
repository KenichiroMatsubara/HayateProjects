mod layer_compositor;
mod painter;

use hayate_core::{render_scene_graph, SceneGraph};
use vello_cpu::kurbo::Rect;
use vello_cpu::{Pixmap, RenderContext, Resources};

// ADR-0054: ScenePainter は crate 内部 seam。host 向け公開契約ではない。
use painter::VelloCpuPainter;

pub use layer_compositor::{
    VelloCpuCompositeTarget, VelloCpuLayerCompositor, VelloCpuLayerRasterizer,
};

pub struct VelloCpuSceneRenderer;

impl VelloCpuSceneRenderer {
    pub fn new() -> Self {
        Self
    }

    pub fn render_scene(
        &mut self,
        graph: &SceneGraph,
        pixmap: &mut Pixmap,
        clear_color: [f32; 4],
        content_scale: f32,
    ) {
        let width = pixmap.width();
        let height = pixmap.height();
        let mut context = RenderContext::new(width, height);
        let mut resources = Resources::new();

        // tiny-skia の `pixmap.fill(clear_color)` に相当する下地塗り。vello_cpu の
        // `Pixmap` には単色 fill API が無いため、全面矩形として描く。
        context.set_paint(to_color(clear_color));
        context.fill_rect(&Rect::new(0.0, 0.0, f64::from(width), f64::from(height)));

        let mut painter = VelloCpuPainter::new(&mut context, content_scale);
        render_scene_graph(graph, &mut painter);

        context.flush();
        context.render_to_pixmap(&mut resources, pixmap);
    }
}

impl Default for VelloCpuSceneRenderer {
    fn default() -> Self {
        Self::new()
    }
}

fn to_color(c: [f32; 4]) -> vello_cpu::peniko::Color {
    let [r, g, b, a] = c;
    vello_cpu::peniko::Color::new([
        r.clamp(0.0, 1.0),
        g.clamp(0.0, 1.0),
        b.clamp(0.0, 1.0),
        a.clamp(0.0, 1.0),
    ])
}

/// `vello_cpu::Pixmap`（premultiplied RGBA8）を straight alpha へ変換する。canvas
/// `ImageData`/`put_image_data` は straight alpha を期待するため、blit 直前に呼ぶ
/// （tiny-skia crate の同名関数と同じ役割）。
pub fn premultiplied_to_straight(data: &mut [u8]) {
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

// crate 内部のみ（painter.rs が image lowering で使用）。公開しない。
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
