use hayate_core::SceneGraph;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use crate::renderer_selection::{
    diagnostic_renderer_selection_policy, is_runtime_fallback_reason,
    standard_renderer_selection_policy, RendererCapabilities, RendererSelectionPlan,
    RendererSelectionPolicy, RendererSelectionReason, SceneRendererKind,
};

pub(crate) type ClearColor = [f32; 4];

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

#[cfg(feature = "backend-null")]
mod null;

#[cfg(feature = "backend-null")]
use null::SelectedBackend as NullBackend;

#[cfg(not(any(
    feature = "backend-vello",
    feature = "backend-recording",
    feature = "backend-tiny-skia",
    feature = "backend-null"
)))]
compile_error!("Enable one of: backend-vello, backend-recording, backend-tiny-skia, backend-null");

pub(crate) trait SceneRenderer {
    fn kind(&self) -> SceneRendererKind;
    fn render_scene(&mut self, scene: &SceneGraph, clear_color: ClearColor) -> Result<(), JsValue>;
    #[allow(dead_code)]
    fn clear(&mut self, clear_color: ClearColor) -> Result<(), JsValue>;

    /// 描画サーフェスを canvas の新しいピクセル寸法に合わせてリサイズする。
    /// `content_scale` は CSS レイアウト座標を物理ピクセルに変換する（dpr）。
    /// オフスクリーン対象（GPU テクスチャ / CPU ピクスマップ）に描画する
    /// バックエンドはここで再確保しないと、canvas が広がっても内容が初期
    /// サイズにクリップされたままになる。サイズを持たないバックエンドは no-op。
    fn resize(&mut self, _width: u32, _height: u32, _content_scale: f32) {}
}

impl RendererCapabilities {
    /// ポリシーが必要とする実行環境の事実を調べる。GPU を初期化せず、
    /// `navigator.gpu` の有無だけを確認する（アダプタは要求しない）ので、
    /// ポリシーは WebGPU 系レンダラーの採否を初期化なしで判断できる。
    fn detect() -> Self {
        Self {
            webgpu_available: navigator_has_gpu(),
        }
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

impl SceneRendererKind {
    pub(crate) async fn try_init(
        self,
        canvas: HtmlCanvasElement,
    ) -> Result<Box<dyn SceneRenderer>, JsValue> {
        match self {
            Self::Vello => {
                #[cfg(feature = "backend-vello")]
                {
                    return Ok(Box::new(VelloBackend::init(canvas).await?));
                }
                #[cfg(not(feature = "backend-vello"))]
                {
                    let _ = canvas;
                    Err(not_compiled_error(self))
                }
            }
            Self::TinySkia => {
                #[cfg(feature = "backend-tiny-skia")]
                {
                    return Ok(Box::new(TinySkiaBackend::init(canvas).await?));
                }
                #[cfg(not(feature = "backend-tiny-skia"))]
                {
                    let _ = canvas;
                    Err(not_compiled_error(self))
                }
            }
            Self::Recording => {
                #[cfg(feature = "backend-recording")]
                {
                    return Ok(Box::new(RecordingBackend::init(canvas).await?));
                }
                #[cfg(not(feature = "backend-recording"))]
                {
                    let _ = canvas;
                    Err(not_compiled_error(self))
                }
            }
            Self::Null => {
                #[cfg(feature = "backend-null")]
                {
                    return Ok(Box::new(NullBackend::init(canvas).await?));
                }
                #[cfg(not(feature = "backend-null"))]
                {
                    let _ = canvas;
                    Err(not_compiled_error(self))
                }
            }
        }
    }

    /// 一方向のランタイムフォールバック用の同期初期化（ADR-0050）。
    pub(crate) fn try_init_sync_for_fallback(
        self,
        canvas: HtmlCanvasElement,
    ) -> Result<Box<dyn SceneRenderer>, JsValue> {
        match self {
            Self::Vello => Err(JsValue::from_str(&format!(
                "renderer cannot be initialized synchronously for runtime fallback: {}",
                self.name()
            ))),
            Self::TinySkia => {
                #[cfg(feature = "backend-tiny-skia")]
                {
                    return Ok(Box::new(TinySkiaBackend::init_sync(canvas)?));
                }
                #[cfg(not(feature = "backend-tiny-skia"))]
                {
                    let _ = canvas;
                    Err(not_compiled_error(self))
                }
            }
            Self::Recording => {
                #[cfg(feature = "backend-recording")]
                {
                    return Ok(Box::new(RecordingBackend::init_sync(canvas)?));
                }
                #[cfg(not(feature = "backend-recording"))]
                {
                    let _ = canvas;
                    Err(not_compiled_error(self))
                }
            }
            Self::Null => {
                #[cfg(feature = "backend-null")]
                {
                    return Ok(Box::new(NullBackend::init_sync(canvas)?));
                }
                #[cfg(not(feature = "backend-null"))]
                {
                    let _ = canvas;
                    Err(not_compiled_error(self))
                }
            }
        }
    }

    pub(crate) fn classify_init_error(self, error: &JsValue) -> RendererSelectionReason {
        let message = js_error_message(error).to_ascii_lowercase();

        if self == Self::Vello
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

fn not_compiled_error(kind: SceneRendererKind) -> JsValue {
    JsValue::from_str(&format!("renderer not compiled: {}", kind.name()))
}

pub(crate) struct RenderHost {
    canvas: HtmlCanvasElement,
    renderer: Option<Box<dyn SceneRenderer>>,
    /// このホストが実行するポリシー決定。どのレンダラーを試すか、なぜ他が
    /// 棄却されたか。ホストは実行するだけで再導出はしない。
    selection_plan: RendererSelectionPlan,
}

impl RenderHost {
    pub(crate) async fn init(canvas: HtmlCanvasElement) -> Result<Self, JsValue> {
        Self::init_with_policy(canvas, standard_renderer_selection_policy()).await
    }

    /// テスト・診断用（ADR-0050）。本番は `init` を使う。
    #[allow(dead_code)]
    pub(crate) async fn init_diagnostic(canvas: HtmlCanvasElement) -> Result<Self, JsValue> {
        Self::init_with_policy(canvas, diagnostic_renderer_selection_policy()).await
    }

    pub(crate) async fn init_with_policy(
        canvas: HtmlCanvasElement,
        selection_policy: RendererSelectionPolicy,
    ) -> Result<Self, JsValue> {
        // ポリシーは検出した能力だけから、どのレンダラーをどの順で試すかを
        // 純粋に決める。`init` はその決定を実行するだけ（計画順に各レンダラー
        // を試し、失敗を表面化する）。
        let plan = selection_policy.choose(RendererCapabilities::detect());

        let mut attempts: Vec<String> = plan
            .rejected()
            .iter()
            .map(|rejection| format!("{}: {:?}", rejection.renderer.name(), rejection.reason))
            .collect();

        for &renderer_kind in plan.attempt_order() {
            match renderer_kind.try_init(canvas.clone()).await {
                Ok(renderer) => {
                    log::info!("selected scene renderer: {}", renderer.kind().name());
                    return Ok(Self {
                        canvas,
                        renderer: Some(renderer),
                        selection_plan: plan,
                    });
                }
                Err(error) => {
                    let reason = renderer_kind.classify_init_error(&error);
                    log::warn!(
                        "scene renderer init failed: {} ({reason:?})",
                        renderer_kind.name()
                    );
                    attempts.push(format!(
                        "{}: {reason:?} ({})",
                        renderer_kind.name(),
                        js_error_message(&error)
                    ));
                }
            }
        }

        Err(JsValue::from_str(&format!(
            "no scene renderer could be selected; attempts: {}",
            attempts.join(", ")
        )))
    }

    fn fallback_after_runtime_failure(
        &mut self,
        error: JsValue,
        retry: impl FnOnce(&mut dyn SceneRenderer) -> Result<(), JsValue>,
    ) -> Result<(), JsValue> {
        let Some(failed_kind) = self.renderer.as_ref().map(|renderer| renderer.kind()) else {
            return Err(error);
        };
        let reason = failed_kind.classify_init_error(&error);
        if !is_runtime_fallback_reason(reason) {
            return Err(error);
        }

        // ポリシー決定に従う。次のレンダラーは計画が失敗したものの後ろに
        // すでに置いたもの。再選択もしないし、ポリシーが見送ったレンダラーの
        // init を再実行もしない。
        let Some(next_kind) = self.selection_plan.next_after(failed_kind) else {
            return Err(error);
        };

        log::warn!(
            "scene renderer runtime failure: {} ({reason:?}); one-way fallback to {}",
            failed_kind.name(),
            next_kind.name()
        );

        let failed_renderer = self
            .renderer
            .take()
            .expect("runtime fallback requires an active scene renderer");
        drop(failed_renderer);

        match next_kind.try_init_sync_for_fallback(self.canvas.clone()) {
            Ok(mut renderer) => {
                debug_assert!(self.selection_plan.includes(renderer.kind()));
                renderer.resize(self.canvas.width(), self.canvas.height(), 1.0);
                let retry_result = retry(renderer.as_mut());
                self.renderer = Some(renderer);
                retry_result
            }
            Err(fallback_error) => Err(JsValue::from_str(&format!(
                "{} failed with {reason:?} ({}); fallback to {} also failed ({})",
                failed_kind.name(),
                js_error_message(&error),
                next_kind.name(),
                js_error_message(&fallback_error)
            ))),
        }
    }
}

impl SceneRenderer for RenderHost {
    fn kind(&self) -> SceneRendererKind {
        let renderer = self
            .renderer
            .as_ref()
            .expect("RenderHost has no active scene renderer");
        debug_assert!(self.selection_plan.includes(renderer.kind()));
        renderer.kind()
    }

    fn render_scene(&mut self, scene: &SceneGraph, clear_color: ClearColor) -> Result<(), JsValue> {
        let Some(renderer) = self.renderer.as_mut() else {
            return Err(JsValue::from_str("RenderHost has no active scene renderer"));
        };
        debug_assert!(self.selection_plan.includes(renderer.kind()));
        match renderer.render_scene(scene, clear_color) {
            Ok(()) => Ok(()),
            Err(error) => self.fallback_after_runtime_failure(error, |renderer| {
                renderer.render_scene(scene, clear_color)
            }),
        }
    }

    fn clear(&mut self, clear_color: ClearColor) -> Result<(), JsValue> {
        let Some(renderer) = self.renderer.as_mut() else {
            return Err(JsValue::from_str("RenderHost has no active scene renderer"));
        };
        debug_assert!(self.selection_plan.includes(renderer.kind()));
        match renderer.clear(clear_color) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.fallback_after_runtime_failure(error, |renderer| renderer.clear(clear_color))
            }
        }
    }

    fn resize(&mut self, width: u32, height: u32, content_scale: f32) {
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.resize(width, height, content_scale);
        }
    }
}

fn js_error_message(error: &JsValue) -> String {
    error.as_string().unwrap_or_else(|| format!("{error:?}"))
}

pub(crate) use RenderHost as SelectedBackend;
pub(crate) use SceneRenderer as CanvasBackend;
