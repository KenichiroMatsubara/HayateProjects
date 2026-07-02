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

/// warmup ダミーターゲットの一辺（px、#644）。主要パイプライン variant を確実にコンパイルさせる
/// 最小サイズ。vello の fine ラスタは 16px タイル単位なので、1 タイルを完全に覆う 16px を使い、
/// 1px の退化ケースで一部ステージの dispatch が飛ぶ可能性を避ける。
const WARMUP_TARGET_SIZE: u32 = 16;

/// warmup シーンの塗りつぶし色（不透明白、#644）。ターゲットを覆う矩形 1 つで、初回タップ/スク
/// ロールのフレームが叩く基本 fill/path/coarse/fine パイプラインを実コンパイルさせる。色自体に
/// 意味はない（オフスクリーンで捨てる）。
const WARMUP_FILL_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];

/// warmup のクリア色（不透明黒、#644）。オフスクリーンで捨てるので値に意味はない。
const WARMUP_CLEAR_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

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

    /// init 直後に極小のダミーシーンを 1 回 render し、vello の主要パイプライン variant を実
    /// コンパイルさせる（#644）。ブラウザ（Dawn）はパイプラインを内部で非同期コンパイルするため、
    /// warmup が無いと初回 dispatch（初回タップ/スクロールのフレーム）にコンパイル遅延が乗る
    /// （診断 要因 5、PRD #607 User Story 4）。ADR-0130a の compositor warmup（#633）と対をなす
    /// vello 本体側の warmup。
    ///
    /// 呼び出し側は init 直後・最初の実アプリフレーム前に 1 回だけ呼ぶ。塗りつぶし矩形 1 つの
    /// オフスクリーン render で fill/path/coarse/fine の標準パイプラインを通す。失敗（アダプタ
    /// 不調等）は `Err` で返すが、呼び出し側は boot を落とさず警告のみで続行すること（初回
    /// フレームで従来どおりコンパイル遅延が出るだけで、描画自体は壊れない）。
    pub fn warmup(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> Result<(), String> {
        use vello::kurbo::{Affine, Rect};
        use vello::peniko::Fill;

        let view = create_target_view(device, WARMUP_TARGET_SIZE, WARMUP_TARGET_SIZE);
        let mut scene = Scene::new();
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            AlphaColor::<Srgb>::new(WARMUP_FILL_COLOR),
            None,
            &Rect::new(
                0.0,
                0.0,
                WARMUP_TARGET_SIZE as f64,
                WARMUP_TARGET_SIZE as f64,
            ),
        );
        self.renderer
            .render_to_texture(
                device,
                queue,
                &scene,
                &view,
                &RenderParams {
                    base_color: AlphaColor::<Srgb>::new(WARMUP_CLEAR_COLOR),
                    width: WARMUP_TARGET_SIZE,
                    height: WARMUP_TARGET_SIZE,
                    antialiasing_method: AaConfig::Area,
                },
            )
            .map_err(|e| format!("vello warmup render failed: {e}"))
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
