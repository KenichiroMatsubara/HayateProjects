mod painter;

use std::num::NonZeroUsize;
use std::sync::Arc;

use hayate_core::{render_scene_graph, RenderImage, RenderImageAlphaType, RenderImageFormat, SceneGraph};
use vello::{
    peniko::{Blob, ImageAlphaType, ImageData, ImageFormat},
    AaConfig, AaSupport, RenderParams, Renderer, RendererOptions, Scene,
};
use vello::peniko::color::{AlphaColor, Srgb};
use wgpu::util::TextureBlitter;

pub use painter::VelloPainter;

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
    ) -> Result<(), String> {
        let mut scene = Scene::new();
        {
            let mut painter = VelloPainter::new(&mut scene);
            render_scene_graph(graph, &mut painter);
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
        data: Blob::new(Arc::new(image.data.as_ref().to_vec().into_boxed_slice())),
        format,
        alpha_type,
        width: image.width,
        height: image.height,
    }
}
