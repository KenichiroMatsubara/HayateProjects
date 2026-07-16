//! `LayerRasterizer` / `LayerCompositor` の wgpu 実装（#633・ADR-0125 backend 半分 / #707 ADR-0127
//! scroll overscan サイジング配線）。
//!
//! - [`VelloLayerRasterizer`]: レイヤの抽出済み sub-scene を vello でレイヤ texture
//!   （既定はサーフェスサイズ・Rgba8Unorm・透明クリア）へ raster してキャッシュする。scroll 内容
//!   レイヤは [`RasterBand`] 付きで呼ばれると、texture を帯サイズに縮めて確保し、内容を
//!   `origin_y` だけ平行移動してから raster する（#707）——vello に scissor/viewport の概念は無く
//!   （`vello::RenderParams` は `{base_color, width, height, antialiasing_method}` のみ）、texture の
//!   寸法そのものが render 範囲になるため、部分帯だけ欲しければ「小さい texture へ内容をずらして
//!   焼く」以外の手段が無い。
//! - [`WgpuQuadCompositor`]: キャッシュ texture を CompositeQuad（transform / opacity / 軸並行
//!   clip = scissor）として **1 render pass** で合成する。合成に vello は使わない
//!   （ADR-0125 Decision 4）——composite-only フレームでは vello フルパイプラインが一切動かない。
//!   quad の「素の」矩形は **texture 自身の寸法**（`quad.texture.width/height`）を使う——帯サイズの
//!   scroll レイヤ texture は full-surface レイヤより小さいため、合成先 `target` の寸法を流用すると
//!   帯を surface 全体に引き伸ばしてしまう（#707 で修正。full-surface レイヤは texture 寸法 ==
//!   target 寸法なので、この修正は既存レイヤの出力を変えない）。
//!
//! パイプライン variant（surface format × blend）は [`WgpuQuadCompositor::warmup`] がエンジン初期化
//! 時に全直積を前倒し生成する（ADR-0130a）。`composite` は生成済み variant を引くだけで、遅延生成の
//! 経路を持たない（未生成はエラー）＝初回合成フレームのパイプラインコンパイルスパイクが構造的に
//! 起きない。
//!
//! quad の頂点は CPU 側でアフィン変換・NDC 変換まで済ませて流し込む（シェーダは通過＋サンプルのみ）。
//! レイヤ texture は straight alpha（vello の `render_to_texture` 出力）なので、blend の色チャネルは
//! (SrcAlpha, OneMinusSrcAlpha)（issue #699 — 以前は premultiplied 前提で (One, ...) にしており、
//! 半透明 box-shadow を持つレイヤが白潰れする不具合があった）。

use std::collections::HashMap;

use hayate_core::element::id::ElementId;
use hayate_core::SceneGraph;
use hayate_layer_compositor::{
    tunables, warmup_variants, BlendMode, CompositeQuad, LayerCompositor, LayerRasterizer,
    PipelineVariant, RasterBand, ScrollLayerExtent, SurfaceFormat,
};

use crate::{VelloRenderTarget, VelloSceneRenderer};

/// レイヤキャッシュ面は透明クリアで raster する（背景は合成パスの clear color が持つ）。
const TRANSPARENT: [f32; 4] = [0.0, 0.0, 0.0, 0.0];

/// wgpu の surface format を warmup 正本の variant へ写す。未知フォーマットは None（呼び元が
/// 明示エラーにする——variant の暗黙追加＝遅延生成をしない）。
pub fn surface_format_variant(format: wgpu::TextureFormat) -> Option<SurfaceFormat> {
    match format {
        wgpu::TextureFormat::Bgra8Unorm => Some(SurfaceFormat::Bgra8Unorm),
        wgpu::TextureFormat::Bgra8UnormSrgb => Some(SurfaceFormat::Bgra8UnormSrgb),
        wgpu::TextureFormat::Rgba8Unorm => Some(SurfaceFormat::Rgba8Unorm),
        wgpu::TextureFormat::Rgba8UnormSrgb => Some(SurfaceFormat::Rgba8UnormSrgb),
        _ => None,
    }
}

fn wgpu_format(format: SurfaceFormat) -> wgpu::TextureFormat {
    match format {
        SurfaceFormat::Bgra8Unorm => wgpu::TextureFormat::Bgra8Unorm,
        SurfaceFormat::Bgra8UnormSrgb => wgpu::TextureFormat::Bgra8UnormSrgb,
        SurfaceFormat::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
        SurfaceFormat::Rgba8UnormSrgb => wgpu::TextureFormat::Rgba8UnormSrgb,
    }
}

/// レイヤ 1 枚のキャッシュ面（vello の raster 先 ＝ compositor のサンプル元）。`width`/`height` は
/// device px の実サイズ（#707 以降、scroll 内容レイヤは帯サイズで full-surface より小さくなり得る
/// ため、compositor 側が `target` の寸法を仮定できるように texture 自身が寸法を持つ）。
#[derive(Debug)]
pub struct LayerTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
}

/// vello によるレイヤ rasterizer（`LayerRasterizer` の wgpu 実装）。キャッシュ texture は
/// サーフェスサイズ（絶対座標のまま raster、transform は quad が適用）。
pub struct VelloLayerRasterizer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: VelloSceneRenderer,
    textures: HashMap<ElementId, LayerTexture>,
    /// Scrollbar など viewport 固定 chrome。content band とは別 texture に保持する。
    scroll_chrome_textures: HashMap<ElementId, LayerTexture>,
    width: u32,
    height: u32,
    /// 論理座標（layout ビューポート単位）を物理バッファへ引き伸ばす倍率（DPI 対応）。
    /// Web の `hayate-adapter-web` と同じ `VelloSceneRenderer::render_scene` 契約を使う
    /// （tiny-skia 側は `LayerCompositor::content_scale` で同型に持つ）。
    content_scale: f32,
}

impl VelloLayerRasterizer {
    pub fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        width: u32,
        height: u32,
        content_scale: f32,
    ) -> Result<Self, String> {
        let renderer = VelloSceneRenderer::new(&device)?;
        Ok(Self {
            device,
            queue,
            renderer,
            textures: HashMap::new(),
            scroll_chrome_textures: HashMap::new(),
            width,
            height,
            content_scale: content_scale.max(1.0),
        })
    }

    /// サーフェスサイズ変更。キャッシュ面は全部作り直しになる（呼び元は planner も invalidate）。
    pub fn resize(&mut self, width: u32, height: u32, content_scale: f32) {
        self.width = width;
        self.height = height;
        self.content_scale = content_scale.max(1.0);
        self.textures.clear();
        self.scroll_chrome_textures.clear();
    }

    /// `width`×`height`（device px）のキャッシュ texture を確保する。既定（非 scroll レイヤ）は
    /// 常に `self.width`×`self.height`（サーフェスサイズ）で呼ぶ；scroll 帯は
    /// [`Self::band_device_height`] で決めた、より小さい高さで呼ぶ（#707）。
    fn create_texture(&self, width: u32, height: u32) -> LayerTexture {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("hayate_layer_cache"),
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
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        LayerTexture {
            texture,
            view,
            width,
            height,
        }
    }

    /// `band`（論理 px の帯高）が device px で占めるキャッシュ texture の高さ（ADR-0127）。
    /// `self.content_scale` を掛けて切り上げる——full-surface レイヤの `self.height` が既に
    /// device px であるのと同じ変換（下側で切ると帯の下端が 1px 欠けて可視域を覆い損ねうる）。
    fn band_device_height(&self, band_height_logical: f32) -> u32 {
        (band_height_logical * self.content_scale).ceil().max(1.0) as u32
    }

    /// `band`（content-local、`ScrollLayerExtent` 語彙）で scroll レイヤを raster したときの
    /// キャッシュ texture バイト数（ADR-0127 の GPU 予算計上・#707）。`texture_bytes_per_layer`
    /// が非 scroll レイヤの一様な full-surface バイトを返すのに対し、こちらは帯の高さだけを
    /// 計上する（全高では確保しないので、予算に対してもそう計上する）。`present_layers` が
    /// `PresentPlanner::note_scroll_rasterized` へ渡す値の単一正本——ここと [`Self::rasterize`]
    /// の [`Self::band_device_height`] 呼び出しが分岐すると、予算計上と実 texture サイズが
    /// ずれてしまう。
    pub fn scroll_band_bytes(&self, band: ScrollLayerExtent) -> u64 {
        u64::from(self.width)
            * u64::from(self.band_device_height(band.height))
            * tunables::BYTES_PER_PIXEL
    }

    /// Scroll content band と viewport 固定 chrome texture を合わせた実キャッシュ量。
    pub fn scroll_cache_bytes(&self, band: ScrollLayerExtent) -> u64 {
        self.scroll_band_bytes(band) + self.texture_bytes_per_layer()
    }

    /// Scrollbar 等の固定 chrome は full-surface texture へ別 raster し、content band 用の
    /// compensating translate を掛けずに合成する。
    pub fn rasterize_scroll_chrome(
        &mut self,
        layer: ElementId,
        scene: &SceneGraph,
    ) -> Result<(), String> {
        let needs_new_texture = self
            .scroll_chrome_textures
            .get(&layer)
            .is_none_or(|existing| existing.width != self.width || existing.height != self.height);
        if needs_new_texture {
            let texture = self.create_texture(self.width, self.height);
            self.scroll_chrome_textures.insert(layer, texture);
        }
        let target_view = &self.scroll_chrome_textures[&layer].view;
        self.renderer.render_scene(
            scene,
            &VelloRenderTarget {
                device: &self.device,
                queue: &self.queue,
                target_view,
                width: self.width,
                height: self.height,
            },
            TRANSPARENT,
            self.content_scale,
        )
    }

    pub fn scroll_chrome_texture(&self, layer: ElementId) -> Option<&LayerTexture> {
        self.scroll_chrome_textures.get(&layer)
    }
}

impl LayerRasterizer for VelloLayerRasterizer {
    type Texture = LayerTexture;

    /// `band` が `Some` なら scroll 内容レイヤの overscan 帯サイジング（ADR-0127・#707）:
    /// texture を `self.width`×帯高（device px）に確保し、`band.origin_y`（絶対シーン座標）が
    /// texture 行 0 に来るよう内容を平行移動して raster する。キャッシュ済み texture の寸法が
    /// 要求と食い違えば（帯が動いた／非 scroll へ戻った等）作り直す。`None` は従来どおり
    /// サーフェスサイズで raster する（既存レイヤの挙動は無変更）。
    fn rasterize(
        &mut self,
        layer: ElementId,
        scene: &SceneGraph,
        band: Option<RasterBand>,
    ) -> Result<(), String> {
        let (texture_width, texture_height, origin_y) = match band {
            Some(band) => (
                self.width,
                self.band_device_height(band.height),
                band.origin_y,
            ),
            None => (self.width, self.height, 0.0),
        };
        let needs_new_texture = self.textures.get(&layer).is_none_or(|existing| {
            existing.width != texture_width || existing.height != texture_height
        });
        if needs_new_texture {
            let texture = self.create_texture(texture_width, texture_height);
            self.textures.insert(layer, texture);
        }
        let target_view = &self.textures[&layer].view;
        self.renderer.render_scene_at(
            scene,
            &VelloRenderTarget {
                device: &self.device,
                queue: &self.queue,
                target_view,
                width: texture_width,
                height: texture_height,
            },
            TRANSPARENT,
            self.content_scale,
            origin_y,
        )
    }

    fn texture(&self, layer: ElementId) -> Option<&LayerTexture> {
        self.textures.get(&layer)
    }

    fn texture_bytes_per_layer(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height) * tunables::BYTES_PER_PIXEL
    }

    fn discard(&mut self, layer: ElementId) {
        self.textures.remove(&layer);
        self.scroll_chrome_textures.remove(&layer);
    }

    fn discard_all(&mut self) {
        self.textures.clear();
        self.scroll_chrome_textures.clear();
    }
}

/// 合成先（surface の 1 フレーム分の view）。
pub struct CompositeTarget {
    pub view: wgpu::TextureView,
    pub width: u32,
    pub height: u32,
    pub format: wgpu::TextureFormat,
    /// 合成パス冒頭の clear color（従来の raster の base color と同じもの）。
    pub clear: [f32; 4],
}

/// 頂点 1 個 = NDC 座標 + UV + opacity（CPU 側で変換済み。シェーダは通過＋サンプルのみ）。
#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct QuadVertex {
    pos: [f32; 2],
    uv: [f32; 2],
    opacity: f32,
}

const VERTEX_FLOATS: usize = 5;
const VERTICES_PER_QUAD: usize = 6;

const QUAD_SHADER: &str = r#"
struct VsIn {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) opacity: f32,
}
struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) opacity: f32,
}

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    out.pos = vec4<f32>(in.pos, 0.0, 1.0);
    out.uv = in.uv;
    out.opacity = in.opacity;
    return out;
}

@group(0) @binding(0) var layer_tex: texture_2d<f32>;
@group(0) @binding(1) var layer_samp: sampler;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // レイヤ texture は premultiplied alpha（vello 出力）なので全成分に opacity を乗算。
    return textureSample(layer_tex, layer_samp, in.uv) * in.opacity;
}
"#;

/// 専用 wgpu quad compositor（`LayerCompositor` の wgpu 実装）。パイプライン variant は
/// `warmup` が init 時に全直積を生成し、`composite` は生成済みを引くだけ（遅延生成なし・ADR-0130a）。
pub struct WgpuQuadCompositor {
    device: wgpu::Device,
    queue: wgpu::Queue,
    shader: wgpu::ShaderModule,
    bind_group_layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    sampler: wgpu::Sampler,
    pipelines: HashMap<PipelineVariant, wgpu::RenderPipeline>,
    /// placement / clip は logical px、texture / target は device px。線形部は texture の
    /// device 解像度で相殺されるため、translation と clip だけへこの倍率を適用する。
    content_scale: f32,
    /// WebGPU では小さい buffer でも `mappedAtCreation` が device-lost 状態で例外になり得る。
    /// 合成 hot path は未 map の COPY_DST buffer を再利用し、queue write だけで更新する。
    vertex_buffer: Option<wgpu::Buffer>,
    vertex_buffer_capacity: u64,
}

impl WgpuQuadCompositor {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("hayate_layer_compositor_quad"),
            source: wgpu::ShaderSource::Wgsl(QUAD_SHADER.into()),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("hayate_layer_compositor_quad"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("hayate_layer_compositor_quad"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("hayate_layer_compositor_quad"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            device,
            queue,
            shader,
            bind_group_layout,
            pipeline_layout,
            sampler,
            pipelines: HashMap::new(),
            content_scale: 1.0,
            vertex_buffer: None,
            vertex_buffer_capacity: 0,
        }
    }

    /// エンジン初期化時に全パイプライン variant（surface format × blend）を前倒し生成する
    /// （ADR-0130a）。以後 `composite` で遅延生成は起きない。
    pub fn warmup(&mut self) {
        for variant in warmup_variants() {
            let pipeline = self.build_pipeline(variant);
            self.pipelines.insert(variant, pipeline);
        }
    }

    /// warmup 済み variant 数（契約テスト用）。
    pub fn warmed_variant_count(&self) -> usize {
        self.pipelines.len()
    }

    pub fn set_content_scale(&mut self, content_scale: f32) {
        self.content_scale = content_scale.max(1.0);
    }

    fn build_pipeline(&self, variant: PipelineVariant) -> wgpu::RenderPipeline {
        let blend = match variant.blend {
            // Layer textures hold straight (non-premultiplied) alpha, not premultiplied alpha
            // as this comment previously assumed — see issue #699. `render_scene`'s Vello
            // output written into an isolated layer texture is straight-alpha, so the color
            // channel must scale by `src.a` here (`SrcAlpha`), not skip that scaling (`One`,
            // which is only correct for already-premultiplied input).
            BlendMode::Alpha => Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
            }),
            BlendMode::Opaque => None,
        };
        self.device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("hayate_layer_compositor_quad"),
                layout: Some(&self.pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &self.shader,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: (VERTEX_FLOATS * std::mem::size_of::<f32>()) as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x2,
                                offset: 0,
                                shader_location: 0,
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x2,
                                offset: 8,
                                shader_location: 1,
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32,
                                offset: 16,
                                shader_location: 2,
                            },
                        ],
                    }],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &self.shader,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu_format(variant.format),
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
    }

    /// quad の 6 頂点（2 三角形）を CPU 側でアフィン → NDC 変換して作る。「素の」矩形は
    /// **texture 自身の寸法**（`quad.texture.width/height`、device px）を使う——レイヤは絶対座標の
    /// まま raster されているので、full-surface レイヤ（従来どおり）は texture 寸法 == `target`
    /// 寸法だが、scroll 帯レイヤ（#707・ADR-0127）は texture が帯サイズに縮んでいるため、素の矩形
    /// も帯サイズでなければ全体を surface へ引き伸ばしてしまう。NDC 正規化（`dx/target_w` 等）は
    /// 引き続き合成先 `target` の寸法を使う（NDC は render target 基準——`quad.transform` の
    /// 「素の矩形→絶対シーン座標」変換とは独立な軸）。
    fn quad_vertices(
        &self,
        quad: &CompositeQuad<'_, LayerTexture>,
        target: &CompositeTarget,
    ) -> [QuadVertex; VERTICES_PER_QUAD] {
        let tex_w = quad.texture.width as f64;
        let tex_h = quad.texture.height as f64;
        let target_w = target.width as f64;
        let target_h = target.height as f64;
        let t = quad.transform;
        let s = self.content_scale as f64;
        let corner = |cx: f64, cy: f64, u: f32, v: f32| {
            let dx = t[0] * cx + t[2] * cy + t[4] * s;
            let dy = t[1] * cx + t[3] * cy + t[5] * s;
            QuadVertex {
                pos: [
                    (dx / target_w * 2.0 - 1.0) as f32,
                    (1.0 - dy / target_h * 2.0) as f32,
                ],
                uv: [u, v],
                opacity: quad.opacity,
            }
        };
        let tl = corner(0.0, 0.0, 0.0, 0.0);
        let tr = corner(tex_w, 0.0, 1.0, 0.0);
        let bl = corner(0.0, tex_h, 0.0, 1.0);
        let br = corner(tex_w, tex_h, 1.0, 1.0);
        [tl, tr, bl, tr, br, bl]
    }
}

impl LayerCompositor for WgpuQuadCompositor {
    type Texture = LayerTexture;
    type Target = CompositeTarget;

    fn composite(
        &mut self,
        target: &mut CompositeTarget,
        quads: &[CompositeQuad<'_, LayerTexture>],
    ) -> Result<(), String> {
        let format = surface_format_variant(target.format)
            .ok_or_else(|| format!("unsupported composite surface format: {:?}", target.format))?;
        // 遅延生成はしない（ADR-0130a）：init の warmup が全 variant を生成済みであることが契約。
        let pipeline = self
            .pipelines
            .get(&PipelineVariant {
                format,
                blend: BlendMode::Alpha,
            })
            .ok_or("compositor pipeline not warmed up (ADR-0130a violation)")?;

        // 全 quad の頂点を 1 本の vertex buffer に詰める（draw は quad ごと＝bind group/scissor 切替）。
        let mut vertex_data: Vec<f32> =
            Vec::with_capacity(quads.len() * VERTICES_PER_QUAD * VERTEX_FLOATS);
        for quad in quads {
            for v in self.quad_vertices(quad, target) {
                vertex_data.extend_from_slice(&v.pos);
                vertex_data.extend_from_slice(&v.uv);
                vertex_data.push(v.opacity);
            }
        }
        let vertex_bytes: Vec<u8> = vertex_data.iter().flat_map(|f| f.to_le_bytes()).collect();
        if !vertex_bytes.is_empty() {
            let required = vertex_bytes.len() as u64;
            if self.vertex_buffer_capacity < required {
                let capacity = required.next_power_of_two();
                self.vertex_buffer = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("hayate_layer_compositor_quads"),
                    size: capacity,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
                self.vertex_buffer_capacity = capacity;
            }
            self.queue.write_buffer(
                self.vertex_buffer
                    .as_ref()
                    .expect("vertex buffer allocated"),
                0,
                &vertex_bytes,
            );
        }

        let bind_groups: Vec<wgpu::BindGroup> = quads
            .iter()
            .map(|quad| {
                self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("hayate_layer_compositor_quad"),
                    layout: &self.bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&quad.texture.view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&self.sampler),
                        },
                    ],
                })
            })
            .collect();

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("hayate_layer_compositor"),
            });
        {
            let [r, g, b, a] = target.clear;
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("hayate_layer_compositor"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: f64::from(r),
                            g: f64::from(g),
                            b: f64::from(b),
                            a: f64::from(a),
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(pipeline);
            if let Some(buffer) = &self.vertex_buffer {
                pass.set_vertex_buffer(0, buffer.slice(..));
            }
            for (index, quad) in quads.iter().enumerate() {
                // 軸並行 clip は scissor で適用する（ADR-0125 Decision 4。角丸は内容へ焼き込み済み）。
                let (sx, sy, sw, sh) = match quad.clip {
                    Some([x, y, w, h]) => {
                        let s = self.content_scale;
                        let x0 = (x * s).max(0.0).min(target.width as f32) as u32;
                        let y0 = (y * s).max(0.0).min(target.height as f32) as u32;
                        let x1 = ((x + w) * s).max(0.0).min(target.width as f32) as u32;
                        let y1 = ((y + h) * s).max(0.0).min(target.height as f32) as u32;
                        (x0, y0, x1.saturating_sub(x0), y1.saturating_sub(y0))
                    }
                    None => (0, 0, target.width, target.height),
                };
                if sw == 0 || sh == 0 {
                    continue; // 完全にクリップ外
                }
                pass.set_scissor_rect(sx, sy, sw, sh);
                pass.set_bind_group(0, &bind_groups[index], &[]);
                let start = (index * VERTICES_PER_QUAD) as u32;
                pass.draw(start..start + VERTICES_PER_QUAD as u32, 0..1);
            }
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        Ok(())
    }
}
