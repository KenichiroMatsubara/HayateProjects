use std::collections::{HashMap, HashSet};

use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;
use wgpu::util::TextureBlitter;

use hayate_core::{ElementId, LayerRasterBounds, SceneGraph};
use hayate_layer_compositor::{
    collect_layer_placements, extract_layer_scene, extract_root_scene, extract_scroll_chrome_scene,
    extract_scroll_layer_scene, layer_scene::compose, tunables, CompositeQuad, GpuBudget,
    LayerCompositor, LayerPlacement, LayerRasterizer, PresentPlanner, ScrollLayerGeometry,
};
use hayate_scene_renderer_vello::layer_compositor::{
    CompositeTarget, LayerTexture, VelloLayerRasterizer, WgpuQuadCompositor,
};
use hayate_scene_renderer_vello::{
    create_blitter, create_target_view, VelloRenderTarget, VelloSceneRenderer,
};

use super::{js_to_anyhow, CanvasBackend, ClearColor, SceneRendererKind};

/// per-layer present（#690・ADR-0125/0127）の GPU リソース。`VelloLayerRasterizer`（GPU
/// device/queue を握る）・`WgpuQuadCompositor`（`warmup()` で GPU パイプラインを前倒しコンパイル
/// する、ADR-0130a）という実 GPU リソースを伴うため、`SelectedBackend::ensure_layer_present_resources`
/// が `set_layer_present_enabled(true)` の初回呼び出しでのみ construct・warmup する
/// （ADR-0140・#718 の遅延初期化）。
struct LayerPresentState {
    planner: PresentPlanner,
    rasterizer: VelloLayerRasterizer,
    compositor: WgpuQuadCompositor,
    /// 前フレームのレイヤ集合。消えたレイヤ（transition 終了等）のキャッシュ面と台帳を掃除する。
    prev_layers: HashSet<ElementId>,
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
            planner: PresentPlanner::new(),
            rasterizer,
            compositor,
            prev_layers: HashSet::new(),
        })
    }
}

pub(crate) struct SelectedBackend {
    surface_host: VelloSurfaceHost,
    scene_renderer: VelloSceneRenderer,
    content_scale: f32,
    /// ADR-0138/ADR-0140 比較用トグル。既定 ON（#690 の per-layer 経路を維持）——
    /// `HayateElementRenderer::init` の `layer_present_enabled` 引数で OFF にすると
    /// `supports_layer_present()` が false を返し、呼び出し側（`canvas.rs`）が全面
    /// `render_scene` にフォールバックする。「製品としては有効化しない」という ADR-0135 の
    /// 封印意図は、以後この既定値と本コメント・ADR-0140 に記録される（cargo feature という
    /// 物理的な仕組みではなく運用上の取り決め）。
    layer_present_enabled: bool,
    /// per-layer present（#690・ADR-0125/0127）の GPU リソース。`layer_present_enabled` が
    /// true でも、`set_layer_present_enabled(true)` が一度も呼ばれていなければ `None` のまま
    /// （ADR-0140 の遅延初期化 — `?layerPresent=0` を選ぶユーザーに不要な GPU 初期化コストを
    /// 払わせない）。
    layer_present: Option<LayerPresentState>,
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
        let mut scene_renderer =
            VelloSceneRenderer::new(surface_host.device()).map_err(|e| JsValue::from_str(&e))?;
        // init 直後・最初の実アプリフレーム前に vello パイプラインを warmup する（#644）。ブラウザ
        // （Dawn）は非同期にパイプラインをコンパイルするため、warmup が無いと初回タップ/スクロールの
        // フレームにコンパイル遅延が乗る。warmup の失敗は boot を落とさず、警告のみで続行する
        // （初回フレームで従来どおりコンパイル遅延が出るだけで、描画は壊れない）。
        if let Err(e) = scene_renderer.warmup(surface_host.device(), surface_host.queue()) {
            web_sys::console::warn_1(&JsValue::from_str(&format!("vello warmup skipped: {e}")));
        }
        Ok(Self {
            surface_host,
            scene_renderer,
            content_scale: 1.0,
            layer_present_enabled: true,
            // ADR-0140: GPU リソースは construct しない——`set_layer_present_enabled(true)`
            // （production では `HayateElementRenderer::init` が既定引数で必ずすぐ呼ぶ）が
            // 最初に呼ばれたとき・または `present_layers` が最初に実際に呼ばれたときに、
            // `ensure_layer_present_resources` が construct する。
            layer_present: None,
        })
    }

    /// per-layer 経路の GPU リソースを必要になった時点で construct・warmup する（ADR-0140）。
    /// construct 済みなら何もしない（再 construct・再 warmup しない）。construct 失敗時は
    /// vello scene warmup 失敗時と同じ「boot/フレームを落とさず警告ログのみで続行する」
    /// パターンに倣い、`layer_present_enabled` を false にして全面 raster にフォールバックする。
    fn ensure_layer_present_resources(&mut self) {
        if self.layer_present.is_some() {
            return;
        }
        match LayerPresentState::new(
            self.surface_host.device().clone(),
            self.surface_host.queue().clone(),
            self.surface_host.width,
            self.surface_host.height,
            self.content_scale,
        ) {
            Ok(state) => self.layer_present = Some(state),
            Err(e) => {
                web_sys::console::warn_1(&JsValue::from_str(&format!(
                    "vello layer-present init skipped, falling back to full-surface raster: {e}"
                )));
                self.layer_present_enabled = false;
            }
        }
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

/// The compositing transform for one layer `placement` (ADR-0127・#707). For a banded scroll
/// layer (present in both `planner`'s cache and `scroll_geometry`, this frame), this composes
/// `placement.transform` with this frame's profile-resolved scroll affine. The cached texture's
/// logical origin is applied by `WgpuQuadCompositor`, so the band origin is not repeated here.
/// The cached texture omits the live scroll Group, so this per-frame affine keeps ordinary
/// scrolling, iOS rubber translation, and Android stretch moving without a re-raster.
fn quad_transform(
    placement: &LayerPlacement,
    planner: &PresentPlanner,
    scroll_geometry: &HashMap<ElementId, ScrollLayerGeometry>,
) -> [f64; 6] {
    match (
        planner.cached_scroll_band(placement.layer),
        scroll_geometry.get(&placement.layer),
    ) {
        (Some(_), Some(geometry)) => compose(placement.transform, geometry.scroll_affine),
        _ => placement.transform,
    }
}

impl CanvasBackend for SelectedBackend {
    fn kind(&self) -> SceneRendererKind {
        SceneRendererKind::Vello
    }

    fn render_scene(
        &mut self,
        scene: &SceneGraph,
        clear_color: ClearColor,
    ) -> Result<(), anyhow::Error> {
        let target = self.surface_host.render_target();
        self.scene_renderer
            .render_scene(scene, &target, clear_color, self.content_scale)
            .map_err(|e| anyhow::anyhow!(e))?;
        self.surface_host.present_target().map_err(js_to_anyhow)
    }

    fn clear(&mut self, clear_color: ClearColor) -> Result<(), anyhow::Error> {
        self.render_scene(&SceneGraph::new(), clear_color)
    }

    fn supports_layer_present(&self) -> bool {
        self.layer_present_enabled
    }

    fn set_layer_present_enabled(&mut self, enabled: bool) {
        self.layer_present_enabled = enabled;
        if enabled {
            self.ensure_layer_present_resources();
        }
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
        scene: &SceneGraph,
        layers: &[ElementId],
        layer_raster_bounds: &[LayerRasterBounds],
        layer_dirty: &HashSet<ElementId>,
        chrome_dirty: &HashSet<ElementId>,
        scroll_geometry: &HashMap<ElementId, ScrollLayerGeometry>,
        clear_color: ClearColor,
    ) -> Result<(), anyhow::Error> {
        self.ensure_layer_present_resources();
        let Some(state) = self.layer_present.as_mut() else {
            // construct が失敗した直後（ensure_layer_present_resources が layer_present_enabled
            // を false に落とした）。このフレームは全面 raster にフォールバックする。
            return self.render_scene(scene, clear_color);
        };
        let Some(&root) = layers.first() else {
            return Ok(());
        };
        let boundaries: HashSet<ElementId> = layers.iter().copied().collect();
        let raster_bounds: HashMap<ElementId, LayerRasterBounds> = layer_raster_bounds
            .iter()
            .copied()
            .map(|bounds| (bounds.layer, bounds))
            .collect();

        for stale in state
            .prev_layers
            .difference(&boundaries)
            .copied()
            .collect::<Vec<_>>()
        {
            state.rasterizer.discard(stale);
            state.planner.evict(stale);
        }
        state.prev_layers = boundaries.clone();

        let extract = |layer: ElementId| -> Option<SceneGraph> {
            if layer == root {
                Some(extract_root_scene(scene, root, &boundaries))
            } else {
                extract_layer_scene(scene, layer, &boundaries)
            }
        };

        // 非 scroll レイヤは従来どおり: dirty / 未キャッシュのものだけ一括判定で raster する。
        // scroll レイヤ（`scroll_geometry` にあるもの）はここから除外し、下の専用ループで扱う。
        let non_scroll_layers: Vec<ElementId> = layers
            .iter()
            .copied()
            .filter(|layer| !scroll_geometry.contains_key(layer))
            .collect();
        let plan = state.planner.plan_layers(&non_scroll_layers, layer_dirty);
        for &layer in &plan.raster {
            let Some(extracted) = extract(layer) else {
                continue; // 未 lowering（次フレームで raster される）
            };
            if layer == root {
                state
                    .rasterizer
                    .rasterize(layer, &extracted, None)
                    .map_err(|e| anyhow::anyhow!(e))?;
            } else if let Some(&bounds) = raster_bounds.get(&layer) {
                state
                    .rasterizer
                    .rasterize_in_bounds(layer, &extracted, bounds, None)
                    .map_err(|e| anyhow::anyhow!(e))?;
            } else {
                state
                    .rasterizer
                    .rasterize(layer, &extracted, None)
                    .map_err(|e| anyhow::anyhow!(e))?;
            }
            state.planner.note_layer_rasterized(
                layer,
                state
                    .rasterizer
                    .texture_bytes(layer)
                    .unwrap_or_else(|| state.rasterizer.texture_bytes_per_layer()),
            );
        }

        // scroll 内容レイヤ（ADR-0127・#707）: キャッシュ済み帯が現在の可視域を覆っていれば
        // composite-only（raster しない）。覆っていなければ新しい帯を差分 raster する
        // （`compositor/tests/scroll_composite_only.rs` の `pump_scroll` と同じ判定順）。
        // `geometry.content_dirty`（`frame_layer_dirty()` のみ）を使う——呼び出し側が渡す
        // `layer_dirty` は非 scroll 経路のために scroll chrome dirty（スクロールバー fade 等）も
        // 混ぜているため、それをここで使うとスクロールバーが動くたびに composite-only が崩れる。
        for &layer in layers {
            let Some(geometry) = scroll_geometry.get(&layer) else {
                continue; // 非 scroll レイヤは上のループで処理済み
            };
            let needs_content_raster = geometry.content_dirty
                || state.planner.scroll_layer_needs_raster(
                    layer,
                    geometry.visible_top,
                    geometry.viewport_height,
                );
            if needs_content_raster {
                let Some(extracted) = (if layer == root {
                    Some(extract_root_scene(scene, root, &boundaries))
                } else {
                    extract_scroll_layer_scene(scene, layer, &boundaries, geometry.scroll_affine)
                }) else {
                    continue;
                };
                if layer == root {
                    state
                        .rasterizer
                        .rasterize(layer, &extracted, Some(geometry.raster_band()))
                        .map_err(|e| anyhow::anyhow!(e))?;
                } else if let Some(&bounds) = raster_bounds.get(&layer) {
                    state
                        .rasterizer
                        .rasterize_in_bounds(
                            layer,
                            &extracted,
                            bounds,
                            Some(geometry.raster_band()),
                        )
                        .map_err(|e| anyhow::anyhow!(e))?;
                } else {
                    state
                        .rasterizer
                        .rasterize(layer, &extracted, Some(geometry.raster_band()))
                        .map_err(|e| anyhow::anyhow!(e))?;
                }
            }

            // Scrollbar は viewport 固定 chrome であり、content band と同じ compensating
            // translate を掛けず別 texture として重ねる。CommittedFrame の chrome dirty または
            // cache miss/resize のときだけ更新し、stable frame では Vello raster を起動しない。
            if layer != root {
                if let Some(chrome) = extract_scroll_chrome_scene(scene, layer, &boundaries) {
                    if let Some(&bounds) = raster_bounds.get(&layer) {
                        state
                            .rasterizer
                            .update_scroll_chrome_in_bounds(
                                layer,
                                &chrome,
                                bounds,
                                chrome_dirty.contains(&layer),
                            )
                            .map_err(|e| anyhow::anyhow!(e))?;
                    } else {
                        state
                            .rasterizer
                            .update_scroll_chrome(layer, &chrome, chrome_dirty.contains(&layer))
                            .map_err(|e| anyhow::anyhow!(e))?;
                    }
                }
            }
            if needs_content_raster {
                let bytes = state.rasterizer.cache_bytes(layer);
                state
                    .planner
                    .note_scroll_rasterized(layer, geometry.band, bytes);
            }
        }

        let placements = collect_layer_placements(scene, root, &boundaries);
        let mut quads: Vec<CompositeQuad<'_, LayerTexture>> = Vec::new();
        for placement in &placements {
            if let Some(texture) = state.rasterizer.texture(placement.layer) {
                quads.push(CompositeQuad {
                    layer: placement.layer,
                    transform: quad_transform(placement, &state.planner, scroll_geometry),
                    opacity: 1.0,
                    clip: placement.clip,
                    texture,
                });
            }
            if scroll_geometry.contains_key(&placement.layer) {
                if let Some(texture) = state.rasterizer.scroll_chrome_texture(placement.layer) {
                    quads.push(CompositeQuad {
                        layer: placement.layer,
                        transform: placement.transform,
                        opacity: 1.0,
                        clip: placement.clip,
                        texture,
                    });
                }
            }
        }
        self.surface_host
            .present_composite(&mut state.compositor, &quads, clear_color)
            .map_err(js_to_anyhow)?;
        for quad in &quads {
            state.planner.note_composited(quad.layer);
        }

        // GPU 予算超過なら最も長く composite に使われていないレイヤ texture から LRU 退避する
        // （ADR-0127）。Web はデスクトップ既定値をそのまま使う（このスライスでチューニングしない）。
        let budget = GpuBudget::from_viewports(
            self.surface_host.width,
            self.surface_host.height,
            tunables::GPU_BUDGET_VIEWPORTS_DESKTOP,
        );
        for evicted in state.planner.enforce_budget(budget) {
            state.rasterizer.discard(evicted);
        }
        Ok(())
    }

    fn resize(&mut self, width: u32, height: u32, content_scale: f32) {
        self.content_scale = content_scale.max(1.0);
        self.surface_host.resize(width, height);
        if let Some(state) = self.layer_present.as_mut() {
            // root surface と content scale が変わると、Core bounds 由来の device-px texture 寸法も
            // 作り直しになるため台帳ごと invalidate する。
            state.rasterizer.resize(
                self.surface_host.width,
                self.surface_host.height,
                self.content_scale,
            );
            state.compositor.set_content_scale(self.content_scale);
            state.planner.invalidate();
        }
    }
}
