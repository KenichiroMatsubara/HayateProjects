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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[allow(dead_code)] // feature セットごとに有効なバックエンドは 1 つだけ
pub enum SceneRendererKind {
    /// CanvasKit による Web 専用 Scene Renderer（ADR-0148）。CanvasKit の JS/WASM
    /// surface は Web Host が所有し、ここには選択語彙だけを置く。
    CanvasKit,
    Vello,
    TinySkia,
    /// skia-safe によるネイティブ専用（desktop + Android）Scene Renderer（ADR-0146/0147）。
    /// ネイティブの standard alternative — 既定順序は vello → skia の一方向 fallback。
    /// wasm32 対象外（web の policy には現れない）。
    Skia,
    /// tiny-skia の置き換え候補として検証中の CPU レンダラ（vello_cpu、Web限定スパイク）。
    VelloCpu,
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

    /// カラーグリフ（COLR/CPAL、ビットマップストライク）を描けるか。Vello
    /// （`draw_glyphs().draw()` が COLR/CPAL フォントを `try_draw_colr` に流す）と
    /// skia（scaler がカラーグリフを処理・ADR-0146。Vello 以外で初）が描ける。他の
    /// CPU ペインタはアウトラインのみなので、そこではカラー絵文字がモノクロに退化
    /// する（ADR-0101）。アダプタはフォント調達時にカラー版/モノクロ版を選ぶため
    /// これを参照する。
    pub fn paints_color_glyphs(self) -> bool {
        matches!(self, Self::Vello | Self::Skia)
    }

    /// ログ・エラーメッセージ用の安定したレンダラ ID。
    pub fn name(self) -> &'static str {
        match self {
            Self::CanvasKit => "canvaskit",
            Self::Vello => "vello",
            Self::TinySkia => "tiny-skia",
            Self::Skia => "skia",
            Self::VelloCpu => "vello-cpu",
            Self::Recording => "recording",
            Self::Null => "null",
        }
    }
}

/// Web の既定初期選択順（ADR-0148）。初回 boot で未選択候補だけをこの順に試す。
pub const WEB_RENDERER_ORDER: &[SceneRendererKind] = &[
    SceneRendererKind::CanvasKit,
    SceneRendererKind::Vello,
    SceneRendererKind::TinySkia,
];

/// Web Host 用の Renderer Selection Policy。CanvasKit のロード・surface 初期化が失敗した
/// 場合に限り、同じ boot 中で次の候補へ進む。選択後の terminal failure は RenderHost が扱う。
pub fn web_renderer_selection_policy() -> RendererSelectionPolicy {
    RendererSelectionPolicy::new(WEB_RENDERER_ORDER, WEB_RENDERER_ORDER)
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

/// 初回選択後に runtime fallback を許さない renderer。CanvasKit と Native skia-safe は
/// surface/context の寿命を Host 側で透過的に取り直せないため、描画・clear・present の
/// 失敗を App Host へ terminal failure として返す（ADR-0148）。
pub fn has_terminal_runtime_failure(kind: SceneRendererKind) -> bool {
    matches!(kind, SceneRendererKind::CanvasKit | SceneRendererKind::Skia)
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

#[cfg(any(
    feature = "backend-vello",
    feature = "backend-tiny-skia",
    feature = "backend-vello-cpu"
))]
const PRODUCTION_RENDERERS: &[SceneRendererKind] = &[
    SceneRendererKind::Vello,
    SceneRendererKind::TinySkia,
    SceneRendererKind::VelloCpu,
];

/// C3 コーデック統合テストは `--features backend-null` のみでビルドする。
#[cfg(all(
    feature = "backend-null",
    not(any(
        feature = "backend-vello",
        feature = "backend-tiny-skia",
        feature = "backend-vello-cpu"
    ))
))]
const PRODUCTION_RENDERERS: &[SceneRendererKind] = &[SceneRendererKind::Null];

/// 診断 init 用の予約（ADR-0050）。本番 `init` では使わない。
const DIAGNOSTIC_RENDERERS: &[SceneRendererKind] =
    &[SceneRendererKind::Recording, SceneRendererKind::Null];

/// ネイティブ（desktop + Android）の標準順序（ADR-0146 §2・spec §4 REND-15）:
/// vello を preferred default、skia を standard alternative とする一方向 fallback。
pub const NATIVE_RENDERER_ORDER: &[SceneRendererKind] =
    &[SceneRendererKind::Vello, SceneRendererKind::Skia];

const NATIVE_VELLO_ONLY: &[SceneRendererKind] = &[SceneRendererKind::Vello];
const NATIVE_SKIA_ONLY: &[SceneRendererKind] = &[SceneRendererKind::Skia];

/// ネイティブ Platform Front（desktop winit / Android）用の選択ポリシー（issue #801、
/// spec §4 REND-15）。
///
/// - `vello_linked`: このビルドが vello/wgpu をリンクしているか（`backend-vello`
///   feature、default on）。off ビルドでは skia が唯一のレンダラとして起動し、vello の
///   不在は `DisabledByPolicy` として観測可能に残る。
/// - `forced`: ランタイム上書き（desktop: env / CLI フラグ、Android: intent extra）。
///   強制されたレンダラだけを許可し、他は `DisabledByPolicy` で観測可能に見送る。
///   リンクされていない vello の強制は未指定と同じ既定へ落とす（ADR-0145 の
///   「未知値は既定へ」流儀。呼び出し側が warn ログを出す）。
///
/// preferred 順序は常に [`NATIVE_RENDERER_ORDER`]（vello → skia）で、runtime 失敗時の
/// 一方向 fallback もこの順序に従う（ADR-0050）。
pub fn native_renderer_selection_policy(
    vello_linked: bool,
    forced: Option<SceneRendererKind>,
) -> RendererSelectionPolicy {
    let allowed = match forced {
        Some(SceneRendererKind::Vello) if vello_linked => NATIVE_VELLO_ONLY,
        Some(SceneRendererKind::Skia) => NATIVE_SKIA_ONLY,
        _ if vello_linked => NATIVE_RENDERER_ORDER,
        _ => NATIVE_SKIA_ONLY,
    };
    RendererSelectionPolicy::new(allowed, NATIVE_RENDERER_ORDER)
}

#[cfg(any(
    feature = "backend-vello",
    feature = "backend-tiny-skia",
    feature = "backend-vello-cpu",
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
    fn only_vello_and_skia_paint_color_glyphs() {
        // カラー/モノクロ絵文字の分岐を駆動する。Vello は COLR/CPAL を try_draw_colr に
        // 流し、skia は scaler がカラーグリフを処理する（ADR-0146）。他の CPU/診断
        // ペインタはアウトラインのみ描く。
        assert!(SceneRendererKind::Vello.paints_color_glyphs());
        assert!(SceneRendererKind::Skia.paints_color_glyphs());
        assert!(!SceneRendererKind::TinySkia.paints_color_glyphs());
        assert!(!SceneRendererKind::VelloCpu.paints_color_glyphs());
        assert!(!SceneRendererKind::Recording.paints_color_glyphs());
        assert!(!SceneRendererKind::Null.paints_color_glyphs());
    }

    #[test]
    fn skia_is_a_cpu_renderer_that_does_not_require_webgpu() {
        // skia raster は wgpu 非依存の CPU present 経路（issue #801）。WebGPU / GPU の
        // 有無で policy に却下されてはならない。
        assert!(!SceneRendererKind::Skia.requires_webgpu());
        assert_eq!(SceneRendererKind::Skia.name(), "skia");
    }

    #[test]
    fn skia_color_glyph_capability_drives_emoji_font_procurement_seam() {
        // ADR-0101 のシーム: アダプタは `paints_color_glyphs()` を core の
        // `upgrades_to_color_emoji` に渡してカラー版/モノクロ版フォントを選ぶ。
        // skia が true を返すことで、emoji フォールバックファミリがカラービルドへ
        // 格上げされる（Vello 以外で初）。
        use hayate_core::element::font_coverage::{upgrades_to_color_emoji, EMOJI_FALLBACK_FAMILY};
        assert!(upgrades_to_color_emoji(
            EMOJI_FALLBACK_FAMILY,
            SceneRendererKind::Skia.paints_color_glyphs()
        ));
        assert!(!upgrades_to_color_emoji(
            EMOJI_FALLBACK_FAMILY,
            SceneRendererKind::TinySkia.paints_color_glyphs()
        ));
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

    #[test]
    fn web_boot_attempts_canvaskit_then_vello_then_tiny_skia() {
        let plan = web_renderer_selection_policy().choose(RendererCapabilities {
            webgpu_available: true,
        });

        assert_eq!(
            plan.attempt_order(),
            [
                SceneRendererKind::CanvasKit,
                SceneRendererKind::Vello,
                SceneRendererKind::TinySkia,
            ],
            "web boot must only advance through unselected candidates in ADR-0148 order",
        );
    }

    fn plan_for(
        order: &'static [SceneRendererKind],
        webgpu_available: bool,
    ) -> RendererSelectionPlan {
        RendererSelectionPolicy::new(order, order).choose(RendererCapabilities { webgpu_available })
    }

    /// ネイティブ（issue #801）: capability は「wgpu adapter の有無は init を試すまで
    /// 分からない」ため常に true を渡し、失敗は init フェーズの一方向 fallback が拾う。
    const NATIVE_CAPS: RendererCapabilities = RendererCapabilities {
        webgpu_available: true,
    };

    #[test]
    fn native_default_order_is_vello_then_skia_one_way_fallback() {
        // ADR-0146 §2: vello が preferred default、skia が standard alternative。
        let plan = native_renderer_selection_policy(true, None).choose(NATIVE_CAPS);
        assert_eq!(
            plan.attempt_order(),
            [SceneRendererKind::Vello, SceneRendererKind::Skia],
        );
        assert_eq!(
            plan.next_after(SceneRendererKind::Vello),
            Some(SceneRendererKind::Skia),
        );
        assert_eq!(plan.next_after(SceneRendererKind::Skia), None);
    }

    #[test]
    fn native_forced_skia_rejects_vello_as_disabled_by_policy() {
        // env / CLI / intent extra の強制指定（issue #801/#802）。見送った vello は
        // 既存の RendererSelectionReason 語彙で観測可能。
        let plan = native_renderer_selection_policy(true, Some(SceneRendererKind::Skia))
            .choose(NATIVE_CAPS);
        assert_eq!(plan.attempt_order(), [SceneRendererKind::Skia]);
        assert_eq!(
            plan.rejection_reason(SceneRendererKind::Vello),
            Some(RendererSelectionReason::DisabledByPolicy),
        );
    }

    #[test]
    fn native_forced_vello_rejects_skia_as_disabled_by_policy() {
        let plan = native_renderer_selection_policy(true, Some(SceneRendererKind::Vello))
            .choose(NATIVE_CAPS);
        assert_eq!(plan.attempt_order(), [SceneRendererKind::Vello]);
        assert_eq!(
            plan.rejection_reason(SceneRendererKind::Skia),
            Some(RendererSelectionReason::DisabledByPolicy),
        );
    }

    #[test]
    fn native_without_vello_linked_boots_skia_alone() {
        // `backend-vello` feature off ビルド（issue #801）: skia が唯一のレンダラとして
        // 起動し、vello の不在は DisabledByPolicy として観測可能。
        let plan = native_renderer_selection_policy(false, None).choose(NATIVE_CAPS);
        assert_eq!(plan.attempt_order(), [SceneRendererKind::Skia]);
        assert_eq!(
            plan.rejection_reason(SceneRendererKind::Vello),
            Some(RendererSelectionReason::DisabledByPolicy),
        );
    }

    #[test]
    fn native_forcing_an_unlinked_vello_falls_back_to_the_default_policy() {
        // vello をリンクしないビルドで vello を強制されても壊れない — 未指定と同じ
        // 既定（この構成では skia 単独）へ落とす（ADR-0145 の「未知値は既定へ」流儀）。
        let plan = native_renderer_selection_policy(false, Some(SceneRendererKind::Vello))
            .choose(NATIVE_CAPS);
        assert_eq!(plan.attempt_order(), [SceneRendererKind::Skia]);
    }
}
