//! skia raster の winit window 向け [`SceneRenderer`] 実装（issue #801・ADR-0146 §3）。
//!
//! wgpu 非依存の CPU present 経路 — skia の CPU raster surface に 1 フレーム焼き
//! （[`crate::skia_present::raster_frame_xrgb`]、headless テストと同一経路）、softbuffer で
//! winit window へ software blit する。GPU アダプタが一切無い環境でも desktop が起動する、
//! vello 初期化失敗時の一方向 fallback 先（spec §4 REND-15）。

use std::num::NonZeroU32;
use std::sync::Arc;

use anyhow::{anyhow, Error};
use hayate_app_host::render_host::{ClearColor, SceneRenderer};
use hayate_app_host::renderer_selection::SceneRendererKind;
use hayate_core::SceneGraph;
use winit::window::Window;

use crate::skia_present::raster_frame_xrgb;

/// softbuffer で winit window へ CPU present する skia raster の [`SceneRenderer`] 実装。
pub struct SkiaWindowRenderer {
    window: Arc<Window>,
    /// softbuffer の表示接続。`soft_surface` と同寿命で保持する。
    _context: softbuffer::Context<Arc<Window>>,
    soft_surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,
}

impl SkiaWindowRenderer {
    /// winit `Window` へ softbuffer の software 提示面を立てる。GPU 資源には一切触れない。
    pub fn new(window: Arc<Window>) -> Result<Self, Error> {
        let context = softbuffer::Context::new(window.clone())
            .map_err(|e| anyhow!("softbuffer context: {e}"))?;
        let soft_surface = softbuffer::Surface::new(&context, window.clone())
            .map_err(|e| anyhow!("softbuffer surface: {e}"))?;
        Ok(Self {
            window,
            _context: context,
            soft_surface,
        })
    }
}

impl SceneRenderer for SkiaWindowRenderer {
    fn kind(&self) -> SceneRendererKind {
        SceneRendererKind::Skia
    }

    fn render_scene(&mut self, scene: &SceneGraph, clear_color: ClearColor) -> Result<(), Error> {
        // 寸法・HiDPI 係数は window から毎フレーム自給する（resize の取りこぼしや
        // fallback 直後でも常に現在値で描ける）。
        let size = self.window.inner_size();
        let (Some(width), Some(height)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            return Ok(()); // 最小化等でゼロ寸法のフレームは描かない。
        };
        let content_scale = self.window.scale_factor() as f32;

        let pixels = raster_frame_xrgb(scene, width.get(), height.get(), content_scale, clear_color);

        self.soft_surface
            .resize(width, height)
            .map_err(|e| anyhow!("softbuffer resize: {e}"))?;
        let mut buffer = self
            .soft_surface
            .buffer_mut()
            .map_err(|e| anyhow!("softbuffer buffer_mut: {e}"))?;
        buffer.copy_from_slice(&pixels);
        buffer
            .present()
            .map_err(|e| anyhow!("softbuffer present: {e}"))?;
        Ok(())
    }

    fn clear(&mut self, clear_color: ClearColor) -> Result<(), Error> {
        self.render_scene(&SceneGraph::default(), clear_color)
    }

    fn resize(&mut self, _width: u32, _height: u32, _content_scale: f32) {
        // render_scene が毎フレーム window から寸法を自給するため no-op。
    }
}
