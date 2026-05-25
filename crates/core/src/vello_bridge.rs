use vello::{
    Scene,
    kurbo::{Affine, Rect, RoundedRect},
    peniko::{Fill, color::{AlphaColor, Srgb}},
};

use crate::node::{NodeKind, SceneGraph};

pub fn build_scene(graph: &SceneGraph) -> Scene {
    let mut scene = Scene::new();
    for (_, node) in graph.iter() {
        match &node.kind {
            NodeKind::Rect { x, y, width, height, color, corner_radius } => {
                let brush = AlphaColor::<Srgb>::new(*color);
                let x0 = *x as f64;
                let y0 = *y as f64;
                let x1 = (*x + *width) as f64;
                let y1 = (*y + *height) as f64;
                if *corner_radius == 0.0 {
                    let rect = Rect::new(x0, y0, x1, y1);
                    scene.fill(Fill::NonZero, Affine::IDENTITY, brush, None, &rect);
                } else {
                    let rect = RoundedRect::new(x0, y0, x1, y1, *corner_radius as f64);
                    scene.fill(Fill::NonZero, Affine::IDENTITY, brush, None, &rect);
                }
            }
            NodeKind::TextRun { x, y, color, data } => {
                let brush = AlphaColor::<Srgb>::new(*color);
                scene
                    .draw_glyphs(&data.font)
                    .font_size(data.font_size)
                    .brush(brush)
                    .transform(Affine::translate((*x as f64, *y as f64)))
                    .draw(Fill::NonZero, data.glyphs.iter().copied());
            }
        }
    }
    scene
}
