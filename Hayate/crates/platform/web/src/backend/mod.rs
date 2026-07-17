use hayate_core::Surface;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

pub(crate) use hayate_app_host::render_host::SceneRenderer as CanvasBackend;
use hayate_app_host::render_host::{
    ClearColor, RenderHost as GenericRenderHost, RendererInit, SceneRenderer,
};
use hayate_app_host::renderer_selection::{
    diagnostic_renderer_selection_policy, web_renderer_selection_policy, RendererCapabilities,
    RendererSelectionReason, SceneRendererKind,
};

#[cfg(feature = "backend-canvaskit")]
mod canvaskit;

#[cfg(feature = "backend-canvaskit")]
use canvaskit::SelectedBackend as CanvasKitBackend;

#[cfg(feature = "backend-vello")]
mod vello;

#[cfg(feature = "backend-vello")]
use vello::SelectedBackend as VelloBackend;

#[cfg(feature = "backend-recording")]
mod recording;

#[cfg(feature = "backend-recording")]
use recording::SelectedBackend as RecordingBackend;

#[cfg(feature = "backend-tiny-skia")]
mod tiny_skia_backend;

#[cfg(feature = "backend-tiny-skia")]
use tiny_skia_backend::SelectedBackend as TinySkiaBackend;

#[cfg(feature = "backend-vello-cpu")]
mod vello_cpu_backend;

#[cfg(feature = "backend-vello-cpu")]
use vello_cpu_backend::SelectedBackend as VelloCpuBackend;

#[cfg(feature = "backend-null")]
mod null;

#[cfg(feature = "backend-null")]
use null::SelectedBackend as NullBackend;

#[cfg(not(any(
    feature = "backend-vello",
    feature = "backend-canvaskit",
    feature = "backend-recording",
    feature = "backend-tiny-skia",
    feature = "backend-vello-cpu",
    feature = "backend-null"
)))]
compile_error!(
    "Enable one of: backend-canvaskit, backend-vello, backend-recording, backend-tiny-skia, backend-vello-cpu, backend-null"
);

/// `hayate_core::Surface`（GPU 経路専用の提示サーフェス契約、ADR-0132 スライス3）の web 実装。
/// `RenderHost`（hoist 済み、`hayate-app-host`）が必要とする最小面（clone・width・height）だけを
/// 持つ薄いラッパー。実際の canvas 資源は `WebRendererInit` がバックエンド初期化時に取り出す。
#[derive(Clone)]
pub(crate) struct WebCanvasSurface(pub(crate) HtmlCanvasElement);

impl Surface for WebCanvasSurface {
    fn width(&self) -> u32 {
        self.0.width()
    }

    fn height(&self) -> u32 {
        self.0.height()
    }
}

/// web adapter による [`RendererInit`] 実装（ADR-0132 スライス3）。`hayate-app-host` の
/// `RenderHost` はこれ越しにしか wasm-bindgen/web-sys 型へ触れない。`classify_init_error` は
/// #672 の決定どおり adapter 個別実装のまま（wgpu 語彙と canvas 2D コンテキスト取得失敗という
/// web 固有のエラー形状が1関数に混在するため共有しない）。
pub(crate) struct WebRendererInit;

impl RendererInit<WebCanvasSurface> for WebRendererInit {
    async fn try_init(
        &self,
        kind: SceneRendererKind,
        surface: WebCanvasSurface,
    ) -> Result<Box<dyn SceneRenderer>, anyhow::Error> {
        let canvas = surface.0;
        match kind {
            SceneRendererKind::CanvasKit => {
                #[cfg(feature = "backend-canvaskit")]
                {
                    return CanvasKitBackend::init(canvas)
                        .map(|backend| Box::new(backend) as Box<dyn SceneRenderer>)
                        .map_err(js_to_anyhow);
                }
                #[cfg(not(feature = "backend-canvaskit"))]
                {
                    let _ = canvas;
                    Err(not_compiled_error(kind))
                }
            }
            SceneRendererKind::Vello => {
                #[cfg(feature = "backend-vello")]
                {
                    return VelloBackend::init(canvas)
                        .await
                        .map(|backend| Box::new(backend) as Box<dyn SceneRenderer>)
                        .map_err(js_to_anyhow);
                }
                #[cfg(not(feature = "backend-vello"))]
                {
                    let _ = canvas;
                    Err(not_compiled_error(kind))
                }
            }
            // skia はネイティブ専用（ADR-0146）。skia-safe は wasm32 非対応で、web の
            // selection policy（PRODUCTION_RENDERERS）にも現れない — 防御的に typed
            // エラーを返すのみ。
            SceneRendererKind::Skia => Err(not_compiled_error(kind)),
            SceneRendererKind::TinySkia => {
                #[cfg(feature = "backend-tiny-skia")]
                {
                    return TinySkiaBackend::init(canvas)
                        .await
                        .map(|backend| Box::new(backend) as Box<dyn SceneRenderer>)
                        .map_err(js_to_anyhow);
                }
                #[cfg(not(feature = "backend-tiny-skia"))]
                {
                    let _ = canvas;
                    Err(not_compiled_error(kind))
                }
            }
            SceneRendererKind::VelloCpu => {
                #[cfg(feature = "backend-vello-cpu")]
                {
                    return VelloCpuBackend::init(canvas)
                        .await
                        .map(|backend| Box::new(backend) as Box<dyn SceneRenderer>)
                        .map_err(js_to_anyhow);
                }
                #[cfg(not(feature = "backend-vello-cpu"))]
                {
                    let _ = canvas;
                    Err(not_compiled_error(kind))
                }
            }
            SceneRendererKind::Recording => {
                #[cfg(feature = "backend-recording")]
                {
                    return RecordingBackend::init(canvas)
                        .await
                        .map(|backend| Box::new(backend) as Box<dyn SceneRenderer>)
                        .map_err(js_to_anyhow);
                }
                #[cfg(not(feature = "backend-recording"))]
                {
                    let _ = canvas;
                    Err(not_compiled_error(kind))
                }
            }
            SceneRendererKind::Null => {
                #[cfg(feature = "backend-null")]
                {
                    return NullBackend::init(canvas)
                        .await
                        .map(|backend| Box::new(backend) as Box<dyn SceneRenderer>)
                        .map_err(js_to_anyhow);
                }
                #[cfg(not(feature = "backend-null"))]
                {
                    let _ = canvas;
                    Err(not_compiled_error(kind))
                }
            }
        }
    }

    /// 一方向のランタイムフォールバック用の同期初期化（ADR-0050）。
    fn try_init_sync_for_fallback(
        &self,
        kind: SceneRendererKind,
        surface: WebCanvasSurface,
    ) -> Result<Box<dyn SceneRenderer>, anyhow::Error> {
        let canvas = surface.0;
        match kind {
            // CanvasKit は選択後の runtime failure が terminal なので、同期 fallback
            // 初期化に到達しない（RenderHost が保証）。防御的に typed error を返す。
            SceneRendererKind::CanvasKit => Err(not_compiled_error(kind)),
            SceneRendererKind::Vello => Err(anyhow::anyhow!(
                "renderer cannot be initialized synchronously for runtime fallback: {}",
                kind.name()
            )),
            // skia はネイティブ専用（ADR-0146）— wasm32 に存在しない。防御的な typed エラー。
            SceneRendererKind::Skia => Err(not_compiled_error(kind)),
            SceneRendererKind::TinySkia => {
                #[cfg(feature = "backend-tiny-skia")]
                {
                    return TinySkiaBackend::init_sync(canvas)
                        .map(|backend| Box::new(backend) as Box<dyn SceneRenderer>)
                        .map_err(js_to_anyhow);
                }
                #[cfg(not(feature = "backend-tiny-skia"))]
                {
                    let _ = canvas;
                    Err(not_compiled_error(kind))
                }
            }
            SceneRendererKind::VelloCpu => {
                #[cfg(feature = "backend-vello-cpu")]
                {
                    return VelloCpuBackend::init_sync(canvas)
                        .map(|backend| Box::new(backend) as Box<dyn SceneRenderer>)
                        .map_err(js_to_anyhow);
                }
                #[cfg(not(feature = "backend-vello-cpu"))]
                {
                    let _ = canvas;
                    Err(not_compiled_error(kind))
                }
            }
            SceneRendererKind::Recording => {
                #[cfg(feature = "backend-recording")]
                {
                    return RecordingBackend::init_sync(canvas)
                        .map(|backend| Box::new(backend) as Box<dyn SceneRenderer>)
                        .map_err(js_to_anyhow);
                }
                #[cfg(not(feature = "backend-recording"))]
                {
                    let _ = canvas;
                    Err(not_compiled_error(kind))
                }
            }
            SceneRendererKind::Null => {
                #[cfg(feature = "backend-null")]
                {
                    return NullBackend::init_sync(canvas)
                        .map(|backend| Box::new(backend) as Box<dyn SceneRenderer>)
                        .map_err(js_to_anyhow);
                }
                #[cfg(not(feature = "backend-null"))]
                {
                    let _ = canvas;
                    Err(not_compiled_error(kind))
                }
            }
        }
    }

    fn classify_init_error(
        &self,
        kind: SceneRendererKind,
        error: &anyhow::Error,
    ) -> RendererSelectionReason {
        let message = error.to_string().to_ascii_lowercase();

        if kind == SceneRendererKind::Vello
            && (message.contains("webgpu")
                || message.contains("navigator.gpu")
                || message.contains("adapter not found"))
        {
            return RendererSelectionReason::WebGpuUnavailable;
        }

        if message.contains("surface lost")
            || message.contains("surface outdated")
            || message.contains("validation error")
        {
            return RendererSelectionReason::SurfaceLost;
        }

        if message.contains("surface not supported")
            || message.contains("context unavailable")
            || message.contains("failed to cast")
        {
            return RendererSelectionReason::CapabilityUnsupported;
        }

        if message.contains("not compiled") {
            return RendererSelectionReason::DisabledByPolicy;
        }

        RendererSelectionReason::RendererInitFailed
    }
}

fn not_compiled_error(kind: SceneRendererKind) -> anyhow::Error {
    anyhow::anyhow!("renderer not compiled: {}", kind.name())
}

/// ポリシーが必要とする実行環境の事実を調べる。WebGPU 系レンダラー（Vello）の採否を
/// 判断できるよう、実際にアダプタを取得できるかまで確認する。
///
/// `navigator.gpu` の有無だけでは不十分。Chrome は GPU が無効な Linux などで
/// `navigator.gpu` を公開しつつ実アダプタを持たないことがあり、その環境で Vello を
/// 試すと wgpu が canvas に `getContext("webgpu")` を発行して canvas を「webgpu 専用」に
/// 固定してしまう。すると後続の tiny-skia フォールバックの `getContext("2d")` が null を
/// 返し、DOM は描けるのに Canvas に何も描画されなくなる。アダプタ取得はここで一度だけ
/// 行う（canvas 非依存なので汚染しない）。取得できた時だけ Vello を試行させる。
///
/// `RendererCapabilities` は `hayate-app-host` に定義された型（ADR-0132 スライス1）なので、
/// 検出ロジックは web 固有の自由関数として持つ（foreign type への inherent impl は禁止）。
async fn detect_renderer_capabilities() -> RendererCapabilities {
    RendererCapabilities {
        webgpu_available: webgpu_adapter_available().await,
    }
}

fn navigator_has_gpu() -> bool {
    let Some(window) = web_sys::window() else {
        return false;
    };
    match js_sys::Reflect::get(window.navigator().as_ref(), &JsValue::from_str("gpu")) {
        Ok(gpu) => !gpu.is_undefined() && !gpu.is_null(),
        Err(_) => false,
    }
}

/// WebGPU アダプタを実際に取得できるか。`navigator.gpu` の存在だけでなく
/// `request_adapter` の成否まで見るので、`navigator.gpu` はあるがアダプタが無い環境で
/// Vello を試行して canvas を汚染するのを防げる。アダプタ要求は canvas に紐づかないため
/// 副作用がない（Vello 本番 init の `create_surface` とは別物）。
#[cfg(feature = "backend-vello")]
async fn webgpu_adapter_available() -> bool {
    if !navigator_has_gpu() {
        return false;
    }
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::BROWSER_WEBGPU,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .is_ok()
}

/// Vello を積まないビルドでは WebGPU レンダラーが存在しないので、常に不可とする。
#[cfg(not(feature = "backend-vello"))]
async fn webgpu_adapter_available() -> bool {
    false
}

/// `Render Host`（hoist 済み、ADR-0132 スライス3）の web adapter 具体化。`Surface`（GPU 経路
/// 専用）は [`WebCanvasSurface`]、バックエンド構築・エラー分類は [`WebRendererInit`]。
pub(crate) type RenderHost = GenericRenderHost<WebCanvasSurface, WebRendererInit>;
pub(crate) type SelectedBackend = RenderHost;

pub(crate) async fn init_render_host(
    canvas: HtmlCanvasElement,
) -> Result<RenderHost, anyhow::Error> {
    init_render_host_with_policy(canvas, web_renderer_selection_policy()).await
}

/// テスト・診断用（ADR-0050）。本番は `init_render_host` を使う。
#[allow(dead_code)]
pub(crate) async fn init_diagnostic_render_host(
    canvas: HtmlCanvasElement,
) -> Result<RenderHost, anyhow::Error> {
    init_render_host_with_policy(canvas, diagnostic_renderer_selection_policy()).await
}

async fn init_render_host_with_policy(
    canvas: HtmlCanvasElement,
    selection_policy: hayate_app_host::renderer_selection::RendererSelectionPolicy,
) -> Result<RenderHost, anyhow::Error> {
    // ポリシーは検出した能力だけから、どのレンダラーをどの順で試すかを純粋に決める。
    // `RenderHost::init_with_policy`（hoist 済み）はその決定を実行するだけ。capability 検出は
    // web 固有なのでここで行い、済んだ値を渡す（core/app-host は検出方法を一切知らない）。
    let capabilities = detect_renderer_capabilities().await;
    RenderHost::init_with_policy(
        WebCanvasSurface(canvas),
        selection_policy,
        capabilities,
        WebRendererInit,
    )
    .await
}

/// wasm-bindgen の `JsValue` エラーを `anyhow::Error` へ変換する。`hayate-app-host` の
/// `SceneRenderer`/`RendererInit` は `anyhow::Error` を使うが、web-sys API 自体は
/// `JsValue` エラーを返すため、境界でここを通す（ADR-0132 スライス3）。
pub(crate) fn js_to_anyhow(error: JsValue) -> anyhow::Error {
    anyhow::anyhow!(js_error_message(&error))
}

/// `anyhow::Error` を wasm-bindgen 公開 API 境界（`Result<(), JsValue>`）へ変換する。
pub(crate) fn anyhow_to_js(error: anyhow::Error) -> JsValue {
    JsValue::from_str(&error.to_string())
}

fn js_error_message(error: &JsValue) -> String {
    error.as_string().unwrap_or_else(|| format!("{error:?}"))
}
