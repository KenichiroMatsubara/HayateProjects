use hayate_core::SceneGraph;
use hayate_scene_renderer_tiny_skia::TinySkiaSceneRenderer;
use tiny_skia::Pixmap;

use crate::pixel::{CANVAS_H, CANVAS_W, CLEAR_COLOR};

pub fn render_scene_to_pixels(graph: &SceneGraph) -> Vec<u8> {
    let mut pixmap = Pixmap::new(CANVAS_W, CANVAS_H).expect("pixmap");
    TinySkiaSceneRenderer::new().render_scene(graph, &mut pixmap, CLEAR_COLOR, 1.0);
    pixmap.data().to_vec()
}

pub fn render_scene_to_pixels_scaled(
    graph: &SceneGraph,
    width: u32,
    height: u32,
    content_scale: f32,
) -> Vec<u8> {
    let mut pixmap = Pixmap::new(width, height).expect("pixmap");
    TinySkiaSceneRenderer::new().render_scene(graph, &mut pixmap, CLEAR_COLOR, content_scale);
    pixmap.data().to_vec()
}
