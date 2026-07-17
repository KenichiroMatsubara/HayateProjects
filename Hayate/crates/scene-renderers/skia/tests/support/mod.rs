//! テスト専用の小さな raster ヘルパ。`hayate-scene-renderer-skia` の公開 API
//! （`SkiaSceneRenderer::render_scene` + `new_raster_surface`/`read_rgba`）だけを使い、
//! `SceneGraph` を RGBA8 straight bytes へ描く。
//!
//! `tests/*.rs` の各バイナリはこのモジュールの一部だけを使うため、未使用項目の
//! warning は許容する（統合テストの `mod support;` 共有パターンの通常の副作用）。
#![allow(dead_code)]

use hayate_core::SceneGraph;
use hayate_scene_renderer_skia::{new_raster_surface, read_rgba, SkiaSceneRenderer};

pub const CANVAS_W: u32 = 100;
pub const CANVAS_H: u32 = 100;
pub const CLEAR_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

pub fn render_scene_to_pixels_scaled(
    graph: &SceneGraph,
    width: u32,
    height: u32,
    content_scale: f32,
) -> Vec<u8> {
    let mut surface = new_raster_surface(width as i32, height as i32).expect("skia raster surface");
    SkiaSceneRenderer::new().render_scene(graph, surface.canvas(), CLEAR_COLOR, content_scale);
    read_rgba(&mut surface)
}

pub fn render_scene_to_pixels(graph: &SceneGraph) -> Vec<u8> {
    render_scene_to_pixels_scaled(graph, CANVAS_W, CANVAS_H, 1.0)
}

pub fn pixel(data: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let idx = ((y * width + x) * 4) as usize;
    [data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]
}
