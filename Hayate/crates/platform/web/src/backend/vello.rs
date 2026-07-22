use std::collections::HashMap;

use super::{js_to_anyhow, CanvasBackend, ClearColor, SceneRendererKind};
use hayate_core::{ElementId, LayerTopology, SceneSnapshot};
use hayate_layer_compositor::{
    CompositeQuad, DeviceMemoryClass, GpuBudget, LayerCompositor, LayerPresentation,
    LayerPresentationAdapter, LayerPresentationFrame, LayerRasterizer, MemoryPressure,
    PlacementPlan, RasterJob, RasterJobKind, RenderResourceBudgetPolicy, ResidencyEvent,
    ResourceBudgetInputs, ScrollLayerGeometry,
};
use hayate_scene_renderer_vello::layer_compositor::{
    CompositeTarget, LayerTexture, VelloLayerRasterizer, WgpuQuadCompositor,
};
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

/// Vello resources owned by the retained Layer Presentation path and initialized at boot.
struct LayerPresentState {
    presentation: LayerPresentation,
    rasterizer: VelloLayerRasterizer,
    compositor: WgpuQuadCompositor,
}

impl LayerPresentState {
    fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        width: u32,
        height: u32,
        content_scale: f32,
    ) -> Result<Self, String> {
        let rasterizer =
            VelloLayerRasterizer::new(device.clone(), queue.clone(), width, height, content_scale)?;
        let mut compositor = WgpuQuadCompositor::new(device, queue);
        compositor.set_content_scale(content_scale);
        // construct 時に全パイプライン variant を前倒し生成し、初回合成フレームの遅延生成
        // スパイクを消す（ADR-0130a）。
        compositor.warmup();
        Ok(Self {
            presentation: LayerPresentation::new(),
            rasterizer,
            compositor,
        })
    }
}

pub(crate) struct SelectedBackend {
    surface_host: VelloSurfaceHost,
    content_scale: f32,
    layer_present: LayerPresentState,
    resource_policy: RenderResourceBudgetPolicy,
}

struct VelloSurfaceHost {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    width: u32,
    height: u32,
}

impl SelectedBackend {
    pub(crate) async fn init(canvas: HtmlCanvasElement) -> Result<Self, JsValue> {
        let surface_host = VelloSurfaceHost::init(canvas).await?;
        let layer_present = LayerPresentState::new(
            surface_host.device().clone(),
            surface_host.queue().clone(),
            surface_host.width,
            surface_host.height,
            1.0,
        )
        .map_err(|error| JsValue::from_str(&format!("vello layer presentation: {error}")))?;
        Ok(Self {
            resource_policy: RenderResourceBudgetPolicy::for_device(ResourceBudgetInputs::new(
                DeviceMemoryClass::Balanced,
                surface_host.width,
                surface_host.height,
            )),
            surface_host,
            content_scale: 1.0,
            layer_present,
        })
    }

    fn enforce_resource_budget(&mut self, max_bytes: u64) {
        let (surface_host, state) = (&mut self.surface_host, &mut self.layer_present);
        let mut adapter = VelloLayerPresentationAdapter {
            rasterizer: &mut state.rasterizer,
            compositor: &mut state.compositor,
            surface_host,
            clear: [0.0; 4],
        };
        state
            .presentation
            .enforce_budget(GpuBudget::from_bytes(max_bytes), &mut adapter);
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

        Ok(Self {
            device,
            queue,
            surface,
            surface_config,
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

    /// 次に present するサーフェス texture とその view を取得する。`Occluded` は「今フレームは
    /// 何もしない」を表すため `Ok(None)` を返す（present_target/present_layers 共通の分岐）。
    fn acquire_surface_view(
        &self,
    ) -> Result<Option<(wgpu::SurfaceTexture, wgpu::TextureView)>, JsValue> {
        let surface_texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Timeout => {
                return Err(JsValue::from_str("get_current_texture: timeout"));
            }
            wgpu::CurrentSurfaceTexture::Occluded => return Ok(None),
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
        Ok(Some((surface_texture, surface_view)))
    }

    /// per-layer present（#690）: レイヤ texture quad を専用 wgpu compositor でサーフェスへ直接
    /// 合成する（`target_view`/`blitter` は使わない——合成が単一 blit の代わりを担う）。
    fn present_composite(
        &mut self,
        compositor: &mut WgpuQuadCompositor,
        quads: &[CompositeQuad<'_, LayerTexture>],
        clear_color: ClearColor,
    ) -> Result<(), JsValue> {
        let Some((surface_texture, surface_view)) = self.acquire_surface_view()? else {
            return Ok(());
        };
        let mut target = CompositeTarget {
            view: surface_view,
            width: self.width,
            height: self.height,
            format: self.surface_config.format,
            clear: clear_color,
        };
        compositor
            .composite(&mut target, quads)
            .map_err(|e| JsValue::from_str(&e))?;
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
    }
}

/// Web Vello's resource adapter for the shared layer-presentation transaction. The transaction
/// owns frame validation, planning, stale detection, cache ledger and LRU; this adapter owns only
/// the WGPU/Vello resources and the surface-present step.
struct VelloLayerPresentationAdapter<'a> {
    rasterizer: &'a mut VelloLayerRasterizer,
    compositor: &'a mut WgpuQuadCompositor,
    surface_host: &'a mut VelloSurfaceHost,
    clear: ClearColor,
}

impl LayerPresentationAdapter for VelloLayerPresentationAdapter<'_> {
    type Error = String;

    fn rasterize(&mut self, job: &RasterJob<'_>) -> Result<u64, Self::Error> {
        match job.kind {
            RasterJobKind::Content => match job.bounds {
                Some(bounds) => self
                    .rasterizer
                    .rasterize_in_bounds(job.layer, job.scene, bounds, job.band)?,
                None => self.rasterizer.rasterize(job.layer, job.scene, job.band)?,
            },
            RasterJobKind::ScrollChrome => match job.bounds {
                Some(bounds) => {
                    self.rasterizer.update_scroll_chrome_in_bounds(
                        job.layer,
                        job.scene,
                        bounds,
                        job.repaint,
                    )?;
                }
                None => {
                    self.rasterizer
                        .update_scroll_chrome(job.layer, job.scene, job.repaint)?;
                }
            },
        }
        Ok(self.rasterizer.cache_bytes(job.layer))
    }

    fn composite(&mut self, plan: &PlacementPlan) -> Result<(), Self::Error> {
        let mut quads = Vec::with_capacity(plan.planes.len());
        for plane in &plan.planes {
            let texture = match plane.kind {
                RasterJobKind::Content => self.rasterizer.texture(plane.layer),
                RasterJobKind::ScrollChrome => self.rasterizer.scroll_chrome_texture(plane.layer),
            };
            if let Some(texture) = texture {
                quads.push(CompositeQuad {
                    layer: plane.layer,
                    transform: plane.transform,
                    opacity: 1.0,
                    clip: plane.clip,
                    texture,
                });
            }
        }
        self.surface_host
            .present_composite(self.compositor, &quads, self.clear)
            .map_err(|error| error.as_string().unwrap_or_else(|| format!("{error:?}")))
    }

    fn discard(&mut self, layers: &[ElementId]) {
        for &layer in layers {
            self.rasterizer.discard(layer);
        }
    }
}

impl CanvasBackend for SelectedBackend {
    fn kind(&self) -> SceneRendererKind {
        SceneRendererKind::Vello
    }

    fn configure_resource_residency(&mut self, policy: RenderResourceBudgetPolicy) {
        self.resource_policy = policy;
        self.layer_present
            .rasterizer
            .configure_resource_residency(policy);
        self.enforce_resource_budget(policy.gpu.max_bytes);
    }

    fn handle_resource_lifecycle(&mut self, event: ResidencyEvent) {
        self.layer_present
            .rasterizer
            .handle_resource_lifecycle(event);
        match event {
            ResidencyEvent::MemoryPressure(MemoryPressure::Moderate) => {
                self.enforce_resource_budget(self.resource_policy.gpu.low_watermark_bytes);
            }
            ResidencyEvent::SurfaceLost
            | ResidencyEvent::ContextLost
            | ResidencyEvent::Shutdown => {
                self.layer_present.presentation.invalidate();
            }
        }
    }

    fn clear(&mut self, clear_color: ClearColor) -> Result<(), anyhow::Error> {
        let (surface_host, state) = (&mut self.surface_host, &mut self.layer_present);
        surface_host
            .present_composite(&mut state.compositor, &[], clear_color)
            .map_err(js_to_anyhow)
    }

    /// per-layer present（#690・ADR-0125/0127、scroll overscan サイジング配線 #707）。Android の旧
    /// `GpuSurface::render_frame`（#687 で撤去済み、per-layer 実装コード自体は撤去していない）と
    /// 同型のロジック: (1) 消えたレイヤの掃除 (2) dirty / 未キャッシュの非 scroll レイヤだけ vello
    /// でレイヤ texture へ raster (2b) scroll 内容レイヤ（`scroll_geometry` にあるレイヤ）は帯
    /// カバレッジで別途 gating し、必要なら overscan 帯サイズで raster する——scroll offset だけの
    /// 変化は `layer_dirty`（content dirty）に含まれない（chrome dirty 扱い、#634）ため、非 scroll
    /// と同じ一括判定に混ぜると「offset が変わっても帯はまだ可視域を覆っている」composite-only
    /// フレームを見逃す (3) 専用 wgpu compositor で quad 合成しつつ present——scroll レイヤの quad
    /// は、texture が絶対シーン座標の一部（帯）しか持たないぶんの compensating translate を追加で
    /// 持つ (4) GPU 予算超過分を LRU 退避（scroll レイヤは帯サイズのバイト数で計上、#707）。
    fn present_layers(
        &mut self,
        scene: &SceneSnapshot,
        topology: &LayerTopology,
        scroll_geometry: &HashMap<ElementId, ScrollLayerGeometry>,
        clear_color: ClearColor,
    ) -> Result<(), anyhow::Error> {
        let (surface_host, layer_present) = (&mut self.surface_host, &mut self.layer_present);
        let state = layer_present;
        let mut adapter = VelloLayerPresentationAdapter {
            rasterizer: &mut state.rasterizer,
            compositor: &mut state.compositor,
            surface_host,
            clear: clear_color,
        };
        state
            .presentation
            .present(
                LayerPresentationFrame {
                    snapshot: scene,
                    topology,
                    scroll_geometry,
                },
                &mut adapter,
            )
            .map_err(|error| anyhow::anyhow!("layer presentation: {error:?}"))?;
        let budget = GpuBudget::from_bytes(self.resource_policy.gpu.max_bytes);
        state.presentation.enforce_budget(budget, &mut adapter);
        Ok(())
    }

    fn resize(&mut self, width: u32, height: u32, content_scale: f32) {
        self.content_scale = content_scale.max(1.0);
        self.surface_host.resize(width, height);
        let state = &mut self.layer_present;
        state.rasterizer.resize(
            self.surface_host.width,
            self.surface_host.height,
            self.content_scale,
        );
        state.compositor.set_content_scale(self.content_scale);
        state.presentation.invalidate();
    }
}
