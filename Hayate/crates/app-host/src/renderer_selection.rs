//! Renderer Selection Policy — どの `Scene Renderer` を優先し、どんな根拠で
//! 採否を決めるかのルール（Hayate `CONTEXT.md`）。
//!
//! `Render Host` から切り出した純粋な継ぎ目。`wasm-bindgen` / `web-sys` 型を
//! 一切持たず GPU にも触れない（ADR-0132 スライス1）。判定は
//! [`RendererCapabilities`] の純粋関数なので、実 GPU もブラウザもなく capability
//! 入力だけで検証できる。ホストは capability を集めて結果の
//! [`RendererSelectionPlan`] を消費するだけで、選択ルール自体は持たない。
//!
//! `classify_init_error`（各 platform adapter が個別実装）はここでは扱わない。
//! wgpu 語彙（platform 非依存）と adapter 固有のエラー形状が1関数に混在するため、
//! 共有するのは [`RendererSelectionReason`] という語彙（enum）のみとし、分類ロジック
//! 自体は各 adapter が持つ（ADR-0132）。

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)] // feature セットごとに有効なバックエンドは 1 つだけ
pub enum SceneRendererKind {
    Vello,
    TinySkia,
    /// 非本番レンダラ（ADR-0050）。`init_diagnostic` 経由で使う。
    Recording,
    /// 非本番レンダラ（ADR-0050）。`init_diagnostic` 経由で使う。
    Null,
}

impl SceneRendererKind {
    /// 初期化に `navigator.gpu` が要るか。必要なのは WebGPU ベースの Vello だけで、
    /// 他は CPU で描画する。
    pub fn requires_webgpu(self) -> bool {
        matches!(self, Self::Vello)
    }

    /// カラーグリフ（COLR/CPAL、ビットマップストライク）を描けるか。描けるのは
    /// Vello だけ（`draw_glyphs().draw()` が COLR/CPAL フォントを `try_draw_colr` に
    /// 流す）。CPU ペインタはアウトラインのみなので、そこではカラー絵文字がモノクロに
    /// 退化する（ADR-0101）。アダプタはフォント調達時にカラー版/モノクロ版を選ぶため
    /// これを参照する。
    pub fn paints_color_glyphs(self) -> bool {
        matches!(self, Self::Vello)
    }

    /// ログ・エラーメッセージ用の安定したレンダラ ID。
    pub fn name(self) -> &'static str {
        match self {
            Self::Vello => "vello",
            Self::TinySkia => "tiny-skia",
            Self::Recording => "recording",
            Self::Null => "null",
        }
    }
}

/// `Render Host` がレンダラを採用しなかった、あるいは切り替えた理由。
/// 場当たりのエラー文字列でなく観測可能にするための共通語彙。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RendererSelectionReason {
    WebGpuUnavailable,
    RendererInitFailed,
    SurfaceLost,
    CapabilityUnsupported,
    DisabledByPolicy,
}

/// この理由を伴う実行時失敗が、次のレンダラへの一方向フォールバックを引き起こすか
/// （ADR-0050）。`WebGpuUnavailable` と `DisabledByPolicy` は選択時に確定し、
/// フレーム途中で起きることはない。
pub fn is_runtime_fallback_reason(reason: RendererSelectionReason) -> bool {
    matches!(
        reason,
        RendererSelectionReason::SurfaceLost
            | RendererSelectionReason::CapabilityUnsupported
            | RendererSelectionReason::RendererInitFailed
    )
}

/// [`RendererSelectionPolicy`] が参照する、ホスト環境について静的に分かる事実。
/// どの `Scene Renderer` も初期化せずに集めるので、判定は GPU 非依存かつテスト可能に
/// 保たれる。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RendererCapabilities {
    /// `navigator.gpu` が存在するか。WebGPU ベースのレンダラー（Vello）はこれなしでは
    /// 動かず、CPU レンダラ（tiny-skia）は影響を受けない。
    pub webgpu_available: bool,
}

/// ポリシーが見送った `Scene Renderer` と、その理由を表す
/// `Renderer Selection Reason` の組。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RendererRejection {
    pub renderer: SceneRendererKind,
    pub reason: RendererSelectionReason,
}

/// 与えた capability に対するポリシーの決定。試行するレンダラ（優先順、capability 上
/// 実現可能なものだけ）と、事前に却下したレンダラ（各々理由付き）。呼び出し側がなぜ
/// 選ばれた／選ばれなかったかを報告できるよう観測可能にしてある。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RendererSelectionPlan {
    attempt_order: Vec<SceneRendererKind>,
    rejected: Vec<RendererRejection>,
}

impl RendererSelectionPlan {
    /// 実現可能なものがあれば、ホストが最初に試すべきレンダラ。
    /// 観測用アクセサ。すべてのバックエンド構成が使うわけではない。
    #[allow(dead_code)]
    pub fn primary(&self) -> Option<SceneRendererKind> {
        self.attempt_order.first().copied()
    }

    /// 実現可能なレンダラを、ホストが試すべき順に並べたもの。
    pub fn attempt_order(&self) -> &[SceneRendererKind] {
        &self.attempt_order
    }

    /// ポリシーが init 前に却下したレンダラ（各々理由付き）。
    pub fn rejected(&self) -> &[RendererRejection] {
        &self.rejected
    }

    /// `renderer` が試行シーケンスに含まれるか。ホストは現在アクティブなレンダラに
    /// ついてこの不変条件を維持する。
    pub fn includes(&self, renderer: SceneRendererKind) -> bool {
        self.attempt_order.contains(&renderer)
    }

    /// ポリシーが `renderer` を見送った場合の理由。
    /// 観測用アクセサ。すべてのバックエンド構成が使うわけではない。
    #[allow(dead_code)]
    pub fn rejection_reason(&self, renderer: SceneRendererKind) -> Option<RendererSelectionReason> {
        self.rejected
            .iter()
            .find(|rejection| rejection.renderer == renderer)
            .map(|rejection| rejection.reason)
    }

    /// このプランで `failed` の次に試すレンダラ。実行時フォールバック経路が、選択を
    /// 再導出せずポリシーの決定に従うために使う。
    pub fn next_after(&self, failed: SceneRendererKind) -> Option<SceneRendererKind> {
        let failed_index = self.attempt_order.iter().position(|&kind| kind == failed)?;
        self.attempt_order.get(failed_index + 1).copied()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RendererSelectionPolicy {
    allowed_renderers: &'static [SceneRendererKind],
    preferred_renderer_order: &'static [SceneRendererKind],
}

impl RendererSelectionPolicy {
    pub const fn new(
        allowed_renderers: &'static [SceneRendererKind],
        preferred_renderer_order: &'static [SceneRendererKind],
    ) -> Self {
        Self {
            allowed_renderers,
            preferred_renderer_order,
        }
    }

    pub fn allows(self, renderer: SceneRendererKind) -> bool {
        self.allowed_renderers.contains(&renderer)
    }

    /// 与えた capability に対し、どのレンダラを試すかを決める。純粋関数で、ポリシーの
    /// レンダラリストと capability 入力だけを使うので、実 GPU なしで完全にテストできる。
    /// 許可されない、または capability を満たさないレンダラは事前に除外して理由付きで
    /// [`RendererSelectionPlan::rejected`] に記録し、残りは優先順のまま試行シーケンスにする。
    pub fn choose(self, capabilities: RendererCapabilities) -> RendererSelectionPlan {
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

/// C3 コーデック統合テストは `--features backend-null` のみでビルドする。
#[cfg(all(
    feature = "backend-null",
    not(any(feature = "backend-vello", feature = "backend-tiny-skia"))
))]
const PRODUCTION_RENDERERS: &[SceneRendererKind] = &[SceneRendererKind::Null];

/// 診断 init 用の予約（ADR-0050）。本番 `init` では使わない。
const DIAGNOSTIC_RENDERERS: &[SceneRendererKind] =
    &[SceneRendererKind::Recording, SceneRendererKind::Null];

#[cfg(any(
    feature = "backend-vello",
    feature = "backend-tiny-skia",
    feature = "backend-null"
))]
pub fn standard_renderer_selection_policy() -> RendererSelectionPolicy {
    RendererSelectionPolicy::new(PRODUCTION_RENDERERS, PRODUCTION_RENDERERS)
}

/// 診断 init 用の予約（ADR-0050）。本番 `init` では使わない。
pub fn diagnostic_renderer_selection_policy() -> RendererSelectionPolicy {
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
    fn only_vello_paints_color_glyphs() {
        // カラー/モノクロ絵文字の分岐を駆動する。GPU ペインタは COLR/CPAL を
        // try_draw_colr に流し、CPU/診断ペインタはアウトラインのみ描く。
        assert!(SceneRendererKind::Vello.paints_color_glyphs());
        assert!(!SceneRendererKind::TinySkia.paints_color_glyphs());
        assert!(!SceneRendererKind::Recording.paints_color_glyphs());
        assert!(!SceneRendererKind::Null.paints_color_glyphs());
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
        // tiny-skia は CPU 描画なので、WebGPU 喪失で却下してはならない。
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
        // Vello のみ許可だが WebGPU が無い。試行可能なものはなく、理由は
        // むき出しの失敗ではなく観測可能。
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
