#[cfg(feature = "layer-present")]
use std::collections::HashSet;

use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;
use wgpu::util::TextureBlitter;

#[cfg(feature = "layer-present")]
use hayate_core::ElementId;
use hayate_core::SceneGraph;
use hayate_scene_renderer_vello::{
    create_blitter, create_target_view, VelloRenderTarget, VelloSceneRenderer,
};
#[cfg(feature = "layer-present")]
use hayate_layer_compositor::{
    collect_layer_placements, extract_layer_scene, extract_root_scene, tunables, CompositeQuad,
    GpuBudget, LayerCompositor, LayerRasterizer, PresentPlanner,
};
#[cfg(feature = "layer-present")]
use hayate_scene_renderer_vello::layer_compositor::{
    CompositeTarget, LayerTexture, VelloLayerRasterizer, WgpuQuadCompositor,
};

use super::{js_to_anyhow, CanvasBackend, ClearColor, SceneRendererKind};

pub(crate) struct SelectedBackend {
    surface_host: VelloSurfaceHost,
    scene_renderer: VelloSceneRenderer,
    content_scale: f32,
    /// per-layer present（#690・ADR-0125/0127）。`layer-present` feature が ON のときだけ使う。
    /// dirty / 未キャッシュのレイヤだけ texture へ再 raster し、専用 wgpu compositor で合成する。
    #[cfg(feature = "layer-present")]
    planner: PresentPlanner,
    #[cfg(feature = "layer-present")]
    rasterizer: VelloLayerRasterizer,
    #[cfg(feature = "layer-present")]
    compositor: WgpuQuadCompositor,
    /// 前フレームのレイヤ集合。消えたレイヤ（transition 終了等）のキャッシュ面と台帳を掃除する。
    #[cfg(feature = "layer-present")]
    prev_layers: HashSet<ElementId>,
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

impl SelectedBackend {
    pub(crate) async fn init(canvas: HtmlCanvasElement) -> Result<Self, JsValue> {
        let surface_host = VelloSurfaceHost::init(canvas).await?;
        let mut scene_renderer = VelloSceneRenderer::new(surface_host.device())
            .map_err(|e| JsValue::from_str(&e))?;
        // init 直後・最初の実アプリフレーム前に vello パイプラインを warmup する（#644）。ブラウザ
        // （Dawn）は非同期にパイプラインをコンパイルするため、warmup が無いと初回タップ/スクロールの
        // フレームにコンパイル遅延が乗る。warmup の失敗は boot を落とさず、警告のみで続行する
        // （初回フレームで従来どおりコンパイル遅延が出るだけで、描画は壊れない）。
        if let Err(e) = scene_renderer.warmup(surface_host.device(), surface_host.queue()) {
            web_sys::console::warn_1(&JsValue::from_str(&format!("vello warmup skipped: {e}")));
        }
        #[cfg(feature = "layer-present")]
        let rasterizer = VelloLayerRasterizer::new(
            surface_host.device().clone(),
            surface_host.queue().clone(),
            surface_host.width,
            surface_host.height,
            1.0,
        )
        .map_err(|e| JsValue::from_str(&e))?;
        #[cfg(feature = "layer-present")]
        let mut compositor =
            WgpuQuadCompositor::new(surface_host.device().clone(), surface_host.queue().clone());
        // init 時に全パイプライン variant を前倒し生成し、初回合成フレームの遅延生成スパイクを
        // 消す（ADR-0130a）。
        #[cfg(feature = "layer-present")]
        compositor.warmup();
        Ok(Self {
            surface_host,
            scene_renderer,
            content_scale: 1.0,
            #[cfg(feature = "layer-present")]
            planner: PresentPlanner::new(),
            #[cfg(feature = "layer-present")]
            rasterizer,
            #[cfg(feature = "layer-present")]
            compositor,
            #[cfg(feature = "layer-present")]
            prev_layers: HashSet::new(),
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
        let blitter = create_blitter(&device, surface_format);

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

    #[allow(dead_code)]
    fn target_view(&self) -> &wgpu::TextureView {
        &self.target_view
    }

    fn render_target(&self) -> VelloRenderTarget<'_> {
        VelloRenderTarget {
            device: &self.device,
            queue: &self.queue,
            target_view: &self.target_view,
            width: self.width,
            height: self.height,
        }
    }

    /// 次に present するサーフェス texture とその view を取得する。`Occluded` は「今フレームは
    /// 何もしない」を表すため `Ok(None)` を返す（present_target/present_layers 共通の分岐）。
    fn acquire_surface_view(&self) -> Result<Option<(wgpu::SurfaceTexture, wgpu::TextureView)>, JsValue> {
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

    fn present_target(&mut self) -> Result<(), JsValue> {
        let Some((surface_texture, surface_view)) = self.acquire_surface_view()? else {
            return Ok(());
        };
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

    /// per-layer present（#690）: レイヤ texture quad を専用 wgpu compositor でサーフェスへ直接
    /// 合成する（`target_view`/`blitter` は使わない——合成が単一 blit の代わりを担う）。
    #[cfg(feature = "layer-present")]
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
        self.target_view = create_target_view(&self.device, width, height);
    }
}

impl CanvasBackend for SelectedBackend {
    fn kind(&self) -> SceneRendererKind {
        SceneRendererKind::Vello
    }

    fn render_scene(&mut self, scene: &SceneGraph, clear_color: ClearColor) -> Result<(), anyhow::Error> {
        let target = self.surface_host.render_target();
        self.scene_renderer
            .render_scene(scene, &target, clear_color, self.content_scale)
            .map_err(|e| anyhow::anyhow!(e))?;
        self.surface_host.present_target().map_err(js_to_anyhow)
    }

    fn clear(&mut self, clear_color: ClearColor) -> Result<(), anyhow::Error> {
        self.render_scene(&SceneGraph::new(), clear_color)
    }

    #[cfg(feature = "layer-present")]
    fn supports_layer_present(&self) -> bool {
        true
    }

    /// per-layer present（#690・ADR-0125/0127）。Android の旧 `GpuSurface::render_frame`
    /// （#687 で撤去済み、per-layer 実装コード自体は撤去していない）と同型のロジック:
    /// (1) 消えたレイヤの掃除 (2) dirty / 未キャッシュのレイヤだけ vello でレイヤ texture へ
    /// raster (3) 専用 wgpu compositor で quad 合成しつつ present (4) GPU 予算超過分を LRU 退避。
    #[cfg(feature = "layer-present")]
    fn present_layers(
        &mut self,
        scene: &SceneGraph,
        layers: &[ElementId],
        layer_dirty: &HashSet<ElementId>,
        clear_color: ClearColor,
    ) -> Result<(), anyhow::Error> {
        let Some(&root) = layers.first() else {
            return Ok(());
        };
        let boundaries: HashSet<ElementId> = layers.iter().copied().collect();

        for stale in self.prev_layers.difference(&boundaries).copied().collect::<Vec<_>>() {
            self.rasterizer.discard(stale);
            self.planner.evict(stale);
        }
        self.prev_layers = boundaries.clone();

        let plan = self.planner.plan_layers(layers, layer_dirty);
        for &layer in &plan.raster {
            let extracted = if layer == root {
                extract_root_scene(scene, root, &boundaries)
            } else {
                match extract_layer_scene(scene, layer, &boundaries) {
                    Some(extracted) => extracted,
                    None => continue, // 未 lowering（次フレームで raster される）
                }
            };
            self.rasterizer
                .rasterize(layer, &extracted)
                .map_err(|e| anyhow::anyhow!(e))?;
            self.planner
                .note_layer_rasterized(layer, self.rasterizer.texture_bytes_per_layer());
        }

        let placements = collect_layer_placements(scene, root, &boundaries);
        let quads: Vec<CompositeQuad<'_, LayerTexture>> = placements
            .iter()
            .filter_map(|placement| {
                self.rasterizer.texture(placement.layer).map(|texture| CompositeQuad {
                    layer: placement.layer,
                    transform: placement.transform,
                    opacity: 1.0,
                    clip: placement.clip,
                    texture,
                })
            })
            .collect();
        self.surface_host
            .present_composite(&mut self.compositor, &quads, clear_color)
            .map_err(js_to_anyhow)?;
        for quad in &quads {
            self.planner.note_composited(quad.layer);
        }

        // GPU 予算超過なら最も長く composite に使われていないレイヤ texture から LRU 退避する
        // （ADR-0127）。Web はデスクトップ既定値をそのまま使う（このスライスでチューニングしない）。
        let budget = GpuBudget::from_viewports(
            self.surface_host.width,
            self.surface_host.height,
            tunables::GPU_BUDGET_VIEWPORTS_DESKTOP,
        );
        for evicted in self.planner.enforce_budget(budget) {
            self.rasterizer.discard(evicted);
        }
        Ok(())
    }

    fn resize(&mut self, width: u32, height: u32, content_scale: f32) {
        self.content_scale = content_scale.max(1.0);
        self.surface_host.resize(width, height);
        #[cfg(feature = "layer-present")]
        {
            // レイヤ texture はサーフェスサイズなので作り直し＝台帳ごと invalidate する
            // （invalidate しないと古いサイズの内容を合成し続ける）。
            self.rasterizer
                .resize(self.surface_host.width, self.surface_host.height, self.content_scale);
            self.planner.invalidate();
        }
    }
}
