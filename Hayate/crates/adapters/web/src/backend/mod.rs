use hayate_core::SceneGraph;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

pub(crate) type ClearColor = [f32; 4];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SceneRendererKind {
    Vello,
    TinySkia,
    /// Non-production renderer (ADR-0050); used via `init_diagnostic`.
    #[allow(dead_code)]
    Recording,
    /// Non-production renderer (ADR-0050); used via `init_diagnostic`.
    #[allow(dead_code)]
    Null,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RendererSelectionReason {
    WebGpuUnavailable,
    RendererInitFailed,
    SurfaceLost,
    CapabilityUnsupported,
    DisabledByPolicy,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RendererSelectionPolicy {
    allowed_renderers: &'static [SceneRendererKind],
    preferred_renderer_order: &'static [SceneRendererKind],
}

impl RendererSelectionPolicy {
    pub(crate) const fn new(
        allowed_renderers: &'static [SceneRendererKind],
        preferred_renderer_order: &'static [SceneRendererKind],
    ) -> Self {
        Self {
            allowed_renderers,
            preferred_renderer_order,
        }
    }

    pub(crate) fn allows(self, renderer: SceneRendererKind) -> bool {
        self.allowed_renderers.contains(&renderer)
    }

    pub(crate) fn preferred_renderer_order(self) -> &'static [SceneRendererKind] {
        self.preferred_renderer_order
    }
}

const PRODUCTION_RENDERERS: &[SceneRendererKind] =
    &[SceneRendererKind::Vello, SceneRendererKind::TinySkia];
/// Reserved for diagnostic init (ADR-0050); not used in production `init`.
#[allow(dead_code)]
const DIAGNOSTIC_RENDERERS: &[SceneRendererKind] =
    &[SceneRendererKind::Recording, SceneRendererKind::Null];

pub(crate) fn standard_renderer_selection_policy() -> RendererSelectionPolicy {
    RendererSelectionPolicy::new(PRODUCTION_RENDERERS, PRODUCTION_RENDERERS)
}

/// Reserved for diagnostic init (ADR-0050); not used in production `init`.
#[allow(dead_code)]
pub(crate) fn diagnostic_renderer_selection_policy() -> RendererSelectionPolicy {
    RendererSelectionPolicy::new(DIAGNOSTIC_RENDERERS, DIAGNOSTIC_RENDERERS)
}

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
    fn clear(&mut self, clear_color: ClearColor) -> Result<(), JsValue>;

    /// Resize the render surface to match the canvas's new pixel dimensions.
    /// Backends that draw to an off-screen target (GPU texture / CPU pixmap)
    /// must reallocate it here, otherwise content stays clipped to the init
    /// size while the canvas grows. Default is a no-op for sizeless backends.
    fn resize(&mut self, _width: u32, _height: u32) {}
}

impl SceneRendererKind {
    /// Stable renderer id for logs and error messages.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Vello => "vello",
            Self::TinySkia => "tiny-skia",
            Self::Recording => "recording",
            Self::Null => "null",
        }
    }

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

    /// Synchronous init for one-way runtime fallback (ADR-0050).
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
    selection_policy: RendererSelectionPolicy,
}

impl RenderHost {
    pub(crate) async fn init(canvas: HtmlCanvasElement) -> Result<Self, JsValue> {
        Self::init_with_policy(canvas, standard_renderer_selection_policy()).await
    }

    /// Reserved for tests and diagnostics (ADR-0050); production uses `init`.
    #[allow(dead_code)]
    pub(crate) async fn init_diagnostic(canvas: HtmlCanvasElement) -> Result<Self, JsValue> {
        Self::init_with_policy(canvas, diagnostic_renderer_selection_policy()).await
    }

    pub(crate) async fn init_with_policy(
        canvas: HtmlCanvasElement,
        selection_policy: RendererSelectionPolicy,
    ) -> Result<Self, JsValue> {
        let mut attempts = Vec::new();

        for &renderer_kind in selection_policy.preferred_renderer_order() {
            if !selection_policy.allows(renderer_kind) {
                attempts.push(format!(
                    "{}: {:?}",
                    renderer_kind.name(),
                    RendererSelectionReason::DisabledByPolicy
                ));
                continue;
            }

            match renderer_kind.try_init(canvas.clone()).await {
                Ok(renderer) => {
                    log::info!(
                        "selected scene renderer: {}",
                        renderer.kind().name()
                    );
                    return Ok(Self {
                        canvas,
                        renderer: Some(renderer),
                        selection_policy,
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

        let Some(next_kind) = self.next_fallback_renderer_after(failed_kind) else {
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
                debug_assert!(self.selection_policy.allows(renderer.kind()));
                renderer.resize(self.canvas.width(), self.canvas.height());
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

    fn next_fallback_renderer_after(
        &self,
        failed_kind: SceneRendererKind,
    ) -> Option<SceneRendererKind> {
        let mut seen_failed = false;
        self.selection_policy
            .preferred_renderer_order()
            .iter()
            .copied()
            .find(|&candidate| {
                if !seen_failed {
                    seen_failed = candidate == failed_kind;
                    return false;
                }
                self.selection_policy.allows(candidate)
            })
    }
}

impl SceneRenderer for RenderHost {
    fn kind(&self) -> SceneRendererKind {
        let renderer = self
            .renderer
            .as_ref()
            .expect("RenderHost has no active scene renderer");
        debug_assert!(self.selection_policy.allows(renderer.kind()));
        renderer.kind()
    }

    fn render_scene(&mut self, scene: &SceneGraph, clear_color: ClearColor) -> Result<(), JsValue> {
        let Some(renderer) = self.renderer.as_mut() else {
            return Err(JsValue::from_str("RenderHost has no active scene renderer"));
        };
        debug_assert!(self.selection_policy.allows(renderer.kind()));
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
        debug_assert!(self.selection_policy.allows(renderer.kind()));
        match renderer.clear(clear_color) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.fallback_after_runtime_failure(error, |renderer| renderer.clear(clear_color))
            }
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.resize(width, height);
        }
    }
}

fn is_runtime_fallback_reason(reason: RendererSelectionReason) -> bool {
    matches!(
        reason,
        RendererSelectionReason::SurfaceLost
            | RendererSelectionReason::CapabilityUnsupported
            | RendererSelectionReason::RendererInitFailed
    )
}

fn js_error_message(error: &JsValue) -> String {
    error.as_string().unwrap_or_else(|| format!("{error:?}"))
}

pub(crate) use RenderHost as SelectedBackend;
pub(crate) use SceneRenderer as CanvasBackend;
