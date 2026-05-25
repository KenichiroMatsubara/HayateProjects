use std::num::NonZeroUsize;

use hayate_core::{
    Node, NodeKind, SceneGraph,
    vello_bridge,
};
use slotmap::{Key, KeyData};
use vello::{
    AaConfig, AaSupport, RenderParams, Renderer, RendererOptions, Scene,
    peniko::color::AlphaColor,
    peniko::color::Srgb,
};
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;
use wgpu::util::TextureBlitter;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Debug);
}

/// Holds GPU state for a single canvas.
#[wasm_bindgen]
pub struct HayateRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    renderer: Renderer,
    target_view: wgpu::TextureView,
    blitter: TextureBlitter,
    width: u32,
    height: u32,
    scene_graph: SceneGraph,
}

#[wasm_bindgen]
impl HayateRenderer {
    /// Initialise wgpu (WebGPU) + Vello from an HTML canvas element.
    /// Returns a `Promise<HayateRenderer>` because GPU requests are async.
    pub async fn init(canvas: HtmlCanvasElement) -> Result<HayateRenderer, JsValue> {
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

        // Vello renders into an intermediate Rgba8Unorm texture, then we blit to the surface.
        let target_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("hayate_vello_target"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let target_view = target_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let renderer = Renderer::new(
            &device,
            RendererOptions {
                use_cpu: false,
                antialiasing_support: AaSupport::area_only(),
                num_init_threads: NonZeroUsize::new(1),
                pipeline_cache: None,
            },
        )
        .map_err(|e| JsValue::from_str(&format!("Vello init failed: {e}")))?;

        let blitter = TextureBlitter::new(&device, surface_format);

        log::info!("Hayate renderer initialised ({width}x{height}, format={surface_format:?})");

        Ok(HayateRenderer {
            device,
            queue,
            surface,
            renderer,
            target_view,
            blitter,
            width,
            height,
            scene_graph: SceneGraph::new(),
        })
    }

    /// Add a Rect node to the scene graph. Returns an opaque node ID (as f64).
    /// Color components and position are in logical pixels / 0.0–1.0 range for RGBA.
    pub fn node_create(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        r: f32,
        g: f32,
        b: f32,
        a: f32,
        corner_radius: f32,
    ) -> f64 {
        let node = Node {
            kind: NodeKind::Rect { x, y, width, height, color: [r, g, b, a], corner_radius },
            children: Vec::new(),
        };
        let id = self.scene_graph.insert(node);
        id.data().as_ffi() as f64
    }

    /// Remove a node previously created with node_create.
    pub fn node_remove(&mut self, raw_id: f64) {
        use hayate_core::node::NodeId;
        let key_data = KeyData::from_ffi(raw_id as u64);
        let id = NodeId::from(key_data);
        self.scene_graph.remove(id);
    }

    /// Render the current scene graph to the canvas.
    pub fn render(&mut self, bg_r: f64, bg_g: f64, bg_b: f64) -> Result<(), JsValue> {
        let base_color = AlphaColor::<Srgb>::new([bg_r as f32, bg_g as f32, bg_b as f32, 1.0]);
        let scene = vello_bridge::build_scene(&self.scene_graph);
        self.present_scene(&scene, base_color)
    }

    /// Clear the canvas to an RGB solid colour (components in 0.0 – 1.0).
    pub fn clear(&mut self, r: f64, g: f64, b: f64) -> Result<(), JsValue> {
        let base_color = AlphaColor::<Srgb>::new([r as f32, g as f32, b as f32, 1.0]);
        let scene = Scene::new();
        self.present_scene(&scene, base_color)
    }

    fn present_scene(
        &mut self,
        scene: &Scene,
        base_color: AlphaColor<Srgb>,
    ) -> Result<(), JsValue> {
        self.renderer
            .render_to_texture(
                &self.device,
                &self.queue,
                scene,
                &self.target_view,
                &RenderParams {
                    base_color,
                    width: self.width,
                    height: self.height,
                    antialiasing_method: AaConfig::Area,
                },
            )
            .map_err(|e| JsValue::from_str(&format!("render_to_texture: {e}")))?;

        let surface_texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Timeout => {
                return Err(JsValue::from_str("get_current_texture: timeout"))
            }
            wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                return Err(JsValue::from_str("get_current_texture: surface outdated"))
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                return Err(JsValue::from_str("get_current_texture: surface lost"))
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err(JsValue::from_str("get_current_texture: validation error"))
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
}
