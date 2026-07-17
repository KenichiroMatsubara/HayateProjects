//! skia raster の winit window 向け [`SceneRenderer`] 実装（issue #801・ADR-0146 §3）。
//!
//! wgpu 非依存の CPU present 経路 — skia-safe のレイヤ cache/composite surface を softbuffer で
//! winit window へ software blit する。全面描画 fallback は
//! [`crate::skia_present::raster_frame_xrgb`] と headless テストで共有する。GPU アダプタが一切無い
//! 環境でも desktop が起動する、vello 初期化失敗時の一方向 fallback 先（spec §4 REND-15）。

use std::collections::{HashMap, HashSet};
use std::num::NonZeroU32;
use std::sync::Arc;

use anyhow::{anyhow, Error};
use hayate_app_host::render_host::{ClearColor, SceneRenderer};
use hayate_app_host::renderer_selection::SceneRendererKind;
use hayate_core::{ElementId, SceneGraph};
use hayate_layer_compositor::{tunables, GpuBudget, ScrollLayerGeometry};
use hayate_scene_renderer_skia::{new_raster_surface, read_rgba, SkiaLayerPresenter};
use winit::window::Window;

use crate::skia_present::{copy_rgba_to_xrgb, raster_frame_xrgb};

/// softbuffer で winit window へ CPU present する skia raster の [`SceneRenderer`] 実装。
pub struct SkiaWindowRenderer {
    window: Arc<Window>,
    /// softbuffer の表示接続。`soft_surface` と同寿命で保持する。
    _context: softbuffer::Context<Arc<Window>>,
    soft_surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,
    presenter: SkiaLayerPresenter,
}

impl SkiaWindowRenderer {
    /// winit `Window` へ softbuffer の software 提示面を立てる。GPU 資源には一切触れない。
    pub fn new(window: Arc<Window>) -> Result<Self, Error> {
        let context = softbuffer::Context::new(window.clone())
            .map_err(|e| anyhow!("softbuffer context: {e}"))?;
        let soft_surface = softbuffer::Surface::new(&context, window.clone())
            .map_err(|e| anyhow!("softbuffer surface: {e}"))?;
        let size = window.inner_size();
        let presenter = SkiaLayerPresenter::new(
            size.width.max(1),
            size.height.max(1),
            window.scale_factor() as f32,
        );
        Ok(Self {
            window,
            _context: context,
            soft_surface,
            presenter,
        })
    }

    fn present_pixels(
        &mut self,
        width: NonZeroU32,
        height: NonZeroU32,
        pixels: &[u32],
    ) -> Result<(), Error> {
        self.soft_surface
            .resize(width, height)
            .map_err(|e| anyhow!("softbuffer resize: {e}"))?;
        let mut buffer = self
            .soft_surface
            .buffer_mut()
            .map_err(|e| anyhow!("softbuffer buffer_mut: {e}"))?;
        buffer.copy_from_slice(pixels);
        buffer
            .present()
            .map_err(|e| anyhow!("softbuffer present: {e}"))
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
        let (Some(width), Some(height)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            return Ok(()); // 最小化等でゼロ寸法のフレームは描かない。
        };
        let content_scale = self.window.scale_factor() as f32;

        let pixels =
            raster_frame_xrgb(scene, width.get(), height.get(), content_scale, clear_color);

        self.present_pixels(width, height, &pixels)
    }

    fn supports_layer_present(&self) -> bool {
        true
    }

    fn present_layers(
        &mut self,
        scene: &SceneGraph,
        layers: &[ElementId],
        layer_dirty: &HashSet<ElementId>,
        _chrome_dirty: &HashSet<ElementId>,
        scroll_geometry: &HashMap<ElementId, ScrollLayerGeometry>,
        clear_color: ClearColor,
    ) -> Result<(), Error> {
        let size = self.window.inner_size();
        let (Some(width), Some(height)) =
            (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
        else {
            return Ok(()); // 最小化等でゼロ寸法のフレームは描かない。
        };
        let content_scale = self.window.scale_factor() as f32;
        self.presenter
            .resize(width.get(), height.get(), content_scale);
        let target = new_raster_surface(width.get() as i32, height.get() as i32)
            .ok_or_else(|| anyhow!("skia present surface {}x{}", width, height))?;
        let mut target = self
            .presenter
            .present(
                scene,
                layers,
                layer_dirty,
                scroll_geometry,
                clear_color,
                (0.0, 0.0),
                GpuBudget::from_viewports(
                    width.get(),
                    height.get(),
                    tunables::GPU_BUDGET_VIEWPORTS_DESKTOP,
                ),
                target,
            )
            .map_err(|e| anyhow!("skia layer present: {e}"))?;
        let rgba = read_rgba(&mut target);
        let mut pixels = vec![0u32; width.get() as usize * height.get() as usize];
        copy_rgba_to_xrgb(&rgba, &mut pixels);
        self.present_pixels(width, height, &pixels)
    }

    fn clear(&mut self, clear_color: ClearColor) -> Result<(), Error> {
        self.render_scene(&SceneGraph::default(), clear_color)
    }

    fn resize(&mut self, width: u32, height: u32, content_scale: f32) {
        self.presenter.resize(width, height, content_scale);
    }
}
