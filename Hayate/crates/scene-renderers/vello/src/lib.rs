pub mod layer_compositor;
mod painter;

use std::num::NonZeroUsize;

use hayate_core::{
    render_scene_graph, RenderImage, RenderImageAlphaType, RenderImageFormat, SceneGraph,
    ScenePainter,
};
use vello::{
    peniko::{ImageAlphaType, ImageData, ImageFormat},
    AaConfig, AaSupport, RenderParams, Renderer, RendererOptions, Scene,
};
use vello::peniko::color::{AlphaColor, Srgb};
use wgpu::util::TextureBlitter;

// ADR-0054: ScenePainter は crate 内部 seam。host 向け公開契約ではない。
use painter::VelloPainter;

pub struct VelloRenderTarget<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub target_view: &'a wgpu::TextureView,
    pub width: u32,
    pub height: u32,
}

pub struct VelloSceneRenderer {
    renderer: Renderer,
}

impl VelloSceneRenderer {
    pub fn new(device: &wgpu::Device) -> Result<Self, String> {
        let renderer = Renderer::new(
            device,
            RendererOptions {
                use_cpu: false,
                antialiasing_support: AaSupport::area_only(),
                num_init_threads: NonZeroUsize::new(1),
                pipeline_cache: None,
            },
        )
        .map_err(|e| format!("Vello init failed: {e}"))?;
        Ok(Self { renderer })
    }

    pub fn render_scene(
        &mut self,
        graph: &SceneGraph,
        target: &VelloRenderTarget<'_>,
        clear_color: [f32; 4],
        content_scale: f32,
    ) -> Result<(), String> {
        let mut scene = Scene::new();
        {
            let mut painter = VelloPainter::new(&mut scene);
            let scaled = content_scale != 1.0;
            if scaled {
                painter.push_transform([
                    content_scale as f64,
                    0.0,
                    0.0,
                    content_scale as f64,
                    0.0,
                    0.0,
                ]);
            }
            render_scene_graph(graph, &mut painter);
            if scaled {
                painter.pop_transform();
            }
        }
        self.renderer
            .render_to_texture(
                target.device,
                target.queue,
                &scene,
                target.target_view,
                &RenderParams {
                    base_color: AlphaColor::<Srgb>::new(clear_color),
                    width: target.width,
                    height: target.height,
                    antialiasing_method: AaConfig::Area,
                },
            )
            .map_err(|e| format!("render_to_texture: {e}"))
    }
}

// perf プローブ用 seam（`tests/perf_probe.rs`）：GPU なしで「SceneGraph → vello Scene
// エンコード」だけの所要時間を測るために公開する。公開契約ではない。
#[doc(hidden)]
pub fn debug_encode_scene(graph: &SceneGraph, content_scale: f32) -> Scene {
    let mut scene = Scene::new();
    {
        let mut painter = VelloPainter::new(&mut scene);
        let scaled = content_scale != 1.0;
        if scaled {
            painter.push_transform([
                content_scale as f64,
                0.0,
                0.0,
                content_scale as f64,
                0.0,
                0.0,
            ]);
        }
        render_scene_graph(graph, &mut painter);
        if scaled {
            painter.pop_transform();
        }
    }
    scene
}

pub fn create_target_view(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    let target_texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("hayate_vello_target"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    target_texture.create_view(&wgpu::TextureViewDescriptor::default())
}

pub fn create_blitter(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> TextureBlitter {
    TextureBlitter::new(device, surface_format)
}

pub(crate) fn to_vello_image(image: &RenderImage) -> ImageData {
    let format = match image.format {
        RenderImageFormat::Rgba8 => ImageFormat::Rgba8,
    };
    let alpha_type = match image.alpha_type {
        RenderImageAlphaType::Opaque | RenderImageAlphaType::Alpha => ImageAlphaType::Alpha,
        RenderImageAlphaType::Premultiplied => ImageAlphaType::AlphaPremultiplied,
    };
    ImageData {
        // `RenderImage` が保持する Blob をそのまま共有する（コピー無し）。id が
        // 安定するので vello の画像アトラスにヒットし、再アップロードされない。
        data: image.data.clone(),
        format,
        alpha_type,
        width: image.width,
        height: image.height,
    }
}
