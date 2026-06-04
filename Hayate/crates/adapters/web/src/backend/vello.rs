use std::num::NonZeroUsize;
use std::sync::Arc;

use hayate_core::{
    NodeId, NodeKind, RenderImage, RenderImageAlphaType, RenderImageFormat, SceneGraph,
};
use vello::{
    AaConfig, AaSupport, RenderParams, Renderer, RendererOptions, Scene,
    kurbo::{Affine, Rect, RoundedRect},
    peniko::{
        Blob, Fill, FontData, ImageAlphaType, ImageBrush, ImageData, ImageFormat,
        color::{AlphaColor, Srgb},
    },
};
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;
use wgpu::util::TextureBlitter;

use super::{CanvasBackend, ClearColor, SceneRendererKind};

pub(crate) struct SelectedBackend {
    surface_host: VelloSurfaceHost,
    scene_renderer: VelloSceneRenderer,
}

struct VelloSurfaceHost {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    target_view: wgpu::TextureView,
    blitter: TextureBlitter,
    width: u32,
    height: u32,
}

struct VelloSceneRenderer {
    renderer: Renderer,
}

/// Create the off-screen RGBA8 texture Vello renders into before it is blitted
/// to the surface. Used at init and on every resize.
fn create_target_view(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
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

impl SelectedBackend {
    pub(crate) async fn init(canvas: HtmlCanvasElement) -> Result<Self, JsValue> {
        let surface_host = VelloSurfaceHost::init(canvas).await?;
        let scene_renderer = VelloSceneRenderer::new(surface_host.device())?;
        Ok(Self {
            surface_host,
            scene_renderer,
        })
    }
}

impl VelloSurfaceHost {
    async fn init(canvas: HtmlCanvasElement) -> Result<Self, JsValue> {
        let width = canvas.width();
        let height = canvas.height();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });

        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .map_err(|e| JsValue::from_str(&format!("WebGPU adapter not found: {e}")))?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("hayate"),
                ..Default::default()
            })
            .await
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let mut surface_config = surface
            .get_default_config(&adapter, width, height)
            .ok_or_else(|| JsValue::from_str("surface not supported by adapter"))?;
        surface_config.usage |= wgpu::TextureUsages::RENDER_ATTACHMENT;
        surface.configure(&device, &surface_config);

        let surface_format = surface_config.format;
        let target_view = create_target_view(&device, width, height);
        let blitter = TextureBlitter::new(&device, surface_format);

        Ok(Self {
            device,
            queue,
            surface,
            surface_config,
            target_view,
            blitter,
            width,
            height,
        })
    }

    fn device(&self) -> &wgpu::Device {
        &self.device
    }

    fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    fn target_view(&self) -> &wgpu::TextureView {
        &self.target_view
    }

    fn render_params(&self, clear_color: ClearColor) -> RenderParams {
        RenderParams {
            base_color: AlphaColor::<Srgb>::new(clear_color),
            width: self.width,
            height: self.height,
            antialiasing_method: AaConfig::Area,
        }
    }

    fn present_target(&mut self) -> Result<(), JsValue> {
        let surface_texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Timeout => {
                return Err(JsValue::from_str("get_current_texture: timeout"));
            }
            wgpu::CurrentSurfaceTexture::Occluded => return Ok(()),
            wgpu::CurrentSurfaceTexture::Outdated => {
                return Err(JsValue::from_str("get_current_texture: surface outdated"));
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                return Err(JsValue::from_str("get_current_texture: surface lost"));
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err(JsValue::from_str("get_current_texture: validation error"));
            }
        };

        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("hayate_blit"),
            });
        self.blitter
            .copy(&self.device, &mut encoder, &self.target_view, &surface_view);
        self.queue.submit(std::iter::once(encoder.finish()));
        surface_texture.present();
        Ok(())
    }

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

impl VelloSceneRenderer {
    fn new(device: &wgpu::Device) -> Result<Self, JsValue> {
        let renderer = Renderer::new(
            device,
            RendererOptions {
                use_cpu: false,
                antialiasing_support: AaSupport::area_only(),
                num_init_threads: NonZeroUsize::new(1),
                pipeline_cache: None,
            },
        )
        .map_err(|e| JsValue::from_str(&format!("Vello init failed: {e}")))?;
        Ok(Self { renderer })
    }

    fn render_to_target(
        &mut self,
        surface_host: &VelloSurfaceHost,
        scene: &Scene,
        clear_color: ClearColor,
    ) -> Result<(), JsValue> {
        self.renderer
            .render_to_texture(
                surface_host.device(),
                surface_host.queue(),
                scene,
                surface_host.target_view(),
                &surface_host.render_params(clear_color),
            )
            .map_err(|e| JsValue::from_str(&format!("render_to_texture: {e}")))
    }
}

impl CanvasBackend for SelectedBackend {
    fn kind(&self) -> SceneRendererKind {
        SceneRendererKind::Vello
    }

    fn render_scene(&mut self, scene: &SceneGraph, clear_color: ClearColor) -> Result<(), JsValue> {
        let encoded = encode_scene(scene);
        self.scene_renderer
            .render_to_target(&self.surface_host, &encoded, clear_color)?;
        self.surface_host.present_target()
    }

    fn clear(&mut self, clear_color: ClearColor) -> Result<(), JsValue> {
        self.render_scene(&SceneGraph::new(), clear_color)
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.surface_host.resize(width, height);
    }
}

fn encode_scene(graph: &SceneGraph) -> Scene {
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
            let font = FontData::new(data.font.data.clone(), data.font.index);
            let glyphs = data.glyphs.iter().map(|glyph| vello::Glyph {
                id: glyph.id,
                x: glyph.x,
                y: glyph.y,
            });
            scene
                .draw_glyphs(&font)
                .font_size(data.font_size)
                .brush(brush)
                .transform(Affine::translate((*x as f64, *y as f64)))
                .draw(Fill::NonZero, glyphs);
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
            let brush = ImageBrush::new(to_vello_image(data));
            scene.draw_image(&brush, transform);
        }
        NodeKind::Group { transform } => {
            let affine = Affine::new(*transform);
            let mut sub = Scene::new();
            for &child_id in &node.children {
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
            scene.push_clip_layer(Fill::NonZero, Affine::IDENTITY, &clip);
            for &child_id in &node.children {
                draw_node(graph, child_id, scene);
            }
            scene.pop_layer();
        }
    }
}

fn to_vello_image(image: &RenderImage) -> ImageData {
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
