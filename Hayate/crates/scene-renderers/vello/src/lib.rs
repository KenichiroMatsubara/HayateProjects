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
    /// フレーム間で再利用する Scene（#649）。毎フレーム `Scene::new()` すると内部エンコードバッファを
    /// 作り直して alloc churn（GC 圧）になる。`Scene::reset()` でバッファ容量を保ったまま内容だけ
    /// クリアして再エンコードすることで、毎フレームの新規確保を消す（描画出力は不変）。
    scene: Scene,
}

impl VelloSceneRenderer {
    pub fn new(device: &wgpu::Device) -> Result<Self, String> {
        let renderer = Renderer::new(
            device,
            RendererOptions {
                use_cpu: false,
                antialiasing_support: AaSupport::area_only(),
                // シェーダ init のスレッド数は vello 既定に追従する: macOS のみ single thread
                // 推奨（vello の RendererOptions ドキュメント参照）、それ以外は None（並列
                // init ヒューリスティック）。`Some(1)` を全プラットフォームに固定すると native
                // の起動時シェーダコンパイルが直列化し、初回フレームまでの時間を数百 ms 延ばす
                // （wasm では本オプションは無効なので web 経路の挙動は変わらない）。
                num_init_threads: if cfg!(target_os = "macos") {
                    NonZeroUsize::new(1)
                } else {
                    None
                },
                pipeline_cache: None,
            },
        )
        .map_err(|e| format!("Vello init failed: {e}"))?;
        Ok(Self {
            renderer,
            scene: Scene::new(),
        })
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
        self.render_scene_at(graph, target, clear_color, content_scale, 0.0)
    }

    /// Like [`Self::render_scene`] but additionally shifts the encoded content up by `origin_y`
    /// (logical px, applied *before* `content_scale`) — `origin_y == 0.0` is exactly
    /// `render_scene`. Used by [`crate::layer_compositor::VelloLayerRasterizer`] to raster a
    /// scroll-content band whose texture only covers `[origin_y, origin_y + height)` of the
    /// layer's absolute scene content (ADR-0127 overscan sizing): vello has no scissor/viewport
    /// render concept (`RenderParams` is just `{base_color, width, height,
    /// antialiasing_method}`), so the destination texture's own extent *is* the render bounds —
    /// the only way to raster a sub-region is to translate content so that region's top lands at
    /// texture row 0.
    pub fn render_scene_at(
        &mut self,
        graph: &SceneGraph,
        target: &VelloRenderTarget<'_>,
        clear_color: [f32; 4],
        content_scale: f32,
        origin_y: f32,
    ) -> Result<(), String> {
        // #649: 毎フレーム `Scene::new()` せず、常駐 Scene を reset して再エンコードする（alloc churn 削減）。
        encode_frame(&mut self.scene, graph, content_scale, origin_y);
        self.renderer
            .render_to_texture(
                target.device,
                target.queue,
                &self.scene,
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

/// `graph` を `scene` へエンコードする（#649）。冒頭で `Scene::reset()` して前フレームの内容を消して
/// から再エンコードするので、常駐 Scene をフレーム間で再利用しても内容は毎フレーム新規 Scene と同値。
/// `content_scale != 1.0` のときは content scale の transform を被せる（ADR-0129 のバッファ縮小と同経路）。
/// `origin_y != 0.0` のときは、その **内側**（content_scale 適用前の論理座標）で追加の
/// `translate(0, -origin_y)` を被せる（ADR-0127 の scroll band 平行移動 — ネストした
/// push_transform は outer∘inner で合成されるため、scale は shift 後の量にも一様に掛かる）。
fn encode_frame(scene: &mut Scene, graph: &SceneGraph, content_scale: f32, origin_y: f32) {
    scene.reset();
    let mut painter = VelloPainter::new(scene);
    let scaled = content_scale != 1.0;
    if scaled {
        painter.push_transform([content_scale as f64, 0.0, 0.0, content_scale as f64, 0.0, 0.0]);
    }
    let shifted = origin_y != 0.0;
    if shifted {
        painter.push_transform([1.0, 0.0, 0.0, 1.0, 0.0, -origin_y as f64]);
    }
    render_scene_graph(graph, &mut painter);
    if shifted {
        painter.pop_transform();
    }
    if scaled {
        painter.pop_transform();
    }
}

// perf プローブ用 seam（`tests/perf_probe.rs`）：GPU なしで「SceneGraph → vello Scene
// エンコード」だけの所要時間を測るために公開する。公開契約ではない。
#[doc(hidden)]
pub fn debug_encode_scene(graph: &SceneGraph, content_scale: f32) -> Scene {
    let mut scene = Scene::new();
    encode_frame(&mut scene, graph, content_scale, 0.0);
    scene
}

// フレーム間 Scene 再利用（#649）の GPU なし検証 seam：呼び出し側が持つ Scene を reset して再エンコード
// する。render_scene と同じ経路（`encode_frame`）を通すので、reset 再利用の内容が新規 Scene と同値かつ
// 前フレームを持ち越さないことを GPU 抜きで固定できる。
#[doc(hidden)]
pub fn debug_encode_frame(scene: &mut Scene, graph: &SceneGraph, content_scale: f32) {
    encode_frame(scene, graph, content_scale, 0.0);
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
