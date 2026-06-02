use vello::{
    kurbo::{Affine, Rect, RoundedRect},
    peniko::{
        color::{AlphaColor, Srgb},
        Fill, ImageBrush,
    },
    Scene,
};

use crate::node::{NodeId, NodeKind, SceneGraph};

pub fn build_scene(graph: &SceneGraph) -> Scene {
    let mut scene = Scene::new();
    for &root_id in graph.roots() {
        draw_node(graph, root_id, &mut scene);
    }
    scene
}

fn draw_node(graph: &SceneGraph, id: NodeId, scene: &mut Scene) {
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
            let brush = AlphaColor::<Srgb>::new(*color);
            let x0 = *x as f64;
            let y0 = *y as f64;
            let x1 = (*x + *width) as f64;
            let y1 = (*y + *height) as f64;
            if *corner_radius == 0.0 {
                scene.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    brush,
                    None,
                    &Rect::new(x0, y0, x1, y1),
                );
            } else {
                scene.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    brush,
                    None,
                    &RoundedRect::new(x0, y0, x1, y1, *corner_radius as f64),
                );
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
        NodeKind::Image {
            x,
            y,
            width,
            height,
            data,
        } => {
            let img_w = data.width as f32;
            let img_h = data.height as f32;
            let sx = if img_w > 0.0 { *width / img_w } else { 1.0 };
            let sy = if img_h > 0.0 { *height / img_h } else { 1.0 };
            let transform = Affine::new([sx as f64, 0.0, 0.0, sy as f64, *x as f64, *y as f64]);
            let brush = ImageBrush::new((**data).clone());
            scene.draw_image(&brush, transform);
        }
        NodeKind::Group { transform } => {
            let affine = Affine::new(*transform);
            // Render children into a sub-scene, then append with the transform applied.
            let children: Vec<NodeId> = node.children.clone();
            let mut sub = Scene::new();
            for child_id in children {
                draw_node(graph, child_id, &mut sub);
            }
            scene.append(&sub, Some(affine));
        }
        NodeKind::Clip {
            x,
            y,
            width,
            height,
        } => {
            let clip = Rect::new(
                *x as f64,
                *y as f64,
                (*x + *width) as f64,
                (*y + *height) as f64,
            );
            let children: Vec<NodeId> = node.children.clone();
            scene.push_clip_layer(Fill::NonZero, Affine::IDENTITY, &clip);
            for child_id in children {
                draw_node(graph, child_id, scene);
            }
            scene.pop_layer();
        }
    }
}
