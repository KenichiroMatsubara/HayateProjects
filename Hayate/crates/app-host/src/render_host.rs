//! Render Host（ADR-0068・ADR-0132 スライス3）。`RendererSelectionPolicy`（[`crate::renderer_selection`]）
//! が決めた試行順に従ってレンダラーを初期化し、実行時失敗を検出して次のレンダラーへ
//! 一方向フォールバックする（ADR-0050）。GPU/canvas 等の具体的な資源型には一切触れず、
//! [`hayate_core::Surface`] の抽象と、アダプタが実装する [`RendererInit`]（バックエンド
//! 構築・エラー分類）越しにしかそれらへ触れない。
//!
//! `classify_init_error` は #672 の決定どおり adapter 個別実装のまま（wgpu 語彙と
//! adapter 固有のエラー形状が1関数に混在するため共有しない）だが、`RenderHost` の
//! オーケストレーション（どの順で試すか・いつフォールバックするか）はここに hoist
//! されているので、`RendererInit` 越しに呼ぶ。

use std::collections::{HashMap, HashSet};

use anyhow::Error;
use hayate_core::element::id::ElementId;
use hayate_core::{SceneGraph, Surface};
use hayate_layer_compositor::ScrollLayerGeometry;

use crate::renderer_selection::{
    has_terminal_runtime_failure, is_runtime_fallback_reason, RendererCapabilities,
    RendererSelectionPlan, RendererSelectionPolicy, RendererSelectionReason, SceneRendererKind,
};

pub type ClearColor = [f32; 4];

/// 個々の Scene Renderer バックエンド（Vello / tiny-skia / recording / null 等）が
/// 実装する描画契約。`Render Host` はこの trait 越しにバックエンドを一様に扱う
/// （現 `crates/platform/web/src/backend/mod.rs` の `SceneRenderer`、ADR-0132 スライス3で
/// `Result<(), JsValue>` から `Result<(), anyhow::Error>` へ変更して hoist）。
pub trait SceneRenderer {
    fn kind(&self) -> SceneRendererKind;
    fn render_scene(&mut self, scene: &SceneGraph, clear_color: ClearColor) -> Result<(), Error>;
    fn clear(&mut self, clear_color: ClearColor) -> Result<(), Error>;

    /// このバックエンドが per-layer present（#636）を実装するか。true なら present ループは
    /// [`present_layers`](Self::present_layers) を使う。既定（false）は全面 raster にフォールバック。
    ///
    /// ⚠️ ADR-0135 により封印中 — true を返す実装（web `layer-present` feature 等）を製品として
    /// 有効化しないこと。#697 で実 Chromium 実行時に描画バグが確認され、実用段階にないと判定
    /// された。性能上の実害が具体的に発生するまで再開しない（ADR-0135 が定める本人調査用
    /// トグルは例外）。
    fn supports_layer_present(&self) -> bool {
        false
    }

    /// per-layer present の有効・無効をランタイムで切り替える（ADR-0138、tiny-skia/vello_cpu
    /// の比較トグル用）。既定（no-op）はコンパイル時にしか対応を決めないバックエンド
    /// （vello の `layer-present` feature 等）向け——切り替え可能なバックエンドだけが
    /// override して [`supports_layer_present`](Self::supports_layer_present) が読む
    /// フィールドを更新する。
    fn set_layer_present_enabled(&mut self, _enabled: bool) {}

    /// per-layer present（#636・ADR-0125）。既定は全面 `render_scene` にフォールバック
    /// （未対応バックエンド）。
    ///
    /// `scroll_geometry` は `ElementKind::ScrollView` レイヤごとの ADR-0127 overscan 帯ジオメトリ
    /// （#707）——呼び出し側（`present_frame`）が `ElementTree` から一度だけ計算して渡す。
    /// `present_layers` は `&SceneGraph` とレイヤ id しか受け取らず `ElementTree` を持たないため、
    /// scroll offset / viewport / content 高を自分では問い合わせられない（この小さな表がその境界を
    /// またぐ唯一の橋渡し）。対応しないバックエンド（既定実装含む）は無視してよい。
    ///
    /// ⚠️ ADR-0135 により封印中 — 詳細は [`supports_layer_present`](Self::supports_layer_present)。
    fn present_layers(
        &mut self,
        scene: &SceneGraph,
        _layers: &[ElementId],
        _layer_dirty: &HashSet<ElementId>,
        _scroll_geometry: &HashMap<ElementId, ScrollLayerGeometry>,
        clear_color: ClearColor,
    ) -> Result<(), Error> {
        self.render_scene(scene, clear_color)
    }

    /// 描画サーフェスを新しいピクセル寸法に合わせてリサイズする。サイズを持たない
    /// バックエンドは no-op。
    fn resize(&mut self, _width: u32, _height: u32, _content_scale: f32) {}
}

/// アダプタが実装する、`SceneRendererKind` ごとのバックエンド構築とエラー分類
/// （ADR-0132 スライス3）。`RenderHost` はこれを介してのみ具体的なバックエンド型
/// （wasm/canvas 等）へ触れる。`classify_init_error` は #672 の決定を踏襲し、wgpu 語彙と
/// adapter 固有のエラー形状の判定を adapter 個別実装のまま持つ。
/// wasm32 は単一スレッドで `Send` 境界が意味を持たない（`wasm-bindgen-futures::spawn_local`
/// も `Send` を要求しない）。デスクトップ等マルチスレッド環境の adapter が `Send` な
/// future を要求したくなった場合は、この trait とは別に境界を足す形で対応する。
#[allow(async_fn_in_trait)]
pub trait RendererInit<S: Surface> {
    /// 起動時の非同期初期化（一度きり、tick ループはまだ回っていないので mailbox 不要）。
    async fn try_init(
        &self,
        kind: SceneRendererKind,
        surface: S,
    ) -> Result<Box<dyn SceneRenderer>, Error>;

    /// 一方向のランタイムフォールバック用の同期初期化（ADR-0050）。
    fn try_init_sync_for_fallback(
        &self,
        kind: SceneRendererKind,
        surface: S,
    ) -> Result<Box<dyn SceneRenderer>, Error>;

    /// 初期化・実行時エラーを共通語彙 [`RendererSelectionReason`] へ分類する。
    fn classify_init_error(&self, kind: SceneRendererKind, error: &Error) -> RendererSelectionReason;
}

/// `RendererSelectionPolicy` の決定を実行するホスト。ポリシー決定の実行と実行時
/// フォールバックのブックキーピングだけを持ち、レンダラー選択ルール自体は持たない
/// （選択ルールは [`RendererSelectionPolicy`] が純粋関数として持つ）。
pub struct RenderHost<S: Surface, I: RendererInit<S>> {
    surface: S,
    renderer: Option<Box<dyn SceneRenderer>>,
    /// このホストが実行するポリシー決定。どのレンダラーを試すか、なぜ他が
    /// 棄却されたか。ホストは実行するだけで再導出はしない。
    selection_plan: RendererSelectionPlan,
    init: I,
}

impl<S: Surface, I: RendererInit<S>> RenderHost<S, I> {
    /// ポリシーが選んだ試行順に従ってレンダラーを初期化する。`capabilities` は
    /// 呼び出し側（adapter）が検出済みの値を渡す（core/app-host は GPU capability
    /// 検出の方法を一切知らない）。
    pub async fn init_with_policy(
        surface: S,
        selection_policy: RendererSelectionPolicy,
        capabilities: RendererCapabilities,
        init: I,
    ) -> Result<Self, Error> {
        let plan = selection_policy.choose(capabilities);

        // 見送ったレンダラーと理由（RendererSelectionReason 語彙）を成功時にも観測可能に
        // する（issue #801: 選択レンダラ・選択理由を stderr / console ログで確認できる）。
        for rejection in plan.rejected() {
            log::info!(
                "scene renderer rejected: {} ({:?})",
                rejection.renderer.name(),
                rejection.reason
            );
        }

        let mut attempts: Vec<String> = plan
            .rejected()
            .iter()
            .map(|rejection| format!("{}: {:?}", rejection.renderer.name(), rejection.reason))
            .collect();

        for &renderer_kind in plan.attempt_order() {
            match init.try_init(renderer_kind, surface.clone()).await {
                Ok(renderer) => {
                    log::info!("selected scene renderer: {}", renderer.kind().name());
                    return Ok(Self {
                        surface,
                        renderer: Some(renderer),
                        selection_plan: plan,
                        init,
                    });
                }
                Err(error) => {
                    let reason = init.classify_init_error(renderer_kind, &error);
                    log::warn!(
                        "scene renderer init failed: {} ({reason:?})",
                        renderer_kind.name()
                    );
                    attempts.push(format!("{}: {reason:?} ({error})", renderer_kind.name()));
                }
            }
        }

        Err(anyhow::anyhow!(
            "no scene renderer could be selected; attempts: {}",
            attempts.join(", ")
        ))
    }

    fn fallback_after_runtime_failure(
        &mut self,
        error: Error,
        retry: impl FnOnce(&mut dyn SceneRenderer) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let Some(failed_kind) = self.renderer.as_ref().map(|renderer| renderer.kind()) else {
            return Err(error);
        };
        let reason = self.init.classify_init_error(failed_kind, &error);
        if has_terminal_runtime_failure(failed_kind) {
            return Err(anyhow::anyhow!(
                "terminal scene renderer failure: {} ({reason:?}): {error}",
                failed_kind.name(),
            ));
        }
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

        match self
            .init
            .try_init_sync_for_fallback(next_kind, self.surface.clone())
        {
            Ok(mut renderer) => {
                debug_assert!(self.selection_plan.includes(renderer.kind()));
                renderer.resize(self.surface.width(), self.surface.height(), 1.0);
                let retry_result = retry(renderer.as_mut());
                self.renderer = Some(renderer);
                retry_result
            }
            Err(fallback_error) => Err(anyhow::anyhow!(
                "{} failed with {reason:?} ({error}); fallback to {} also failed ({fallback_error})",
                failed_kind.name(),
                next_kind.name(),
            )),
        }
    }
}

impl<S: Surface, I: RendererInit<S>> SceneRenderer for RenderHost<S, I> {
    fn kind(&self) -> SceneRendererKind {
        let renderer = self
            .renderer
            .as_ref()
            .expect("RenderHost has no active scene renderer");
        debug_assert!(self.selection_plan.includes(renderer.kind()));
        renderer.kind()
    }

    fn render_scene(&mut self, scene: &SceneGraph, clear_color: ClearColor) -> Result<(), Error> {
        let Some(renderer) = self.renderer.as_mut() else {
            return Err(anyhow::anyhow!("RenderHost has no active scene renderer"));
        };
        debug_assert!(self.selection_plan.includes(renderer.kind()));
        match renderer.render_scene(scene, clear_color) {
            Ok(()) => Ok(()),
            Err(error) => self.fallback_after_runtime_failure(error, |renderer| {
                renderer.render_scene(scene, clear_color)
            }),
        }
    }

    fn supports_layer_present(&self) -> bool {
        self.renderer
            .as_ref()
            .is_some_and(|renderer| renderer.supports_layer_present())
    }

    fn set_layer_present_enabled(&mut self, enabled: bool) {
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.set_layer_present_enabled(enabled);
        }
    }

    fn present_layers(
        &mut self,
        scene: &SceneGraph,
        layers: &[ElementId],
        layer_dirty: &HashSet<ElementId>,
        scroll_geometry: &HashMap<ElementId, ScrollLayerGeometry>,
        clear_color: ClearColor,
    ) -> Result<(), Error> {
        let Some(renderer) = self.renderer.as_mut() else {
            return Err(anyhow::anyhow!("RenderHost has no active scene renderer"));
        };
        debug_assert!(self.selection_plan.includes(renderer.kind()));
        match renderer.present_layers(scene, layers, layer_dirty, scroll_geometry, clear_color) {
            Ok(()) => Ok(()),
            // ランタイムフォールバック時は次バックエンドの present（既定は全面 raster）へ委ねる。
            Err(error) => self.fallback_after_runtime_failure(error, |renderer| {
                renderer.present_layers(scene, layers, layer_dirty, scroll_geometry, clear_color)
            }),
        }
    }

    fn clear(&mut self, clear_color: ClearColor) -> Result<(), Error> {
        let Some(renderer) = self.renderer.as_mut() else {
            return Err(anyhow::anyhow!("RenderHost has no active scene renderer"));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer_selection::RendererSelectionPolicy;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Clone)]
    struct FakeSurface {
        width: u32,
        height: u32,
    }

    impl Surface for FakeSurface {
        fn width(&self) -> u32 {
            self.width
        }
        fn height(&self) -> u32 {
            self.height
        }
    }

    /// 呼び出しの記録だけを持つ Scene Renderer フェイク。`fails` に含まれる kind への
    /// `render_scene` は常に失敗する（ランタイムフォールバックを駆動するため）。
    struct FakeRenderer {
        kind: SceneRendererKind,
        fails: Rc<RefCell<HashSet<SceneRendererKind>>>,
        resized: Rc<RefCell<Vec<SceneRendererKind>>>,
        layer_present_enabled: Rc<RefCell<bool>>,
    }

    impl SceneRenderer for FakeRenderer {
        fn kind(&self) -> SceneRendererKind {
            self.kind
        }
        fn render_scene(&mut self, _scene: &SceneGraph, _clear_color: ClearColor) -> Result<(), Error> {
            if self.fails.borrow().contains(&self.kind) {
                Err(anyhow::anyhow!("surface lost"))
            } else {
                Ok(())
            }
        }
        fn clear(&mut self, _clear_color: ClearColor) -> Result<(), Error> {
            if self.fails.borrow().contains(&self.kind) {
                Err(anyhow::anyhow!("surface lost"))
            } else {
                Ok(())
            }
        }
        fn resize(&mut self, _width: u32, _height: u32, _content_scale: f32) {
            self.resized.borrow_mut().push(self.kind);
        }
        fn supports_layer_present(&self) -> bool {
            *self.layer_present_enabled.borrow()
        }
        fn set_layer_present_enabled(&mut self, enabled: bool) {
            *self.layer_present_enabled.borrow_mut() = enabled;
        }
    }

    /// init 呼び出しを記録し、`unavailable` に含まれる kind の init は失敗するフェイク。
    struct FakeInit {
        fails: Rc<RefCell<HashSet<SceneRendererKind>>>,
        unavailable: HashSet<SceneRendererKind>,
        resized: Rc<RefCell<Vec<SceneRendererKind>>>,
        sync_init_calls: Rc<RefCell<Vec<SceneRendererKind>>>,
        layer_present_enabled: Rc<RefCell<bool>>,
    }

    impl RendererInit<FakeSurface> for FakeInit {
        async fn try_init(
            &self,
            kind: SceneRendererKind,
            _surface: FakeSurface,
        ) -> Result<Box<dyn SceneRenderer>, Error> {
            if self.unavailable.contains(&kind) {
                return Err(anyhow::anyhow!("not compiled: {}", kind.name()));
            }
            Ok(Box::new(FakeRenderer {
                kind,
                fails: self.fails.clone(),
                resized: self.resized.clone(),
                layer_present_enabled: self.layer_present_enabled.clone(),
            }))
        }

        fn try_init_sync_for_fallback(
            &self,
            kind: SceneRendererKind,
            _surface: FakeSurface,
        ) -> Result<Box<dyn SceneRenderer>, Error> {
            self.sync_init_calls.borrow_mut().push(kind);
            if self.unavailable.contains(&kind) {
                return Err(anyhow::anyhow!("not compiled: {}", kind.name()));
            }
            Ok(Box::new(FakeRenderer {
                kind,
                fails: self.fails.clone(),
                resized: self.resized.clone(),
                layer_present_enabled: self.layer_present_enabled.clone(),
            }))
        }

        fn classify_init_error(&self, kind: SceneRendererKind, error: &Error) -> RendererSelectionReason {
            let message = error.to_string();
            if message.contains("not compiled") {
                RendererSelectionReason::DisabledByPolicy
            } else if kind == SceneRendererKind::Vello && message.contains("webgpu") {
                RendererSelectionReason::WebGpuUnavailable
            } else {
                RendererSelectionReason::SurfaceLost
            }
        }
    }

    const PREFERRED: &[SceneRendererKind] = &[SceneRendererKind::Vello, SceneRendererKind::TinySkia];

    fn policy() -> RendererSelectionPolicy {
        RendererSelectionPolicy::new(PREFERRED, PREFERRED)
    }

    #[test]
    fn init_selects_the_primary_renderer_from_the_plan() {
        pollster::block_on(async {
            let init = FakeInit {
                fails: Rc::new(RefCell::new(HashSet::new())),
                unavailable: HashSet::new(),
                resized: Rc::new(RefCell::new(Vec::new())),
                sync_init_calls: Rc::new(RefCell::new(Vec::new())),
                layer_present_enabled: Rc::new(RefCell::new(true)),
            };
            let host = RenderHost::init_with_policy(
                FakeSurface { width: 800, height: 600 },
                policy(),
                RendererCapabilities { webgpu_available: true },
                init,
            )
            .await
            .expect("a feasible renderer must be selected");

            assert_eq!(host.kind(), SceneRendererKind::Vello);
        });
    }

    #[test]
    fn init_skips_an_unavailable_primary_and_falls_to_the_next() {
        pollster::block_on(async {
            let mut unavailable = HashSet::new();
            unavailable.insert(SceneRendererKind::Vello);
            let init = FakeInit {
                fails: Rc::new(RefCell::new(HashSet::new())),
                unavailable,
                resized: Rc::new(RefCell::new(Vec::new())),
                sync_init_calls: Rc::new(RefCell::new(Vec::new())),
                layer_present_enabled: Rc::new(RefCell::new(true)),
            };
            let host = RenderHost::init_with_policy(
                FakeSurface { width: 800, height: 600 },
                policy(),
                RendererCapabilities { webgpu_available: true },
                init,
            )
            .await
            .expect("tiny-skia must be selected once vello init fails");

            assert_eq!(host.kind(), SceneRendererKind::TinySkia);
        });
    }

    #[test]
    fn init_fails_when_no_renderer_can_be_selected() {
        pollster::block_on(async {
            let mut unavailable = HashSet::new();
            unavailable.insert(SceneRendererKind::Vello);
            unavailable.insert(SceneRendererKind::TinySkia);
            let init = FakeInit {
                fails: Rc::new(RefCell::new(HashSet::new())),
                unavailable,
                resized: Rc::new(RefCell::new(Vec::new())),
                sync_init_calls: Rc::new(RefCell::new(Vec::new())),
                layer_present_enabled: Rc::new(RefCell::new(true)),
            };
            let result = RenderHost::init_with_policy(
                FakeSurface { width: 800, height: 600 },
                policy(),
                RendererCapabilities { webgpu_available: true },
                init,
            )
            .await;

            assert!(result.is_err(), "no feasible renderer must surface as an error");
        });
    }

    #[test]
    fn runtime_failure_falls_back_one_way_following_next_after() {
        pollster::block_on(async {
            let fails = Rc::new(RefCell::new(HashSet::new()));
            fails.borrow_mut().insert(SceneRendererKind::Vello);
            let resized = Rc::new(RefCell::new(Vec::new()));
            let sync_init_calls = Rc::new(RefCell::new(Vec::new()));
            let init = FakeInit {
                fails: fails.clone(),
                unavailable: HashSet::new(),
                resized: resized.clone(),
                sync_init_calls: sync_init_calls.clone(),
                layer_present_enabled: Rc::new(RefCell::new(true)),
            };
            let mut host = RenderHost::init_with_policy(
                FakeSurface { width: 800, height: 600 },
                policy(),
                RendererCapabilities { webgpu_available: true },
                init,
            )
            .await
            .expect("vello selected as primary");
            assert_eq!(host.kind(), SceneRendererKind::Vello);

            // ランタイムで vello が失敗（"surface lost" → フォールバック対象理由）。
            let scene = SceneGraph::default();
            host.render_scene(&scene, [0.0, 0.0, 0.0, 1.0])
                .expect("one-way fallback to tiny-skia must recover the frame");

            assert_eq!(host.kind(), SceneRendererKind::TinySkia);
            assert_eq!(*sync_init_calls.borrow(), vec![SceneRendererKind::TinySkia]);
            assert_eq!(*resized.borrow(), vec![SceneRendererKind::TinySkia]);
        });
    }

    #[test]
    fn canvaskit_runtime_failure_is_terminal_and_never_initializes_vello() {
        pollster::block_on(async {
            let fails = Rc::new(RefCell::new(HashSet::from([SceneRendererKind::CanvasKit])));
            let sync_init_calls = Rc::new(RefCell::new(Vec::new()));
            let init = FakeInit {
                fails,
                unavailable: HashSet::new(),
                resized: Rc::new(RefCell::new(Vec::new())),
                sync_init_calls: sync_init_calls.clone(),
                layer_present_enabled: Rc::new(RefCell::new(true)),
            };
            let mut host = RenderHost::init_with_policy(
                FakeSurface { width: 800, height: 600 },
                RendererSelectionPolicy::new(
                    &[
                        SceneRendererKind::CanvasKit,
                        SceneRendererKind::Vello,
                        SceneRendererKind::TinySkia,
                    ],
                    &[
                        SceneRendererKind::CanvasKit,
                        SceneRendererKind::Vello,
                        SceneRendererKind::TinySkia,
                    ],
                ),
                RendererCapabilities { webgpu_available: true },
                init,
            )
            .await
            .expect("CanvasKit must be selected during boot");

            let result = host.render_scene(&SceneGraph::default(), [0.0, 0.0, 0.0, 1.0]);
            assert!(result.is_err(), "a selected CanvasKit failure must be terminal");
            assert_eq!(host.kind(), SceneRendererKind::CanvasKit);
            assert!(
                sync_init_calls.borrow().is_empty(),
                "terminal failure must not start an unselected renderer",
            );
        });
    }

    #[test]
    fn skia_clear_failure_is_terminal_and_never_restarts_another_renderer() {
        pollster::block_on(async {
            let fails = Rc::new(RefCell::new(HashSet::from([SceneRendererKind::Skia])));
            let sync_init_calls = Rc::new(RefCell::new(Vec::new()));
            let init = FakeInit {
                fails,
                unavailable: HashSet::new(),
                resized: Rc::new(RefCell::new(Vec::new())),
                sync_init_calls: sync_init_calls.clone(),
                layer_present_enabled: Rc::new(RefCell::new(true)),
            };
            let mut host = RenderHost::init_with_policy(
                FakeSurface { width: 800, height: 600 },
                RendererSelectionPolicy::new(
                    &[SceneRendererKind::Skia, SceneRendererKind::Vello],
                    &[SceneRendererKind::Skia, SceneRendererKind::Vello],
                ),
                RendererCapabilities { webgpu_available: true },
                init,
            )
            .await
            .expect("skia must be selected during boot");

            let result = host.clear([0.0, 0.0, 0.0, 1.0]);
            assert!(result.is_err(), "a selected skia clear failure must be terminal");
            assert_eq!(host.kind(), SceneRendererKind::Skia);
            assert!(sync_init_calls.borrow().is_empty());
        });
    }

    #[test]
    fn a_second_runtime_failure_does_not_fall_back_past_the_last_renderer() {
        pollster::block_on(async {
            // one-way: tiny-skia の次は無いので、tiny-skia が失敗したらエラーを素通しする。
            let fails = Rc::new(RefCell::new(HashSet::new()));
            fails.borrow_mut().insert(SceneRendererKind::TinySkia);
            let init = FakeInit {
                fails: fails.clone(),
                unavailable: HashSet::new(),
                resized: Rc::new(RefCell::new(Vec::new())),
                sync_init_calls: Rc::new(RefCell::new(Vec::new())),
                layer_present_enabled: Rc::new(RefCell::new(true)),
            };
            let mut host = RenderHost::init_with_policy(
                FakeSurface { width: 800, height: 600 },
                RendererSelectionPolicy::new(
                    &[SceneRendererKind::TinySkia],
                    &[SceneRendererKind::TinySkia],
                ),
                RendererCapabilities { webgpu_available: false },
                init,
            )
            .await
            .expect("tiny-skia selected as the only renderer");

            let scene = SceneGraph::default();
            let result = host.render_scene(&scene, [0.0, 0.0, 0.0, 1.0]);
            assert!(
                result.is_err(),
                "the last renderer in the plan has nowhere left to fall back to"
            );
        });
    }

    #[test]
    fn set_layer_present_enabled_delegates_to_the_active_renderer() {
        pollster::block_on(async {
            let layer_present_enabled = Rc::new(RefCell::new(true));
            let init = FakeInit {
                fails: Rc::new(RefCell::new(HashSet::new())),
                unavailable: HashSet::new(),
                resized: Rc::new(RefCell::new(Vec::new())),
                sync_init_calls: Rc::new(RefCell::new(Vec::new())),
                layer_present_enabled: layer_present_enabled.clone(),
            };
            let mut host = RenderHost::init_with_policy(
                FakeSurface { width: 800, height: 600 },
                policy(),
                RendererCapabilities { webgpu_available: true },
                init,
            )
            .await
            .expect("a feasible renderer must be selected");

            assert!(host.supports_layer_present());

            host.set_layer_present_enabled(false);
            assert!(
                !host.supports_layer_present(),
                "set_layer_present_enabled(false) must flip supports_layer_present() on the active renderer"
            );

            host.set_layer_present_enabled(true);
            assert!(host.supports_layer_present());
        });
    }
}
