//! `LayerRasterizer` / `LayerCompositor` の wgpu 実装（#633・ADR-0125 backend 半分）。
//!
//! - [`VelloLayerRasterizer`]: レイヤの抽出済み sub-scene を vello でレイヤ texture
//!   （サーフェスサイズ・Rgba8Unorm・透明クリア）へ raster してキャッシュする。
//! - [`WgpuQuadCompositor`]: キャッシュ texture を CompositeQuad（transform / opacity / 軸並行
//!   clip = scissor）として **1 render pass** で合成する。合成に vello は使わない
//!   （ADR-0125 Decision 4）——composite-only フレームでは vello フルパイプラインが一切動かない。
//!
//! パイプライン variant（surface format × blend）は [`WgpuQuadCompositor::warmup`] がエンジン初期化
//! 時に全直積を前倒し生成する（ADR-0130a）。`composite` は生成済み variant を引くだけで、遅延生成の
//! 経路を持たない（未生成はエラー）＝初回合成フレームのパイプラインコンパイルスパイクが構造的に
//! 起きない。
//!
//! quad の頂点は CPU 側でアフィン変換・NDC 変換まで済ませて流し込む（シェーダは通過＋サンプルのみ）。
//! レイヤ texture は premultiplied alpha（vello 出力）なので、blend は (One, OneMinusSrcAlpha)。

use std::collections::HashMap;

use hayate_core::element::id::ElementId;
use hayate_core::SceneGraph;
use hayate_layer_compositor::{
    warmup_variants, BlendMode, CompositeQuad, LayerCompositor, LayerRasterizer, PipelineVariant,
    SurfaceFormat,
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

/// レイヤ 1 枚のキャッシュ面（vello の raster 先 ＝ compositor のサンプル元）。
#[derive(Debug)]
pub struct LayerTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
}

/// vello によるレイヤ rasterizer（`LayerRasterizer` の wgpu 実装）。キャッシュ texture は
/// サーフェスサイズ（絶対座標のまま raster、transform は quad が適用）。
pub struct VelloLayerRasterizer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: VelloSceneRenderer,
    textures: HashMap<ElementId, LayerTexture>,
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
    }

    fn create_texture(&self) -> LayerTexture {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("hayate_layer_cache"),
            size: wgpu::Extent3d {
                width: self.width,
                height: self.height,
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
        LayerTexture { texture, view }
    }
}

impl LayerRasterizer for VelloLayerRasterizer {
    type Texture = LayerTexture;

    fn rasterize(&mut self, layer: ElementId, scene: &SceneGraph) -> Result<(), String> {
        if !self.textures.contains_key(&layer) {
            let texture = self.create_texture();
            self.textures.insert(layer, texture);
        }
        let target_view = &self.textures[&layer].view;
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

    fn texture(&self, layer: ElementId) -> Option<&LayerTexture> {
        self.textures.get(&layer)
    }

    fn texture_bytes_per_layer(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height) * hayate_layer_compositor::tunables::BYTES_PER_PIXEL
    }

    fn discard(&mut self, layer: ElementId) {
        self.textures.remove(&layer);
    }

    fn discard_all(&mut self) {
        self.textures.clear();
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

    fn build_pipeline(&self, variant: PipelineVariant) -> wgpu::RenderPipeline {
        let blend = match variant.blend {
            // premultiplied alpha 合成（vello のレイヤ出力前提）。
            BlendMode::Alpha => Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
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

    /// quad の 6 頂点（2 三角形）を CPU 側でアフィン → NDC 変換して作る。texture は絶対座標
    /// `[0,0,w,h]` を覆う（レイヤは絶対座標のまま raster されている）。
    fn quad_vertices(&self, quad: &CompositeQuad<'_, LayerTexture>, target: &CompositeTarget) -> [QuadVertex; VERTICES_PER_QUAD] {
        let w = target.width as f64;
        let h = target.height as f64;
        let t = quad.transform;
        let corner = |cx: f64, cy: f64, u: f32, v: f32| {
            let dx = t[0] * cx + t[2] * cy + t[4];
            let dy = t[1] * cx + t[3] * cy + t[5];
            QuadVertex {
                pos: [
                    (dx / w * 2.0 - 1.0) as f32,
                    (1.0 - dy / h * 2.0) as f32,
                ],
                uv: [u, v],
                opacity: quad.opacity,
            }
        };
        let tl = corner(0.0, 0.0, 0.0, 0.0);
        let tr = corner(w, 0.0, 1.0, 0.0);
        let bl = corner(0.0, h, 0.0, 1.0);
        let br = corner(w, h, 1.0, 1.0);
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
        let mut vertex_data: Vec<f32> = Vec::with_capacity(quads.len() * VERTICES_PER_QUAD * VERTEX_FLOATS);
        for quad in quads {
            for v in self.quad_vertices(quad, target) {
                vertex_data.extend_from_slice(&v.pos);
                vertex_data.extend_from_slice(&v.uv);
                vertex_data.push(v.opacity);
            }
        }
        let vertex_bytes: Vec<u8> = vertex_data.iter().flat_map(|f| f.to_le_bytes()).collect();
        let vertex_buffer = if vertex_bytes.is_empty() {
            None
        } else {
            let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("hayate_layer_compositor_quads"),
                size: vertex_bytes.len() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: true,
            });
            buffer
                .slice(..)
                .get_mapped_range_mut()
                .copy_from_slice(&vertex_bytes);
            buffer.unmap();
            Some(buffer)
        };

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
            if let Some(buffer) = &vertex_buffer {
                pass.set_vertex_buffer(0, buffer.slice(..));
            }
            for (index, quad) in quads.iter().enumerate() {
                // 軸並行 clip は scissor で適用する（ADR-0125 Decision 4。角丸は内容へ焼き込み済み）。
                let (sx, sy, sw, sh) = match quad.clip {
                    Some([x, y, w, h]) => {
                        let x0 = x.max(0.0).min(target.width as f32) as u32;
                        let y0 = y.max(0.0).min(target.height as f32) as u32;
                        let x1 = (x + w).max(0.0).min(target.width as f32) as u32;
                        let y1 = (y + h).max(0.0).min(target.height as f32) as u32;
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
