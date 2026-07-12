//! skia raster フレームの ANativeWindow への CPU present 用ピクセル変換
//! （issue #802・ADR-0146 §3）。
//!
//! desktop の `skia_present.rs`（softbuffer 0RGB 変換）と同型の wgpu 非依存経路。ANativeWindow
//! は RGBX_8888（4byte/px・アルファチャンネルは無視されるが書く必要はある）を present 形式に
//! 使う——clear color が不透明（`STAGE_A_CLEAR_COLOR` は alpha 1.0）なので合成結果も実質不透明で
//! あり、alpha は固定 0xff で書けばよい（un-premultiply 不要、desktop の `copy_rgba_to_xrgb` と
//! 同じ理由）。ここは `ndk::native_window` に一切触れない純関数だけを置き、ホストでテストする。
//! ANativeWindow への実書き込み（lock/lines/unlockAndPost）は device 専用の `skia_window.rs`。

use hayate_core::SceneGraph;
use hayate_scene_renderer_skia::{new_raster_surface, read_rgba, SkiaSceneRenderer};

/// RGBA8888（premultiplied、skia raster surface の読み戻し形式）を ANativeWindow の
/// RGBX_8888（4byte/px、tightly packed、stride 非依存）へ変換して `out` に書く。
pub fn copy_rgba_to_rgbx(rgba: &[u8], out: &mut [u8]) {
    debug_assert_eq!(rgba.len(), out.len());
    for (dst, src) in out.chunks_exact_mut(4).zip(rgba.chunks_exact(4)) {
        dst[0] = src[0];
        dst[1] = src[1];
        dst[2] = src[2];
        dst[3] = 0xff;
    }
}

/// `graph` を skia CPU raster で 1 枚焼き、ANativeWindow present 形式（RGBX_8888、tightly
/// packed row-major、stride 非依存）のピクセル列を返す。`width`/`height` は物理px、
/// `content_scale` は論理→物理の変換係数（HiDPI）。`origin_x`/`origin_y`（論理px）は安全領域
/// インセットのシーン平行移動原点（b2, issue #794・ADR-0144・`SafeAreaInsets::scene_origin`）——
/// vello の `render_scene_with_offset` と同じ役割を skia 側で担う。
pub fn raster_frame_rgbx(
    graph: &SceneGraph,
    width: u32,
    height: u32,
    content_scale: f32,
    clear_color: [f32; 4],
    origin_x: f32,
    origin_y: f32,
) -> Vec<u8> {
    let mut surface = new_raster_surface(width as i32, height as i32)
        .expect("skia raster surface allocation must succeed for positive sizes");
    let canvas = surface.canvas();
    canvas.save();
    canvas.translate((origin_x, origin_y));
    SkiaSceneRenderer::new().render_scene(graph, canvas, clear_color, content_scale);
    canvas.restore();
    let rgba = read_rgba(&mut surface);
    let mut out = vec![0u8; rgba.len()];
    copy_rgba_to_rgbx(&rgba, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_bytes_map_to_rgbx_with_alpha_forced_opaque() {
        let rgba = [0x12, 0x34, 0x56, 0x80, 0xff, 0x00, 0x00, 0x00];
        let mut out = [0u8; 8];
        copy_rgba_to_rgbx(&rgba, &mut out);
        assert_eq!(out, [0x12, 0x34, 0x56, 0xff, 0xff, 0x00, 0x00, 0xff]);
    }

    #[test]
    fn raster_frame_rgbx_produces_tightly_packed_rows_of_the_requested_size() {
        let graph = SceneGraph::default();
        let pixels = raster_frame_rgbx(&graph, 4, 3, 1.0, [1.0, 0.0, 0.0, 1.0], 0.0, 0.0);
        assert_eq!(pixels.len(), 4 * 3 * 4, "tightly packed RGBX8888, no stride padding");
        // clear color 赤・不透明 → 全ピクセルが RGBX(255,0,0,255)。
        for px in pixels.chunks_exact(4) {
            assert_eq!(px, [255, 0, 0, 255]);
        }
    }
}
