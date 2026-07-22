pub mod layer_compositor;
mod painter;

use std::num::NonZeroUsize;

use hayate_core::{
    render_scene_graph, RenderImage, RenderImageAlphaType, RenderImageFormat, ScenePainter,
    SceneRead,
};
use vello::peniko::color::{AlphaColor, Srgb};
use vello::{
    peniko::{ImageAlphaType, ImageData, ImageFormat},
    AaConfig, AaSupport, RenderParams, Renderer, RendererOptions, Scene,
};
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

/// vello のアンチエイリアス方式（issue #795）。Nothing Phone 3a（Adreno 710）で CSS Gallery
/// ページのパス描画が破綻する切り分けのため、Area / MSAA8 / MSAA16 をランタイム注入可能にする
/// （ADR-0138/0140 の「常時コンパイル＋ランタイムフラグ」流儀。cargo feature や別ビルドは作らない）。
///
/// Area AA はコンピュートシェーダの atomics に最も依存する経路で、「複雑なシーンでだけ破綻」という
/// 症状と整合する。web 経路（`hayate-adapter-web`）は既定 [`DEFAULT_AA_METHOD`]（Area）のまま挙動不変。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VelloAaMethod {
    /// Area AA（既定）。解析的カバレッジ。
    Area,
    /// 8x MSAA。
    Msaa8,
    /// 16x MSAA。
    Msaa16,
}

/// 既定の AA 方式（現行どおり Area）。名前付き定数（マジック値の禁止）。後続の完全人力 issue
/// （実機実験）が実験結果でこの定数を確定させる。
pub const DEFAULT_AA_METHOD: VelloAaMethod = VelloAaMethod::Area;

impl VelloAaMethod {
    /// 実行時上書き文字列（Android の intent extra `adb am start -e` 等）から解釈する。
    /// 未知値は `None`（呼び元は [`DEFAULT_AA_METHOD`] へフォールバックする）。
    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s {
            "area" => Some(Self::Area),
            "msaa8" => Some(Self::Msaa8),
            "msaa16" => Some(Self::Msaa16),
            _ => None,
        }
    }

    /// logcat / 実験記録・上流報告（wgpu/naga）用の安定名。`from_str_opt` と往復する。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Area => "area",
            Self::Msaa8 => "msaa8",
            Self::Msaa16 => "msaa16",
        }
    }

    /// `RenderParams::antialiasing_method` に渡す vello の AA config。
    fn config(self) -> AaConfig {
        match self {
            Self::Area => AaConfig::Area,
            Self::Msaa8 => AaConfig::Msaa8,
            Self::Msaa16 => AaConfig::Msaa16,
        }
    }

    /// `RendererOptions::antialiasing_support`：選んだ方式のパイプラインだけをコンパイルする
    /// （`AaSupport: FromIterator<AaConfig>` で該当フラグのみ立てる）。render 時の `config()` と
    /// 必ず一致させる（有効化していない方式で render すると vello がエラーになる）。
    fn support(self) -> AaSupport {
        std::iter::once(self.config()).collect()
    }
}

pub struct VelloSceneRenderer {
    renderer: Renderer,
    /// このレンダラがコンパイル・使用する AA 方式（#795）。`RendererOptions` の support と
    /// 各 render の config を一致させるため保持する。
    aa: VelloAaMethod,
    /// フレーム間で再利用する Scene（#649）。毎フレーム `Scene::new()` すると内部エンコードバッファを
    /// 作り直して alloc churn（GC 圧）になる。`Scene::reset()` でバッファ容量を保ったまま内容だけ
    /// クリアして再エンコードすることで、毎フレームの新規確保を消す（描画出力は不変）。
    scene: Scene,
}

#[cfg(test)]
mod aa_method_tests {
    use super::{VelloAaMethod, DEFAULT_AA_METHOD};

    #[test]
    fn default_aa_method_is_area_so_the_web_path_is_unchanged() {
        // web 経路（hayate-adapter-web）は VelloSceneRenderer::new を使う＝既定 Area のまま。
        assert_eq!(DEFAULT_AA_METHOD, VelloAaMethod::Area);
    }

    #[test]
    fn aa_method_parses_the_runtime_override_strings() {
        // intent extra（adb am start -e）等の実行時上書き。3 実験（MSAA8/16）を再ビルドなしで回す。
        assert_eq!(
            VelloAaMethod::from_str_opt("area"),
            Some(VelloAaMethod::Area)
        );
        assert_eq!(
            VelloAaMethod::from_str_opt("msaa8"),
            Some(VelloAaMethod::Msaa8)
        );
        assert_eq!(
            VelloAaMethod::from_str_opt("msaa16"),
            Some(VelloAaMethod::Msaa16)
        );
        // 未知値は None（呼び元は既定へフォールバック）。
        assert_eq!(VelloAaMethod::from_str_opt("msaa32"), None);
    }

    #[test]
    fn aa_method_names_round_trip_for_logcat_and_experiment_records() {
        for m in [
            VelloAaMethod::Area,
            VelloAaMethod::Msaa8,
            VelloAaMethod::Msaa16,
        ] {
            assert_eq!(VelloAaMethod::from_str_opt(m.as_str()), Some(m));
        }
    }
}

#[cfg(test)]
mod fingerprint_tests {
    #[test]
    fn shader_set_fingerprint_is_deterministic_and_nonzero() {
        // 永続キャッシュキー（ADR-0130b）に載るので、呼ぶたび同値・自明値でないことを固定。
        let a = super::shader_set_fingerprint();
        assert_eq!(a, super::shader_set_fingerprint());
        assert_ne!(a, 0);
    }
}

/// vello のシェーダ集合（`vello_shaders::SHADERS` の全 WGSL ソース）の決定的指紋。
/// 永続パイプラインキャッシュ（ADR-0130b・issue #777）の `shader_hash` キーに使い、vello
/// 更新でシェーダが変わったとき古いキャッシュファイルを丸ごと無効化する。exhaustive
/// destructuring なので、vello 更新でシェーダの増減があればここがコンパイルエラーになり、
/// 指紋の取りこぼしを型で防ぐ。
pub fn shader_set_fingerprint() -> u64 {
    let vello_shaders::Shaders {
        backdrop,
        backdrop_dyn,
        bbox_clear,
        binning,
        clip_leaf,
        clip_reduce,
        coarse,
        draw_leaf,
        draw_reduce,
        fine_area,
        fine_msaa16,
        fine_msaa8,
        flatten,
        path_count,
        path_count_setup,
        path_tiling,
        path_tiling_setup,
        pathtag_reduce,
        pathtag_reduce2,
        pathtag_scan1,
        pathtag_scan_large,
        pathtag_scan_small,
        tile_alloc,
    } = vello_shaders::SHADERS;
    let shaders = [
        backdrop,
        backdrop_dyn,
        bbox_clear,
        binning,
        clip_leaf,
        clip_reduce,
        coarse,
        draw_leaf,
        draw_reduce,
        fine_area,
        fine_msaa16,
        fine_msaa8,
        flatten,
        path_count,
        path_count_setup,
        path_tiling,
        path_tiling_setup,
        pathtag_reduce,
        pathtag_reduce2,
        pathtag_scan1,
        pathtag_scan_large,
        pathtag_scan_small,
        tile_alloc,
    ];
    hayate_layer_compositor::pipeline_cache::fnv1a_hash(
        shaders
            .iter()
            .flat_map(|s| [s.name.as_bytes(), s.wgsl.code.as_bytes()]),
    )
}

impl VelloSceneRenderer {
    pub fn new(device: &wgpu::Device) -> Result<Self, String> {
        Self::new_with_pipeline_cache(device, None)
    }

    /// 永続パイプラインキャッシュ（ADR-0130b・issue #777）を差して初期化する。`cache` は
    /// `wgpu::Features::PIPELINE_CACHE` が使える環境（現状 Vulkan のみ）で呼び出し側が
    /// `create_pipeline_cache` したもの。`None` なら従来どおり（web/wasm・非対応 backend）。
    /// キャッシュの読み書き・永続化は呼び出し側（Platform Front）の責務で、本 crate は
    /// vello へ注入するだけ。AA 方式は既定（[`DEFAULT_AA_METHOD`]＝Area）。
    pub fn new_with_pipeline_cache(
        device: &wgpu::Device,
        cache: Option<&wgpu::PipelineCache>,
    ) -> Result<Self, String> {
        Self::new_with_options(device, cache, DEFAULT_AA_METHOD)
    }

    /// パイプラインキャッシュに加えて AA 方式を注入して初期化する（#795）。`aa` で選んだ方式の
    /// パイプラインだけをコンパイルし（`support()`）、warmup / render も同じ config で回す。
    /// web/desktop/iOS は `new` / `new_with_pipeline_cache` 経由で既定 Area のまま。Android
    /// アダプタだけが実験用に intent extra 由来の `aa` を渡す。
    pub fn new_with_options(
        device: &wgpu::Device,
        cache: Option<&wgpu::PipelineCache>,
        aa: VelloAaMethod,
    ) -> Result<Self, String> {
        let renderer = Renderer::new(
            device,
            RendererOptions {
                use_cpu: false,
                antialiasing_support: aa.support(),
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
                pipeline_cache: cache.cloned(),
            },
        )
        .map_err(|e| format!("Vello init failed: {e}"))?;
        Ok(Self {
            renderer,
            aa,
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
                    // 選択中の AA 方式のパイプラインを warmup する（support と一致必須、#795）。
                    antialiasing_method: self.aa.config(),
                },
            )
            .map_err(|e| format!("vello warmup render failed: {e}"))
    }

    pub fn render_scene(
        &mut self,
        graph: &(impl SceneRead + ?Sized),
        target: &VelloRenderTarget<'_>,
        clear_color: [f32; 4],
        content_scale: f32,
    ) -> Result<(), String> {
        self.render_scene_with_offset(graph, target, clear_color, content_scale, 0.0, 0.0)
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
        graph: &(impl SceneRead + ?Sized),
        target: &VelloRenderTarget<'_>,
        clear_color: [f32; 4],
        content_scale: f32,
        origin_y: f32,
    ) -> Result<(), String> {
        // scroll band は content を上へ（-y）ずらして band の上端をテクスチャ行 0 に合わせる。
        self.render_scene_with_offset(graph, target, clear_color, content_scale, 0.0, -origin_y)
    }

    /// Like [`Self::render_scene`] but translates the encoded content by `(offset_x, offset_y)`
    /// logical px (applied *before* `content_scale`). Positive values push content right/down.
    ///
    /// Android の edge-to-edge / b2（issue #794・ADR-0144）で使う：GPU ターゲットはフルウィンドウ
    /// のまま、シーンを安全領域インセット分だけ右下へ平行移動して systemBars/カットアウトの裏を
    /// 空ける。バー裏の空き領域は vello が `base_color`（= ルート背景色 `clear_color`）でターゲット
    /// 全面をクリアするので、そのまま塗られる。`render_scene_at` の scroll band shift（0, -origin_y）
    /// もこの一般化経路を通る。
    pub fn render_scene_with_offset(
        &mut self,
        graph: &(impl SceneRead + ?Sized),
        target: &VelloRenderTarget<'_>,
        clear_color: [f32; 4],
        content_scale: f32,
        offset_x: f32,
        offset_y: f32,
    ) -> Result<(), String> {
        // #649: 毎フレーム `Scene::new()` せず、常駐 Scene を reset して再エンコードする（alloc churn 削減）。
        encode_frame(&mut self.scene, graph, content_scale, offset_x, offset_y);
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
                    // 注入された AA 方式（#795）。既定は Area（web/desktop/iOS 不変）。
                    antialiasing_method: self.aa.config(),
                },
            )
            .map_err(|e| format!("render_to_texture: {e}"))
    }
}

/// `graph` を `scene` へエンコードする（#649）。冒頭で `Scene::reset()` して前フレームの内容を消して
/// から再エンコードするので、常駐 Scene をフレーム間で再利用しても内容は毎フレーム新規 Scene と同値。
/// `content_scale != 1.0` のときは content scale の transform を被せる（ADR-0129 のバッファ縮小と同経路）。
/// `(offset_x, offset_y) != (0,0)` のときは、その **内側**（content_scale 適用前の論理座標）で追加の
/// `translate(offset_x, offset_y)` を被せる（ADR-0127 の scroll band 平行移動と ADR-0144 の
/// 安全領域シフトが共有する経路 — ネストした push_transform は outer∘inner で合成されるため、
/// scale は shift 後の量にも一様に掛かる）。
fn encode_frame(
    scene: &mut Scene,
    graph: &(impl SceneRead + ?Sized),
    content_scale: f32,
    offset_x: f32,
    offset_y: f32,
) {
    scene.reset();
    let mut painter = VelloPainter::new(scene);
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
    let shifted = offset_x != 0.0 || offset_y != 0.0;
    if shifted {
        painter.push_transform([1.0, 0.0, 0.0, 1.0, offset_x as f64, offset_y as f64]);
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
pub fn debug_encode_scene(graph: &(impl SceneRead + ?Sized), content_scale: f32) -> Scene {
    let mut scene = Scene::new();
    encode_frame(&mut scene, graph, content_scale, 0.0, 0.0);
    scene
}

// フレーム間 Scene 再利用（#649）の GPU なし検証 seam：呼び出し側が持つ Scene を reset して再エンコード
// する。render_scene と同じ経路（`encode_frame`）を通すので、reset 再利用の内容が新規 Scene と同値かつ
// 前フレームを持ち越さないことを GPU 抜きで固定できる。
#[doc(hidden)]
pub fn debug_encode_frame(
    scene: &mut Scene,
    graph: &(impl SceneRead + ?Sized),
    content_scale: f32,
) {
    encode_frame(scene, graph, content_scale, 0.0, 0.0);
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

pub fn create_blitter(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
) -> TextureBlitter {
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
