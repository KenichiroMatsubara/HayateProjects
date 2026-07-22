//! App Host — platform 非依存の最上位協調層であり mount 先（ADR-0117）。
//!
//! App Host は `ElementTree` 実体を所有し、フレームループ（[`AppHost::tick`]）・
//! `Event Delivery` の drain・描画オーケストレーションを担う。OS フレームループ自体は
//! 所有せず、Platform Front（web `requestAnimationFrame` / Android `Choreographer`）が
//! 毎フレーム [`AppHost::tick`] を呼ぶ。フレームを起こす唯一の入口は構築時に受け取る
//! `request_redraw` クロージャで、`tick` 末尾に pending visual work（進行中 transition 等）が
//! 残るときだけ次フレームを要求し、無ければ idle に落ちる。
//!
//! consumer（in-process の Hayabusa / wire の Tsubame Canvas Renderer）は [`DeliverySink`]
//! を mount 時に渡す。App Host は drain を所有し続け、`tick` で drain した delivery バッチ
//! （空のこともある）を毎フレーム sink へ同期 push する。`ListenerId → handler` の解決と
//! reactive flush は consumer 側の責務で、App Host は consumer 非依存を保つ。
//!
//! ADR-0068 の Render Host は [`render_host`] へ hoist 済み（ADR-0132 スライス3）。GPU
//! サーフェス資源の契約は [`hayate_core::Surface`] が core 所有で持つ。App Host 自身は
//! `Render Host` を「フレームごとに committed frame を提示できる何か」としてのみ扱い、その
//! 最小 seam を [`PresentTarget`] trait の裏に置く（テストは no-op 実装を渡す）。

use hayate_core::{CommittedFrame, ElementTree, EventDelivery};
use hayate_performance_observability::{
    FrameCounters, FrameDeadline, PerformanceObservability, PerformancePhase,
    DEFAULT_REFRESH_RATE_HZ,
};
use std::time::Instant;

mod font_mailbox;
pub mod render_host;
pub mod renderer_selection;

pub use font_mailbox::{FontFetchResult, FontMailbox, FontMailboxHandle};

/// 1 フレーム分の [`CommittedFrame`] の提示先。`Render Host`（[`render_host::RenderHost`]）を
/// App Host から見た最小 seam。headless・テストでは no-op 実装を渡す。
pub trait PresentTarget {
    /// Projection-specific execution failure, observable by headless callers.
    type Error;

    /// 本フレームの一貫した commit view を提示する。
    fn present(&mut self, frame: &CommittedFrame) -> Result<(), Self::Error>;
}

/// `()` を提示先とする no-op `PresentTarget`。headless 実行やテストで使う。
pub struct HeadlessPresentTarget;

impl PresentTarget for HeadlessPresentTarget {
    type Error = std::convert::Infallible;

    fn present(&mut self, _frame: &CommittedFrame) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// consumer が mount 時に渡す push 型 delivery 受け口（ADR-0117）。
///
/// App Host は drain を所有し、drain した delivery バッチ（空のこともある）を毎フレーム
/// [`DeliverySink::handle`] へ同期 push する。consumer は受け取った delivery の handler を
/// 実行し、handler 由来・非同期由来を問わず reactive graph を flush して、結果の Element
/// Layer mutation を `tree` へ発行し、**return する前にそのフレーム分を出し切る**。
pub trait DeliverySink {
    /// drain 済みの delivery バッチを処理し、結果の mutation を `tree` へ発行する。
    /// `deliveries` は空のこともある（毎フレーム無条件に呼ばれる）。
    fn handle(&mut self, deliveries: &[EventDelivery], tree: &mut ElementTree);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FrameId(u64);

impl FrameId {
    pub fn from_u64(value: u64) -> Self {
        Self(value)
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug)]
pub struct PreparedFrame {
    frame_id: FrameId,
    deliveries: Vec<EventDelivery>,
}

impl PreparedFrame {
    pub fn frame_id(&self) -> FrameId {
        self.frame_id
    }

    pub fn deliveries(&self) -> &[EventDelivery] {
        &self.deliveries
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum FrameState {
    Idle,
    Prepared {
        frame_id: FrameId,
        timestamp_ms: f64,
    },
}

/// AppHost と wire projection が共有する唯一の frame protocol state。
pub struct FrameTransaction {
    state: FrameState,
    next_frame_id: u64,
}

impl Default for FrameTransaction {
    fn default() -> Self {
        Self {
            state: FrameState::Idle,
            next_frame_id: 1,
        }
    }
}

impl FrameTransaction {
    pub fn prepare(&mut self, timestamp_ms: f64) -> Result<FrameId, FrameProtocolError> {
        if let FrameState::Prepared { frame_id, .. } = self.state {
            return Err(FrameProtocolError::AlreadyPrepared { frame_id });
        }
        let frame_id = FrameId(self.next_frame_id);
        self.next_frame_id = self.next_frame_id.wrapping_add(1);
        self.state = FrameState::Prepared {
            frame_id,
            timestamp_ms,
        };
        Ok(frame_id)
    }

    pub fn commit(&mut self, frame_id: FrameId) -> Result<f64, FrameProtocolError> {
        match self.state {
            FrameState::Idle => Err(FrameProtocolError::NotPrepared),
            FrameState::Prepared {
                frame_id: expected, ..
            } if expected != frame_id => Err(FrameProtocolError::MismatchedFrameId {
                expected,
                received: frame_id,
            }),
            FrameState::Prepared { timestamp_ms, .. } => {
                self.state = FrameState::Idle;
                Ok(timestamp_ms)
            }
        }
    }

    pub fn abort(&mut self, frame_id: FrameId) -> Result<(), FrameProtocolError> {
        self.commit(frame_id).map(|_| ())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameProtocolError {
    AlreadyPrepared {
        frame_id: FrameId,
    },
    NotPrepared,
    MismatchedFrameId {
        expected: FrameId,
        received: FrameId,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub struct FrameExecutionError<E> {
    pub source: E,
}

#[derive(Debug, PartialEq, Eq)]
pub enum FrameCommitError<E> {
    Protocol(FrameProtocolError),
    Execution(FrameExecutionError<E>),
}

/// App Host-owned decision made after one Core commit. Platform Fronts translate
/// `RequestNextFrame` into their native one-shot frame primitive; they do not inspect individual
/// animation, cursor, scroll, or resource states themselves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameContinuation {
    Idle,
    RequestNextFrame,
}

impl FrameContinuation {
    pub fn after_commit(frame: &CommittedFrame) -> Self {
        if frame.has_pending_visual_work() {
            Self::RequestNextFrame
        } else {
            Self::Idle
        }
    }

    pub fn requests_frame(self) -> bool {
        self == Self::RequestNextFrame
    }
}

/// platform 非依存の最上位協調層。`ElementTree` 実体を所有し、`tick` でフレームを進める。
pub struct AppHost<S: PresentTarget> {
    tree: ElementTree,
    surface: S,
    request_redraw: Box<dyn Fn()>,
    sink: Option<Box<dyn DeliverySink>>,
    font_mailbox: FontMailbox,
    frame_transaction: FrameTransaction,
    observability: PerformanceObservability,
}

impl<S: PresentTarget> AppHost<S> {
    /// 空の `ElementTree` で App Host を構築する。`request_redraw` は Platform Front が
    /// 供給するフレーム要求クロージャ（唯一の wake 入口・ADR-0117）。
    pub fn new(surface: S, request_redraw: Box<dyn Fn()>) -> Self {
        Self {
            tree: ElementTree::new(),
            surface,
            request_redraw,
            sink: None,
            font_mailbox: FontMailbox::new(),
            frame_transaction: FrameTransaction::default(),
            observability: PerformanceObservability::new(),
        }
    }

    /// アダプタの `impl FontFetcher`（ADR-0132 スライス2）へ渡す、フォント取得完了
    /// 報告用の clone 可能なハンドル。非同期取得クロージャがここへ結果を push し、
    /// `tick` が毎フレーム layout より前に drain する。
    pub fn font_mailbox_handle(&self) -> FontMailboxHandle {
        self.font_mailbox.handle()
    }

    /// consumer が tree を組み立てるための可変参照。App Host が tree を所有するので、
    /// consumer は mount 前にここで element を作り、listener を登録する。
    pub fn tree_mut(&mut self) -> &mut ElementTree {
        &mut self.tree
    }

    /// 所有する `ElementTree` への不変参照。
    pub fn tree(&self) -> &ElementTree {
        &self.tree
    }

    /// 所有する `PresentTarget` への可変参照。`tree_mut` と対称の seam。App Host が surface を
    /// 所有しても、Platform Front は resize 時に present サーフェス（wgpu surface 等）を
    /// 再 configure する必要があるため、ここで具体型 `S` へ可変アクセスする。
    pub fn surface_mut(&mut self) -> &mut S {
        &mut self.surface
    }

    /// The bounded report from the most recently committed frame, when this binary was compiled
    /// with `performance-observability`. Production hosts return `None` without recording.
    pub fn latest_performance_report(
        &self,
    ) -> Option<hayate_performance_observability::FrameReport> {
        self.observability.latest_report()
    }

    /// consumer の [`DeliverySink`] を登録する（mount）。
    pub fn mount(&mut self, sink: Box<dyn DeliverySink>) {
        self.sink = Some(sink);
    }

    pub fn prepare_frame(
        &mut self,
        timestamp_ms: f64,
    ) -> Result<PreparedFrame, FrameProtocolError> {
        let frame_id = self.frame_transaction.prepare(timestamp_ms)?;
        for result in self.font_mailbox.drain() {
            match result {
                FontFetchResult::Loaded { family, bytes } => {
                    self.tree.register_font(&family, bytes)
                }
                FontFetchResult::Failed { family } => {
                    self.tree.font_fetch_failed(&family);
                }
            }
        }
        let deliveries = self.tree.poll_deliveries();
        Ok(PreparedFrame {
            frame_id,
            deliveries,
        })
    }

    pub fn commit_frame(&mut self, frame_id: FrameId) -> Result<(), FrameCommitError<S::Error>> {
        let mut observation = self
            .observability
            .begin_frame(FrameDeadline::from_refresh_rate_hz(DEFAULT_REFRESH_RATE_HZ));
        let app_host_started = observation.is_enabled().then(Instant::now);
        let timestamp_ms = self
            .frame_transaction
            .commit(frame_id)
            .map_err(FrameCommitError::Protocol)?;
        let frame = observation.measure(PerformancePhase::CoreCommit, || {
            self.tree.commit_rendered_frame(timestamp_ms)
        });
        observation.set_counters(FrameCounters {
            nodes: frame.snapshot().len() as u32,
            layers: frame.layer_topology().paint_order().len() as u32,
            dirty_layers: frame.layer_topology().content_changed().len() as u32,
            cache_hits: 0,
            cache_misses: 0,
            allocations: 0,
            ..FrameCounters::default()
        });
        let present = observation.measure(PerformancePhase::RendererPresent, || {
            self.surface.present(&frame)
        });
        if let Some(started) = app_host_started {
            observation.record_phase(
                PerformancePhase::AppHost,
                started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64,
            );
        }
        observation.finish();
        present.map_err(|source| FrameCommitError::Execution(FrameExecutionError { source }))?;
        if FrameContinuation::after_commit(&frame).requests_frame() {
            (self.request_redraw)();
        }
        Ok(())
    }

    pub fn abort_frame(&mut self, frame_id: FrameId) -> Result<(), FrameProtocolError> {
        self.frame_transaction.abort(frame_id)
    }

    /// 1 フレーム進める。Platform Front が毎フレーム呼ぶ（ADR-0117 のフェーズ順）。
    ///
    /// 0. **font mailbox drain**：アダプタが非同期取得を完了した font を、layout（3+4）
    ///    より前に `tree.register_font` / `tree.font_fetch_failed` へ流し込む（ADR-0132
    ///    スライス2、「フォント登録は layout より前」という順序不変条件）。
    /// 1. **drain**：App Host が `poll_deliveries()` を drain する（delivery 所有）。
    /// 2. **advance**：DeliverySink を毎フレーム無条件に呼ぶ（空 batch でも）。consumer が
    ///    handler 実行＋reactive flush＋mutation 発行を return 前に済ます。
    /// 3+4. **commit_frame ＋ render**：Core の renderer-ready な `CommittedFrame` を一度だけ
    ///    確定し、その invariant view を `PresentTarget` へ渡す。
    /// 5. **再要求判定**：pending visual work（進行中 transition 等）が残れば `request_redraw`。
    pub fn tick(&mut self, timestamp_ms: f64) -> Result<(), S::Error> {
        let prepared = self
            .prepare_frame(timestamp_ms)
            .expect("tick starts and ends a frame transaction synchronously");
        let frame_id = prepared.frame_id();

        // 2. advance — sink へ同期 push（disjoint なフィールド借用）。
        if let Some(sink) = self.sink.as_mut() {
            sink.handle(prepared.deliveries(), &mut self.tree);
        }

        match self.commit_frame(frame_id) {
            Ok(()) => Ok(()),
            Err(FrameCommitError::Execution(error)) => Err(error.source),
            Err(FrameCommitError::Protocol(error)) => {
                unreachable!("tick created a matching frame id: {error:?}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hayate_core::element::style::Dimension;
    use hayate_core::{Color, DocumentEventKind, ElementKind, Event, PseudoState, StyleProp};
    use std::cell::RefCell;
    use std::rc::Rc;

    /// present 回数を数える `PresentTarget`。
    struct CountingSurface {
        present_count: Rc<RefCell<u32>>,
    }

    impl PresentTarget for CountingSurface {
        type Error = std::convert::Infallible;

        fn present(&mut self, _frame: &CommittedFrame) -> Result<(), Self::Error> {
            *self.present_count.borrow_mut() += 1;
            Ok(())
        }
    }

    /// 可変フィールドを持つ最小 `PresentTarget`。Platform Front の resize 経路を模す。
    struct ReconfigurableSurface {
        configured_size: (u32, u32),
    }

    impl PresentTarget for ReconfigurableSurface {
        type Error = std::convert::Infallible;

        fn present(&mut self, _frame: &CommittedFrame) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    struct FailingSurface;

    impl PresentTarget for FailingSurface {
        type Error = &'static str;

        fn present(&mut self, _frame: &CommittedFrame) -> Result<(), Self::Error> {
            Err("surface lost")
        }
    }

    #[test]
    fn headless_host_observes_typed_present_failure() {
        let mut app = AppHost::new(FailingSurface, Box::new(|| {}));
        build_min_tree(app.tree_mut());

        assert_eq!(app.tick(16.0), Err("surface lost"));
    }

    #[test]
    fn surface_mut_exposes_owned_surface_for_platform_front_resize() {
        // App Host が surface を所有しても、Platform Front は resize で wgpu surface を
        // 再 configure するため可変アクセスが要る（`tree_mut` と対称の seam）。
        let mut app = AppHost::new(
            ReconfigurableSurface {
                configured_size: (0, 0),
            },
            Box::new(|| {}),
        );
        app.surface_mut().configured_size = (1920, 1080);
        assert_eq!(app.surface_mut().configured_size, (1920, 1080));
    }

    /// テスト用の consumer sink。受け取った delivery バッチを記録し、Click delivery を
    /// 受けたら（mutation 発行の代理として）登録済みの text 要素へテキストを書き込む。
    struct RecordingSink {
        /// 各 `handle` 呼び出しで受け取った delivery 件数（空フレームも 0 として記録）。
        batches: Rc<RefCell<Vec<usize>>>,
        /// Click を受けたら text を書き込む対象。
        text_target: hayate_core::ElementId,
    }

    impl DeliverySink for RecordingSink {
        fn handle(&mut self, deliveries: &[EventDelivery], tree: &mut ElementTree) {
            self.batches.borrow_mut().push(deliveries.len());
            for d in deliveries {
                if matches!(d.event, Event::Click { .. }) {
                    tree.element_set_text(self.text_target, "clicked");
                }
            }
        }
    }

    /// root view に button と text 子を持つ最小ツリーを組み、button へ Click listener を登録する。
    /// 戻り値は (button, text)。
    fn build_min_tree(tree: &mut ElementTree) -> (hayate_core::ElementId, hayate_core::ElementId) {
        let root = tree.element_create(0, ElementKind::View);
        let button = tree.element_create(1, ElementKind::Button);
        let text = tree.element_create(2, ElementKind::Text);
        tree.element_append_child(root, button);
        tree.element_append_child(root, text);
        tree.set_root(root);
        tree.register_listener(button, DocumentEventKind::Click);
        (button, text)
    }

    #[test]
    fn tick_drains_delivery_and_sink_mutates_tree() {
        let present_count = Rc::new(RefCell::new(0));
        let surface = CountingSurface {
            present_count: present_count.clone(),
        };
        let mut app = AppHost::new(surface, Box::new(|| {}));

        let (button, text) = build_min_tree(app.tree_mut());

        let batches = Rc::new(RefCell::new(Vec::new()));
        app.mount(Box::new(RecordingSink {
            batches: batches.clone(),
            text_target: text,
        }));

        // Click を合成 → bubble dispatch で button の listener に delivery が積まれる。
        app.tree_mut().dispatch_event(
            DocumentEventKind::Click,
            Event::Click {
                target_id: button,
                x: 0.0,
                y: 0.0,
            },
        );

        app.tick(16.0);

        // sink は 1 件の delivery を受け取り、tree へ mutation を発行した。
        assert_eq!(*batches.borrow(), vec![1]);
        assert_eq!(app.tree().element_get_text(text), "clicked");
        // present は毎 tick 1 回。
        assert_eq!(*present_count.borrow(), 1);
    }

    #[test]
    fn prepare_then_matching_commit_presents_once() {
        let present_count = Rc::new(RefCell::new(0));
        let surface = CountingSurface {
            present_count: present_count.clone(),
        };
        let mut app = AppHost::new(surface, Box::new(|| {}));
        let (button, _text) = build_min_tree(app.tree_mut());
        app.tree_mut().dispatch_event(
            DocumentEventKind::Click,
            Event::Click {
                target_id: button,
                x: 0.0,
                y: 0.0,
            },
        );

        let prepared = app.prepare_frame(16.0).expect("prepare from Idle");
        assert_eq!(prepared.deliveries().len(), 1);
        assert_eq!(*present_count.borrow(), 0);
        app.commit_frame(prepared.frame_id())
            .expect("matching commit");
        assert_eq!(*present_count.borrow(), 1);
    }

    #[test]
    fn protocol_errors_do_not_consume_the_prepared_frame() {
        let present_count = Rc::new(RefCell::new(0));
        let mut app = AppHost::new(
            CountingSurface {
                present_count: present_count.clone(),
            },
            Box::new(|| {}),
        );
        build_min_tree(app.tree_mut());

        assert_eq!(
            app.commit_frame(FrameId(1)),
            Err(FrameCommitError::Protocol(FrameProtocolError::NotPrepared))
        );
        let prepared = app.prepare_frame(16.0).unwrap();
        assert_eq!(
            app.prepare_frame(17.0).unwrap_err(),
            FrameProtocolError::AlreadyPrepared {
                frame_id: prepared.frame_id()
            }
        );
        assert_eq!(
            app.commit_frame(FrameId(prepared.frame_id().get() + 1)),
            Err(FrameCommitError::Protocol(
                FrameProtocolError::MismatchedFrameId {
                    expected: prepared.frame_id(),
                    received: FrameId(prepared.frame_id().get() + 1),
                }
            ))
        );

        app.commit_frame(prepared.frame_id()).unwrap();
        assert_eq!(*present_count.borrow(), 1);
    }

    #[test]
    fn matching_abort_returns_to_idle_without_presenting() {
        let present_count = Rc::new(RefCell::new(0));
        let mut app = AppHost::new(
            CountingSurface {
                present_count: present_count.clone(),
            },
            Box::new(|| {}),
        );
        build_min_tree(app.tree_mut());

        let prepared = app.prepare_frame(16.0).unwrap();
        assert_eq!(
            app.abort_frame(FrameId(prepared.frame_id().get() + 1)),
            Err(FrameProtocolError::MismatchedFrameId {
                expected: prepared.frame_id(),
                received: FrameId(prepared.frame_id().get() + 1),
            })
        );
        app.abort_frame(prepared.frame_id()).unwrap();
        assert_eq!(*present_count.borrow(), 0);
        assert!(app.prepare_frame(32.0).is_ok());
    }

    #[test]
    fn execution_failure_is_typed_and_ends_the_transaction() {
        let mut app = AppHost::new(FailingSurface, Box::new(|| {}));
        build_min_tree(app.tree_mut());
        let prepared = app.prepare_frame(16.0).unwrap();

        assert_eq!(
            app.commit_frame(prepared.frame_id()),
            Err(FrameCommitError::Execution(FrameExecutionError {
                source: "surface lost"
            }))
        );
        assert_eq!(
            app.commit_frame(prepared.frame_id()),
            Err(FrameCommitError::Protocol(FrameProtocolError::NotPrepared))
        );
    }

    #[test]
    fn sink_called_every_tick_even_without_deliveries() {
        let surface = HeadlessPresentTarget;
        let mut app = AppHost::new(surface, Box::new(|| {}));
        let (_button, text) = build_min_tree(app.tree_mut());

        let batches = Rc::new(RefCell::new(Vec::new()));
        app.mount(Box::new(RecordingSink {
            batches: batches.clone(),
            text_target: text,
        }));

        // delivery を一切積まずに 3 フレーム回す。
        app.tick(16.0);
        app.tick(32.0);
        app.tick(48.0);

        // 毎フレーム空 batch で sink が呼ばれている（ADR-0117「毎 tick flush」）。
        assert_eq!(*batches.borrow(), vec![0, 0, 0]);
    }

    #[test]
    fn idle_when_no_pending_visual_work_does_not_request_redraw() {
        let redraws = Rc::new(RefCell::new(0));
        let r = redraws.clone();
        let surface = HeadlessPresentTarget;
        let mut app = AppHost::new(surface, Box::new(move || *r.borrow_mut() += 1));
        let (_button, _text) = build_min_tree(app.tree_mut());
        app.mount(Box::new(RecordingSink {
            batches: Rc::new(RefCell::new(Vec::new())),
            text_target: _text,
        }));

        // transition も blink も無い静的ツリー：render 後に pending visual work は残らない。
        app.tick(16.0);

        assert_eq!(
            *redraws.borrow(),
            0,
            "静止フレームでは request_redraw を出さない"
        );
    }

    #[test]
    fn app_host_continuation_policy_rearms_only_for_pending_visual_work() {
        let mut tree = ElementTree::new();
        let root = tree.element_create(0, ElementKind::View);
        tree.set_root(root);
        tree.element_set_style(
            root,
            &[
                StyleProp::Width(Dimension::px(40.0)),
                StyleProp::Height(Dimension::px(40.0)),
                StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0)),
                StyleProp::TransitionDuration(200.0),
            ],
        );
        tree.element_set_pseudo_style(
            root,
            PseudoState::Hover,
            &[StyleProp::BackgroundColor(Color::new(0.0, 1.0, 0.0, 1.0))],
        );

        let initial = tree.commit_rendered_frame(0.0);
        assert_eq!(
            FrameContinuation::after_commit(&initial),
            FrameContinuation::Idle
        );
        drop(initial);

        tree.update_pointer_hover(Some(root));
        let animated = tree.commit_rendered_frame(16.0);
        assert_eq!(
            FrameContinuation::after_commit(&animated),
            FrameContinuation::RequestNextFrame
        );
    }

    /// WASM 相当のバンドル代役（CI 常設の DejaVu Sans、Latin のみ）。
    fn latin_only_default() -> Vec<u8> {
        std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf")
            .expect("DejaVuSans.ttf present for the test")
    }

    static NOTO_SANS_JP: &[u8] = include_bytes!("../../core/assets/fonts/NotoSansJP.ttf");

    /// Latin のみのバンドル上に日本語 Text を 1 つ置く。戻り値は (app, label)。
    fn app_with_missing_cjk_font() -> (AppHost<HeadlessPresentTarget>, hayate_core::ElementId) {
        let mut app = AppHost::new(HeadlessPresentTarget, Box::new(|| {}));
        let root = app.tree_mut().element_create(0, ElementKind::View);
        let label = app.tree_mut().element_create(1, ElementKind::Text);
        app.tree_mut().element_append_child(root, label);
        app.tree_mut().set_root(root);
        app.tree_mut().set_viewport(400.0, 300.0);
        app.tree_mut()
            .test_set_wasm_like_fonts(latin_only_default());
        app.tree_mut().element_set_text(label, "あ");
        app.tree_mut()
            .element_set_font_family(label, "Noto Sans JP");
        (app, label)
    }

    #[test]
    fn tick_drains_mailbox_and_registers_the_font_before_layout() {
        // ADR-0132 スライス2: mailbox に積まれた成功報告は、その tick の layout より前に
        // `register_font` されるので、同じフレームで正しいグリフにシェイプされる。
        let (mut app, label) = app_with_missing_cjk_font();

        app.tick(0.0);
        assert!(
            app.tree()
                .test_element_glyph_ids(label)
                .iter()
                .any(|&id| id == 0),
            "font がまだ届く前は .notdef（tofu）のはず"
        );

        app.font_mailbox_handle()
            .report_loaded("Noto Sans JP".to_string(), NOTO_SANS_JP.to_vec());
        app.tick(16.0);

        let glyphs = app.tree().test_element_glyph_ids(label);
        assert!(!glyphs.is_empty());
        assert!(
            glyphs.iter().all(|&id| id != 0),
            "tick は mailbox を layout より前に drain して font を登録しなければならない: {glyphs:?}"
        );
    }

    #[test]
    fn tick_drains_mailbox_failures_so_core_retries_on_a_later_frame() {
        let (mut app, _label) = app_with_missing_cjk_font();

        app.tick(0.0);
        let first: Vec<String> = app
            .tree_mut()
            .poll_events()
            .into_iter()
            .filter_map(|e| match e {
                Event::FetchFont { family } => Some(family),
                _ => None,
            })
            .collect();
        assert_eq!(first, vec!["Noto Sans JP".to_string()]);

        app.font_mailbox_handle()
            .report_failed("Noto Sans JP".to_string());
        app.tick(16.0);

        let retried: Vec<String> = app
            .tree_mut()
            .poll_events()
            .into_iter()
            .filter_map(|e| match e {
                Event::FetchFont { family } => Some(family),
                _ => None,
            })
            .collect();
        assert_eq!(
            retried,
            vec!["Noto Sans JP".to_string()],
            "mailbox の失敗報告は core へ伝わり、再要求されなければならない"
        );
    }
}
