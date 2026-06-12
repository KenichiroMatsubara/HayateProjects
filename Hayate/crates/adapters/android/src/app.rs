//! Stage A render smoke test (ADR-0087): clear an empty `SceneGraph` to a
//! GPU surface backed by the `ANativeWindow` that `android-activity` hands
//! us. No touch input, IME, or AccessKit yet (stages B/C).
//!
//! Crate/feature versions below (`android-activity`, its `ndk`/
//! `raw-window-handle` re-exports, and `wgpu`'s `raw-window-handle`
//! version) need to line up; this is verified locally with the Android NDK,
//! which is unavailable in the sandbox that authored this file (ADR-0087).

use std::time::Duration;

use android_activity::{AndroidApp, MainEvent, PollEvent};
use hayate_core::SceneGraph;
use hayate_scene_renderer_vello::{
    create_blitter, create_target_view, VelloRenderTarget, VelloSceneRenderer,
};
use wgpu::util::TextureBlitter;

/// RGBA clear color for the stage A smoke test.
const CLEAR_COLOR: [f32; 4] = [0.1, 0.1, 0.12, 1.0];

struct GpuSurface {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    target_view: wgpu::TextureView,
    blitter: TextureBlitter,
    width: u32,
    height: u32,
    scene_renderer: VelloSceneRenderer,
}

#[no_mangle]
pub fn android_main(app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );

    let mut gpu: Option<GpuSurface> = None;
    let empty_scene = SceneGraph::new();
    let mut quit = false;

    while !quit {
        app.poll_events(Some(Duration::from_millis(16)), |event| {
            if let PollEvent::Main(main_event) = event {
                match main_event {
                    MainEvent::InitWindow { .. } => {
                        if let Some(window) = app.native_window() {
                            match pollster::block_on(init_gpu_surface(&window)) {
                                Ok(surface) => gpu = Some(surface),
                                Err(err) => log::error!("hayate-adapter-android: GPU init failed: {err}"),
                            }
                        }
                    }
                    MainEvent::TerminateWindow { .. } => {
                        gpu = None;
                    }
                    MainEvent::Destroy => quit = true,
                    _ => {}
                }
            }
        });

        if let Some(surface) = gpu.as_mut() {
            if let Err(err) = surface.render_clear(&empty_scene) {
                log::error!("hayate-adapter-android: render failed: {err}");
            }
        }
    }
}

async fn init_gpu_surface(window: &ndk::native_window::NativeWindow) -> Result<GpuSurface, String> {
    let width = window.width().max(1) as u32;
    let height = window.height().max(1) as u32;

    let instance = wgpu::Instance::default();

    // SAFETY: `window` outlives the surface for the lifetime of this adapter
    // (recreated on `InitWindow`, dropped on `TerminateWindow`).
    let surface = unsafe {
        instance
            .create_surface_unsafe(
                wgpu::SurfaceTargetUnsafe::from_window(window)
                    .map_err(|e| format!("SurfaceTargetUnsafe::from_window: {e}"))?,
            )
            .map_err(|e| format!("create_surface_unsafe: {e}"))?
    };

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        })
        .await
        .map_err(|e| format!("no compatible wgpu adapter: {e}"))?;

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("hayate-android"),
            ..Default::default()
        })
        .await
        .map_err(|e| format!("request_device: {e}"))?;

    let mut surface_config = surface
        .get_default_config(&adapter, width, height)
        .ok_or("surface not supported by adapter")?;
    surface_config.usage |= wgpu::TextureUsages::RENDER_ATTACHMENT;
    surface.configure(&device, &surface_config);

    let surface_format = surface_config.format;
    let target_view = create_target_view(&device, width, height);
    let blitter = create_blitter(&device, surface_format);
    let scene_renderer = VelloSceneRenderer::new(&device)?;

    Ok(GpuSurface {
        device,
        queue,
        surface,
        surface_config,
        target_view,
        blitter,
        width,
        height,
        scene_renderer,
    })
}

impl GpuSurface {
    fn render_clear(&mut self, scene: &SceneGraph) -> Result<(), String> {
        let target = VelloRenderTarget {
            device: &self.device,
            queue: &self.queue,
            target_view: &self.target_view,
            width: self.width,
            height: self.height,
        };
        self.scene_renderer
            .render_scene(scene, &target, CLEAR_COLOR, 1.0)?;

        let surface_texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Occluded => return Ok(()),
            other => return Err(format!("get_current_texture: {other:?}")),
        };

        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("hayate_android_blit"),
            });
        self.blitter
            .copy(&self.device, &mut encoder, &self.target_view, &surface_view);
        self.queue.submit(std::iter::once(encoder.finish()));
        surface_texture.present();
        Ok(())
    }

    #[allow(dead_code)]
    fn resize(&mut self, width: u32, height: u32) {
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
