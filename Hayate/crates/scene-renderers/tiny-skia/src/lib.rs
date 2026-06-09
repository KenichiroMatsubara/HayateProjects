mod painter;

use hayate_core::{render_scene_graph, SceneGraph};
use tiny_skia::{Color, Pixmap};

// ADR-0054: ScenePainter は crate 内部 seam。host 向け公開契約ではない。
use painter::TinySkiaPainter;

pub struct TinySkiaSceneRenderer;

impl TinySkiaSceneRenderer {
    pub fn new() -> Self {
        Self
    }

    pub fn render_scene(
        &mut self,
        graph: &SceneGraph,
        pixmap: &mut Pixmap,
        clear_color: [f32; 4],
    ) {
        pixmap.fill(to_premultiplied_color(clear_color));
        let mut painter = TinySkiaPainter::new(pixmap);
        render_scene_graph(graph, &mut painter);
    }
}

fn to_premultiplied_color(c: [f32; 4]) -> Color {
    let [r, g, b, a] = c;
    Color::from_rgba(
        r.clamp(0.0, 1.0),
        g.clamp(0.0, 1.0),
        b.clamp(0.0, 1.0),
        a.clamp(0.0, 1.0),
    )
    .unwrap_or(Color::TRANSPARENT)
}

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
