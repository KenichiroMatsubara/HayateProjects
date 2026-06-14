//! Renderer Selection Policy — the rule for which `Scene Renderer` to prefer
//! and on what grounds to adopt or reject one (Hayate `CONTEXT.md`).
//!
//! This module is the pure seam lifted out of the `Render Host`: it holds no
//! `wasm-bindgen` / `web-sys` types and never touches the GPU. The policy
//! decision is a pure function of [`RendererCapabilities`], so it can be
//! verified with capability inputs alone — no real GPU, no browser. The host
//! gathers capabilities and consumes the resulting [`RendererSelectionPlan`];
//! it does not embed the selection rules itself.

// On native (non-wasm) builds only the test path exercises this module; the
// host code that consumes the rest lives behind `cfg(target_arch = "wasm32")`.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)] // only one backend variant is live per feature set
pub(crate) enum SceneRendererKind {
    Vello,
    TinySkia,
    /// Non-production renderer (ADR-0050); used via `init_diagnostic`.
    Recording,
    /// Non-production renderer (ADR-0050); used via `init_diagnostic`.
    Null,
}

impl SceneRendererKind {
    /// Whether this renderer needs `navigator.gpu` to initialize. Only the
    /// WebGPU-backed Vello renderer does; the others draw on the CPU.
    pub(crate) fn requires_webgpu(self) -> bool {
        matches!(self, Self::Vello)
    }

    /// Stable renderer id for logs and error messages.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Vello => "vello",
            Self::TinySkia => "tiny-skia",
            Self::Recording => "recording",
            Self::Null => "null",
        }
    }
}

/// Why the `Render Host` did not adopt — or switched away from — a renderer.
/// A shared vocabulary so reasons are observable, not ad-hoc error strings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RendererSelectionReason {
    WebGpuUnavailable,
    RendererInitFailed,
    SurfaceLost,
    CapabilityUnsupported,
    DisabledByPolicy,
}

/// Whether a runtime failure carrying this reason should trigger a one-way
/// fallback to the next renderer (ADR-0050). `WebGpuUnavailable` and
/// `DisabledByPolicy` are settled at selection time, never mid-frame.
pub(crate) fn is_runtime_fallback_reason(reason: RendererSelectionReason) -> bool {
    matches!(
        reason,
        RendererSelectionReason::SurfaceLost
            | RendererSelectionReason::CapabilityUnsupported
            | RendererSelectionReason::RendererInitFailed
    )
}

/// Statically-knowable facts about the host environment that the
/// [`RendererSelectionPolicy`] consults. Gathered without initializing any
/// `Scene Renderer` so the policy decision stays GPU-free and testable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RendererCapabilities {
    /// Whether `navigator.gpu` is present. WebGPU-backed renderers (Vello)
    /// cannot run without it; CPU renderers (tiny-skia) are unaffected.
    pub(crate) webgpu_available: bool,
}

/// A `Scene Renderer` passed over by the policy, paired with the
/// `Renderer Selection Reason` that explains why.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RendererRejection {
    pub(crate) renderer: SceneRendererKind,
    pub(crate) reason: RendererSelectionReason,
}

/// The policy's decision for a given set of capabilities: the renderers to
/// attempt (in preference order, capability-feasible ones only) and the
/// renderers rejected up front, each with its reason. Observable so callers
/// can report why a renderer was — or was not — selected.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RendererSelectionPlan {
    attempt_order: Vec<SceneRendererKind>,
    rejected: Vec<RendererRejection>,
}

impl RendererSelectionPlan {
    /// The renderer the host should attempt first, if any is feasible.
    /// Observability accessor; not every backend configuration consumes it.
    #[allow(dead_code)]
    pub(crate) fn primary(&self) -> Option<SceneRendererKind> {
        self.attempt_order.first().copied()
    }

    /// Feasible renderers in the order the host should attempt them.
    pub(crate) fn attempt_order(&self) -> &[SceneRendererKind] {
        &self.attempt_order
    }

    /// Renderers the policy rejected before init, each with its reason.
    pub(crate) fn rejected(&self) -> &[RendererRejection] {
        &self.rejected
    }

    /// Whether `renderer` is part of the planned attempt sequence — the
    /// invariant the host upholds for whichever renderer is currently active.
    pub(crate) fn includes(&self, renderer: SceneRendererKind) -> bool {
        self.attempt_order.contains(&renderer)
    }

    /// The reason the policy gave for passing over `renderer`, if it did.
    /// Observability accessor; not every backend configuration consumes it.
    #[allow(dead_code)]
    pub(crate) fn rejection_reason(
        &self,
        renderer: SceneRendererKind,
    ) -> Option<RendererSelectionReason> {
        self.rejected
            .iter()
            .find(|rejection| rejection.renderer == renderer)
            .map(|rejection| rejection.reason)
    }

    /// The next renderer to attempt after `failed` per this plan, used by the
    /// runtime fallback path so it follows the policy decision instead of
    /// re-deriving selection.
    pub(crate) fn next_after(&self, failed: SceneRendererKind) -> Option<SceneRendererKind> {
        let failed_index = self.attempt_order.iter().position(|&kind| kind == failed)?;
        self.attempt_order.get(failed_index + 1).copied()
    }
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

    /// Decide which renderers to attempt for the given capabilities. Pure: it
    /// consumes only the policy's renderer lists and the capability inputs, so
    /// the selection is fully testable without a real GPU. Renderers that are
    /// disallowed or whose capabilities are unmet are filtered out up front and
    /// recorded in [`RendererSelectionPlan::rejected`] with their reason; the
    /// rest stay in preference order as the attempt sequence.
    pub(crate) fn choose(self, capabilities: RendererCapabilities) -> RendererSelectionPlan {
        let mut attempt_order = Vec::new();
        let mut rejected = Vec::new();

        for &renderer in self.preferred_renderer_order {
            if !self.allows(renderer) {
                rejected.push(RendererRejection {
                    renderer,
                    reason: RendererSelectionReason::DisabledByPolicy,
                });
                continue;
            }

            if renderer.requires_webgpu() && !capabilities.webgpu_available {
                rejected.push(RendererRejection {
                    renderer,
                    reason: RendererSelectionReason::WebGpuUnavailable,
                });
                continue;
            }

            attempt_order.push(renderer);
        }

        RendererSelectionPlan {
            attempt_order,
            rejected,
        }
    }
}

#[cfg(any(feature = "backend-vello", feature = "backend-tiny-skia"))]
const PRODUCTION_RENDERERS: &[SceneRendererKind] =
    &[SceneRendererKind::Vello, SceneRendererKind::TinySkia];

/// C3 codec integration tests build with `--features backend-null` only.
#[cfg(all(
    feature = "backend-null",
    not(any(feature = "backend-vello", feature = "backend-tiny-skia"))
))]
const PRODUCTION_RENDERERS: &[SceneRendererKind] = &[SceneRendererKind::Null];

/// Reserved for diagnostic init (ADR-0050); not used in production `init`.
const DIAGNOSTIC_RENDERERS: &[SceneRendererKind] =
    &[SceneRendererKind::Recording, SceneRendererKind::Null];

#[cfg(any(
    feature = "backend-vello",
    feature = "backend-tiny-skia",
    feature = "backend-null"
))]
pub(crate) fn standard_renderer_selection_policy() -> RendererSelectionPolicy {
    RendererSelectionPolicy::new(PRODUCTION_RENDERERS, PRODUCTION_RENDERERS)
}

/// Reserved for diagnostic init (ADR-0050); not used in production `init`.
pub(crate) fn diagnostic_renderer_selection_policy() -> RendererSelectionPolicy {
    RendererSelectionPolicy::new(DIAGNOSTIC_RENDERERS, DIAGNOSTIC_RENDERERS)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PREFERRED: &[SceneRendererKind] =
        &[SceneRendererKind::Vello, SceneRendererKind::TinySkia];

    fn policy() -> RendererSelectionPolicy {
        RendererSelectionPolicy::new(PREFERRED, PREFERRED)
    }

    #[test]
    fn prefers_vello_when_webgpu_available() {
        let plan = policy().choose(RendererCapabilities {
            webgpu_available: true,
        });
        assert_eq!(plan.primary(), Some(SceneRendererKind::Vello));
        assert!(plan.rejected().is_empty());
    }

    #[test]
    fn falls_to_tiny_skia_when_webgpu_unavailable() {
        let plan = policy().choose(RendererCapabilities {
            webgpu_available: false,
        });
        assert_eq!(plan.primary(), Some(SceneRendererKind::TinySkia));
        assert_eq!(
            plan.rejection_reason(SceneRendererKind::Vello),
            Some(RendererSelectionReason::WebGpuUnavailable),
        );
    }

    #[test]
    fn cpu_renderer_is_unaffected_by_missing_webgpu() {
        // tiny-skia draws on the CPU, so losing WebGPU must not reject it.
        assert_eq!(
            plan_for(&[SceneRendererKind::TinySkia], false).primary(),
            Some(SceneRendererKind::TinySkia),
        );
    }

    #[test]
    fn renderer_outside_allow_list_is_disabled_by_policy() {
        let policy = RendererSelectionPolicy::new(&[SceneRendererKind::TinySkia], PREFERRED);
        let plan = policy.choose(RendererCapabilities {
            webgpu_available: true,
        });
        assert_eq!(plan.primary(), Some(SceneRendererKind::TinySkia));
        assert_eq!(
            plan.rejection_reason(SceneRendererKind::Vello),
            Some(RendererSelectionReason::DisabledByPolicy),
        );
    }

    #[test]
    fn no_feasible_renderer_yields_empty_attempt_order() {
        // Only Vello is allowed, but WebGPU is absent: nothing is attemptable,
        // and the reason is observable rather than a bare failure.
        let plan = plan_for(&[SceneRendererKind::Vello], false);
        assert_eq!(plan.primary(), None);
        assert!(plan.attempt_order().is_empty());
        assert_eq!(
            plan.rejection_reason(SceneRendererKind::Vello),
            Some(RendererSelectionReason::WebGpuUnavailable),
        );
    }

    #[test]
    fn attempt_order_preserves_preference_for_runtime_fallback() {
        let plan = plan_for(PREFERRED, true);
        assert_eq!(
            plan.attempt_order(),
            [SceneRendererKind::Vello, SceneRendererKind::TinySkia],
        );
        assert_eq!(
            plan.next_after(SceneRendererKind::Vello),
            Some(SceneRendererKind::TinySkia),
        );
        assert_eq!(plan.next_after(SceneRendererKind::TinySkia), None);
    }

    fn plan_for(
        order: &'static [SceneRendererKind],
        webgpu_available: bool,
    ) -> RendererSelectionPlan {
        RendererSelectionPolicy::new(order, order).choose(RendererCapabilities { webgpu_available })
    }
}
