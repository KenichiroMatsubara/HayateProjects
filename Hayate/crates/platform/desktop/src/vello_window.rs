//! vello/wgpu の winit window 向け [`SceneRenderer`] 実装（ADR-0118 → issue #801）。
//!
//! 1 フレームの `SceneGraph` を vello の `render_to_texture` で offscreen target に焼き、
//! `TextureBlitter` で winit の swapchain surface に blit する。web の vello backend と
//! 同型で、違いは surface の供給元が `HtmlCanvasElement` ではなく winit `Window` である
//! 点だけ。issue #801 で `PresentTarget` 直結から Render Host（`RenderHost`）配下の
//! `SceneRenderer` 実装へ置き換わり、初期化失敗・実行時失敗は skia raster への一方向
//! fallback として RenderHost が拾う。`backend-vello` feature（default on）でビルドから
//! 外せる（ADR-0146 §5）。

use std::sync::Arc;
use std::time::Instant;

use anyhow::{anyhow, Error};
use hayate_app_host::render_host::{ClearColor, SceneRenderer};
use hayate_app_host::renderer_selection::SceneRendererKind;
use hayate_core::SceneGraph;
use hayate_scene_renderer_vello::{
    create_blitter, create_target_view, VelloRenderTarget, VelloSceneRenderer,
};
use wgpu::util::TextureBlitter;
use winit::window::Window;

use crate::pipeline_disk_cache;

/// winit の wgpu surface へ present する vello/wgpu の [`SceneRenderer`] 実装。
pub struct VelloWindowRenderer {
    window: Arc<Window>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    target_view: wgpu::TextureView,
    blitter: TextureBlitter,
    renderer: VelloSceneRenderer,
    /// バッキングストア寸法（物理 px）。
    width: u32,
    height: u32,
}

impl VelloWindowRenderer {
    /// winit `Window` から wgpu surface を立て、vello renderer を初期化する。
    pub async fn new_async(window: Arc<Window>) -> Result<Self, Error> {
        let size = window.inner_size();
        let (width, height) = (size.width.max(1), size.height.max(1));

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::from_env().unwrap_or(wgpu::Backends::all()),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });

        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| anyhow!("create_surface: {e}"))?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .map_err(|e| anyhow!("no compatible wgpu adapter: {e}"))?;

        // 永続パイプラインキャッシュ（ADR-0130b・issue #777）。対応 backend（現状 Vulkan のみ）
        // なら feature を要求し、前回起動の blob をディスクから読んで vello に注入する。
        // 非対応・破損・キー不一致はすべてキャッシュ無しにフォールバックし、起動は壊さない。
        let adapter_info = adapter.get_info();
        let supports_pipeline_cache = adapter.features().contains(wgpu::Features::PIPELINE_CACHE);

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("hayate-desktop"),
                required_features: if supports_pipeline_cache {
                    wgpu::Features::PIPELINE_CACHE
                } else {
                    wgpu::Features::empty()
                },
                ..Default::default()
            })
            .await
            .map_err(|e| anyhow!("request_device: {e}"))?;

        let mut surface_config = surface
            .get_default_config(&adapter, width, height)
            .ok_or_else(|| anyhow!("surface not supported by adapter"))?;
        surface_config.usage |= wgpu::TextureUsages::RENDER_ATTACHMENT;
        // vello は offscreen target（`Rgba8Unorm`・非 sRGB）へ sRGB エンコード済みのバイトを書く。
        // surface が *Srgb 形式（Windows の `get_default_config` 既定は `Rgba8UnormSrgb`）だと、
        // blit の書き込みで linear→sRGB エンコードが二重にかかり色が淡く（washed out）見える。
        // surface を非 sRGB 形式に揃えて二重エンコードを防ぐ（web の canvas は既定が非 sRGB なので
        // この問題が出ない）。blitter は下でこの `surface_config.format` で生成するので整合する。
        surface_config.format = surface_config.format.remove_srgb_suffix();
        surface.configure(&device, &surface_config);

        let target_view = create_target_view(&device, width, height);
        let blitter = create_blitter(&device, surface_config.format);

        let disk_cache = supports_pipeline_cache
            .then(|| {
                pipeline_disk_cache::DiskPipelineCache::discover(
                    &adapter_info,
                    hayate_scene_renderer_vello::shader_set_fingerprint(),
                )
            })
            .flatten();
        let gpu_cache = disk_cache.as_ref().map(|dc| {
            match dc.loaded_blob() {
                Some(blob) => log::info!(
                    "pipeline cache: hit ({} bytes, {})",
                    blob.len(),
                    dc.path().display()
                ),
                None => log::info!("pipeline cache: miss ({})", dc.path().display()),
            }
            // Safety: `data` は同一 adapter・同一キー（ドライバ/シェーダ指紋）で過去の
            // `get_data()` が返した blob（ADR-0130b の load がキー検証済み）。万一無効でも
            // `fallback: true` で wgpu が空キャッシュに落とす。
            unsafe {
                device.create_pipeline_cache(&wgpu::PipelineCacheDescriptor {
                    label: Some("hayate-desktop-pipeline-cache"),
                    data: dc.loaded_blob(),
                    fallback: true,
                })
            }
        });

        let init_start = Instant::now();
        let renderer = VelloSceneRenderer::new_with_pipeline_cache(&device, gpu_cache.as_ref())
            .map_err(|e| anyhow!("vello renderer init: {e}"))?;
        log::info!(
            "vello renderer init: {:.0}ms (pipeline cache: {})",
            init_start.elapsed().as_secs_f64() * 1000.0,
            match &disk_cache {
                Some(dc) if dc.loaded_blob().is_some() => "hit",
                Some(_) => "miss",
                None => "unavailable",
            }
        );

        // 次回起動用に blob を永続化する（読めた blob と同一なら書かない）。
        if let (Some(dc), Some(cache)) = (&disk_cache, &gpu_cache) {
            if let Some(data) = cache.get_data() {
                dc.persist(&data);
            }
        }

        Ok(Self {
            window,
            device,
            queue,
            surface,
            surface_config,
            target_view,
            blitter,
            renderer,
            width,
            height,
        })
    }

    /// 同期初期化（Render Host の一方向 fallback 用シーム）。vello はプランの先頭なので
    /// fallback「先」になることは無いが、`RendererInit` の契約として実装しておく。
    pub fn new_sync(window: Arc<Window>) -> Result<Self, Error> {
        pollster::block_on(Self::new_async(window))
    }

    fn reconfigure(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 || (width == self.width && height == self.height) {
            return;
        }
        self.width = width;
        self.height = height;
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
        self.target_view = create_target_view(&self.device, width, height);
    }
}

impl SceneRenderer for VelloWindowRenderer {
    fn kind(&self) -> SceneRendererKind {
        SceneRendererKind::Vello
    }

    fn render_scene(&mut self, scene: &SceneGraph, clear_color: ClearColor) -> Result<(), Error> {
        // 論理→物理の変換係数（HiDPI）は window から毎フレーム導く。fallback 直後の
        // `RenderHost` からの resize は scale を運ばないため、ここで自給するのが安全。
        let content_scale = self.window.scale_factor() as f32;

        self.renderer
            .render_scene(
                scene,
                &VelloRenderTarget {
                    device: &self.device,
                    queue: &self.queue,
                    target_view: &self.target_view,
                    width: self.width,
                    height: self.height,
                },
                clear_color,
                content_scale,
            )
            .map_err(|e| anyhow!("vello render_scene failed: {e}"))?;

        let surface_texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            // surface が陳腐化したら次フレームで再 configure に委ねる。
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.surface_config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Occluded => return Ok(()),
            other => {
                log::warn!("get_current_texture: {other:?}");
                return Ok(());
            }
        };

        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("hayate-desktop-blit"),
            });
        self.blitter
            .copy(&self.device, &mut encoder, &self.target_view, &surface_view);
        self.queue.submit(std::iter::once(encoder.finish()));
        surface_texture.present();
        Ok(())
    }

    fn clear(&mut self, clear_color: ClearColor) -> Result<(), Error> {
        self.render_scene(&SceneGraph::default(), clear_color)
    }

    fn resize(&mut self, width: u32, height: u32, _content_scale: f32) {
        self.reconfigure(width, height);
    }
}
