//! native の UI/Raster 二スレッド分離（ADR-0128）。
//!
//! core（tree / layout / scene_build / lowering）は ADR-0003 どおり **単一スレッドのまま「UI
//! スレッド」**に留め、#610 で `Send` クリーンな seam の裏に隔離したレイヤキャッシュ＋compositor
//! （Vello raster + wgpu compositor）だけを専用 **Raster スレッド**へ移す（Flutter 同型）。
//!
//! **スレッド境界＝[`RasterHandoff`]（immutable Scene Snapshot ＋ Layer Topology）の受け渡し**で、
//! UI スレッドが produce、Raster スレッドが consume する。`Send + Sync` 境界はこのハンドオフに引く。
//! ハンドオフは非ブロッキング channel なので、重い raster が UI スレッドの入力処理を止めない。
//!
//! Raster 側が実行中の 1 frame に加え、連続する frame の未処理 slot は最大 1 件。
//! latest replacement、dirty union、lifecycle barrier、terminal failure、overload observability は
//! platform-free な [`LatestWinsFramePipeline`] が所有する。この module はその共通 policy を
//! `std::thread` と wake primitive で駆動する Native execution adapter に限る。
//!
//! 実 Vello/wgpu の raster/composite は [`RasterThread::spawn`] に渡す sink が担う（native backend が
//! cache+compositor を所有して Raster スレッド上だけで触る）。本モジュールはスレッドモデルと境界型を
//! host で固定し、出力がシングルスレッド時と同値であることをテストする。

use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};

use hayate_frame_pipeline::{
    FrameSubmission, LatestWinsFramePipeline, PipelineCommand, PipelineObservation,
};

/// UI スレッド → Raster スレッドのハンドオフ（ADR-0128）。スレッド境界はこれ 1 つで、lower 済み
/// Scene Snapshot・Layer Topology・scroll facts を owned で渡す。`Send + Sync` 境界。
pub type RasterHandoff = FrameSubmission;

/// UI スレッド → Raster スレッドのメッセージ（ADR-0128）。フレーム提示だけでなく surface ライフ
/// サイクル（resize / 破棄 / 再作成）も同じ順序付きチャネルで渡し、Raster スレッドが surface と
/// swapchain present を所有する（present をまたぐ順序が壊れないよう 1 本のチャネルに直列化する）。
/// surface ハンドルは backend 固有（Android の `ANativeWindow` 等）なので、再作成は sink 側が握る
/// factory を起動する [`RasterCommand::RebuildSurface`] で表す（型としては unit を運ぶ）。
pub enum RasterCommand {
    /// 1 フレームを raster/composite して present する。
    Frame(RasterHandoff),
    /// surface サイズ変更（swapchain 再構成＋レイヤ texture invalidate）。`content_scale` は
    /// レイヤ raster（Vello）が論理座標を物理バッファへ引き伸ばす倍率（DPI 対応, ADR-0080 の
    /// Android 延長）。
    Resize {
        width: u32,
        height: u32,
        content_scale: f32,
    },
    /// surface が失われた（Android TerminateWindow）。以後の Frame は present をスキップする。
    SurfaceLost,
    /// surface を再構築する（新規作成 / Torimi full reload）。sink が握る factory を起動する。
    RebuildSurface,
}

enum RasterBarrier {
    Resize {
        width: u32,
        height: u32,
        content_scale: f32,
    },
    SurfaceLost,
    RebuildSurface,
}

impl RasterCommand {
    fn into_pipeline(self) -> PipelineCommand<RasterHandoff, RasterBarrier> {
        match self {
            Self::Frame(frame) => PipelineCommand::Frame(frame),
            Self::Resize {
                width,
                height,
                content_scale,
            } => PipelineCommand::Barrier(RasterBarrier::Resize {
                width,
                height,
                content_scale,
            }),
            Self::SurfaceLost => PipelineCommand::Barrier(RasterBarrier::SurfaceLost),
            Self::RebuildSurface => PipelineCommand::Barrier(RasterBarrier::RebuildSurface),
        }
    }

    fn from_pipeline(command: PipelineCommand<RasterHandoff, RasterBarrier>) -> Self {
        match command {
            PipelineCommand::Frame(frame) => Self::Frame(frame),
            PipelineCommand::Barrier(RasterBarrier::Resize {
                width,
                height,
                content_scale,
            }) => Self::Resize {
                width,
                height,
                content_scale,
            },
            PipelineCommand::Barrier(RasterBarrier::SurfaceLost) => Self::SurfaceLost,
            PipelineCommand::Barrier(RasterBarrier::RebuildSurface) => Self::RebuildSurface,
        }
    }
}

/// ハンドオフ失敗（Raster スレッドが既に終了している）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RasterHandoffError {
    Disconnected,
    /// The selected renderer failed after boot. This state is terminal: no retry, fallback, or
    /// worker restart is attempted.
    TerminalFailure,
}

struct AdapterState {
    pipeline: LatestWinsFramePipeline<RasterHandoff, RasterBarrier>,
    closed: bool,
}

struct Shared {
    state: Mutex<AdapterState>,
    condvar: Condvar,
}

/// 専用 Raster スレッド。UI スレッド（core＝単一スレッドのまま）が produce した
/// [`RasterCommand`] を受けて raster/composite する。`sink` は cache+compositor
/// （#610 の `Send` クリーン seam）と surface を所有し、Raster スレッド上だけで触る。core 自体は
/// マルチスレッド化しない（ADR-0003 維持）。queue policy は
/// [`LatestWinsFramePipeline`] が所有し、この adapter は thread 起動・wake・完了通知だけを行う。
pub struct RasterThread {
    shared: Option<Arc<Shared>>,
    handle: Option<JoinHandle<()>>,
}

impl RasterThread {
    /// 各メッセージを `sink` で処理する Raster スレッドを起動する。
    pub fn spawn<S>(mut sink: S) -> Self
    where
        S: FnMut(RasterCommand) + Send + 'static,
    {
        Self::spawn_driver(move |message| {
            sink(message);
            true
        })
    }

    /// Spawn a renderer whose first runtime error permanently closes the handoff. This is the
    /// selected-renderer policy: boot-time candidate fallback happens before this worker exists;
    /// runtime render/surface/context failures never retry, fall back, or restart.
    pub fn spawn_fallible<S, E>(mut sink: S) -> Self
    where
        S: FnMut(RasterCommand) -> Result<(), E> + Send + 'static,
    {
        Self::spawn_driver(move |message| sink(message).is_ok())
    }

    fn spawn_driver<D>(mut drive: D) -> Self
    where
        D: FnMut(RasterCommand) -> bool + Send + 'static,
    {
        let shared = Arc::new(Shared {
            state: Mutex::new(AdapterState {
                pipeline: LatestWinsFramePipeline::new(),
                closed: false,
            }),
            condvar: Condvar::new(),
        });
        let worker_shared = Arc::clone(&shared);
        let handle = thread::spawn(move || loop {
            let message = {
                let mut state = worker_shared.state.lock().unwrap();
                loop {
                    if let Some(message) = state.pipeline.start_next() {
                        break Some(message);
                    }
                    if state.closed {
                        break None;
                    }
                    state = worker_shared.condvar.wait(state).unwrap();
                }
            };
            match message {
                Some(message) => {
                    let succeeded = drive(RasterCommand::from_pipeline(message));
                    let mut state = worker_shared.state.lock().unwrap();
                    if succeeded {
                        state
                            .pipeline
                            .complete_active()
                            .expect("worker completion must correspond to its active command");
                    } else {
                        state.pipeline.fail();
                        state.closed = true;
                        worker_shared.condvar.notify_all();
                        break;
                    }
                }
                // closed かつキューが空＝送信側が全て drop された（綺麗な終了）。
                None => break,
            }
        });
        Self {
            shared: Some(shared),
            handle: Some(handle),
        }
    }

    pub fn has_terminal_failure(&self) -> bool {
        self.observation().failure
    }

    pub fn observation(&self) -> PipelineObservation {
        self.shared
            .as_ref()
            .and_then(|shared| {
                shared
                    .state
                    .lock()
                    .ok()
                    .map(|state| state.pipeline.observation())
            })
            .unwrap_or_default()
    }

    /// UI スレッドからメッセージを渡す（非ブロッキング）。raster 完了を待たずに返るので、UI スレッドは
    /// 続けて入力処理・次フレーム生成ができる（重い raster が入力を止めない・ADR-0128）。
    ///
    /// Queue admission and coalescing are delegated to the shared platform-free pipeline.
    pub fn send(&self, message: RasterCommand) -> Result<(), RasterHandoffError> {
        let Some(shared) = self.shared.as_ref() else {
            return Err(RasterHandoffError::Disconnected);
        };
        let mut state = shared
            .state
            .lock()
            .map_err(|_| RasterHandoffError::Disconnected)?;
        if state.pipeline.observation().failure {
            return Err(RasterHandoffError::TerminalFailure);
        }
        if state.closed {
            return Err(RasterHandoffError::Disconnected);
        }
        state
            .pipeline
            .admit(message.into_pipeline())
            .map_err(|_| RasterHandoffError::TerminalFailure)?;
        shared.condvar.notify_one();
        Ok(())
    }

    /// Raster スレッドを停止して join する（surface 破棄 / reload で明示的に畳むとき）。送信済みで
    /// まだキューにあるメッセージは畳む前に drain される。以後の [`send`](Self::send) は
    /// `Disconnected` を返す。Drop でも同じ手順で畳まれる。
    pub fn shutdown(&mut self) {
        if let Some(shared) = self.shared.take() {
            shared.state.lock().unwrap().closed = true;
            shared.condvar.notify_all();
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for RasterThread {
    fn drop(&mut self) {
        // closed を先に立てて worker がキュー drain 後に抜けられるようにしてから join する
        // （join 先行は deadlock）。
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hayate_core::element::style::{Dimension, StyleProp};
    use hayate_core::{Color, ElementId, ElementKind, ElementTree, SceneSnapshot};
    use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
    use std::sync::{mpsc, Arc};

    fn id(raw: u64) -> ElementId {
        ElementId::from_u64(raw)
    }

    fn handoff(dirty: &[u64]) -> RasterHandoff {
        let mut tree = ElementTree::new();
        let root = tree.element_create(u64::MAX, ElementKind::View);
        tree.set_root(root);
        tree.element_set_style(
            root,
            &[
                StyleProp::Width(Dimension::px(100.0)),
                StyleProp::Height(Dimension::px(100.0)),
            ],
        );
        let mut layers = dirty.to_vec();
        layers.sort_unstable();
        layers.dedup();
        for &raw in &layers {
            let layer = tree.element_create(raw, ElementKind::View);
            tree.element_append_child(root, layer);
            tree.element_set_transform(layer, Some([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]));
        }
        let _initial = tree.commit_rendered_frame(0.0);
        for &raw in &layers {
            tree.element_set_style(
                id(raw),
                &[StyleProp::BackgroundColor(Color::new(1.0, 0.0, 0.0, 1.0))],
            );
        }
        RasterHandoff::from_committed_frame(&tree.commit_rendered_frame(16.0))
    }

    /// 決定的な「描画結果」：dirty レイヤ id を昇順に。シングル/マルチスレッドの parity 比較に使う。
    fn rasterize(h: &RasterHandoff) -> Vec<u64> {
        let mut v: Vec<u64> = h
            .topology
            .content_changed()
            .iter()
            .map(|i| i.to_u64())
            .collect();
        v.sort_unstable();
        v
    }

    #[test]
    fn scene_snapshot_handoff_is_send_and_sync() {
        // ADR-0128/0153: thread handoff owns the immutable snapshot handle, not mutable storage.
        fn assert_send_sync<T: Send + Sync>() {}
        fn assert_snapshot(_: &SceneSnapshot) {}
        assert_snapshot(&handoff(&[]).scene);
        assert_send_sync::<SceneSnapshot>();
        assert_send_sync::<RasterHandoff>();
    }

    #[test]
    fn raster_runs_on_a_separate_thread_from_the_ui_thread() {
        let ui_thread = thread::current().id();
        let raster_thread = Arc::new(std::sync::Mutex::new(None));
        let captured = Arc::clone(&raster_thread);
        let rt = RasterThread::spawn(move |_command: RasterCommand| {
            *captured.lock().unwrap() = Some(thread::current().id());
        });
        rt.send(RasterCommand::Frame(handoff(&[1]))).unwrap();
        drop(rt); // sender drop → join（ワーカー完了を待つ）

        let raster = raster_thread.lock().unwrap().expect("raster ran");
        assert_ne!(raster, ui_thread, "raster は UI スレッドと別スレッドで走る");
    }

    #[test]
    fn heavy_raster_does_not_block_the_ui_thread() {
        let processed = Arc::new(AtomicUsize::new(0));
        let worker_count = Arc::clone(&processed);
        // gate で「重い raster」をシミュレート：UI が解放するまでワーカーは完了しない。
        let (gate_tx, gate_rx) = mpsc::channel::<()>();
        let rt = RasterThread::spawn(move |_command: RasterCommand| {
            gate_rx.recv().unwrap(); // 重い raster 中…
            worker_count.fetch_add(1, SeqCst);
        });

        rt.send(RasterCommand::Frame(handoff(&[1]))).unwrap();

        // UI スレッドは raster 完了を待たずに進める（入力処理を継続できる）。
        let mut ui_inputs_handled = 0;
        for _ in 0..5 {
            ui_inputs_handled += 1; // 入力イベント処理の代理
        }
        assert_eq!(ui_inputs_handled, 5);
        assert_eq!(
            processed.load(SeqCst),
            0,
            "重い raster 中も UI スレッドはブロックされず進む（raster 未完）"
        );

        // raster を完了させて畳む。
        gate_tx.send(()).unwrap();
        drop(rt);
        assert_eq!(processed.load(SeqCst), 1, "解放後に raster が完了する");
    }

    #[test]
    fn native_adapter_exposes_the_shared_pipeline_observation() {
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let rt = RasterThread::spawn(move |_command: RasterCommand| {
            started_tx.send(()).unwrap();
            release_rx.recv().unwrap();
        });

        rt.send(RasterCommand::Frame(handoff(&[1]))).unwrap();
        started_rx.recv().unwrap();

        let observation = rt.observation();
        assert_eq!(observation.accepted, 1);
        assert!(observation.active);
        assert_eq!(observation.pending, 0);

        release_tx.send(()).unwrap();
        drop(rt);
    }

    #[test]
    fn threaded_output_matches_single_threaded() {
        let frames = [
            handoff(&[3, 1, 2]),
            handoff(&[]),
            handoff(&[5]),
            handoff(&[4, 4, 2]),
        ];

        // シングルスレッド経路。
        let single: Vec<Vec<u64>> = frames.iter().map(rasterize).collect();

        // マルチスレッド経路：同じハンドオフを Raster スレッドへ流し、結果を順に集める。
        let (out_tx, out_rx) = mpsc::channel::<Vec<u64>>();
        let rt = RasterThread::spawn(move |command: RasterCommand| {
            let RasterCommand::Frame(handoff) = command else {
                panic!("this test sends frames only");
            };
            out_tx.send(rasterize(&handoff)).unwrap();
        });
        let mut threaded = Vec::new();
        for f in &frames {
            rt.send(RasterCommand::Frame(f.clone())).unwrap();
            threaded.push(out_rx.recv().unwrap());
        }
        drop(rt);

        assert_eq!(
            threaded, single,
            "スレッド分離時の出力はシングルスレッド時と同値（DrawOp parity）"
        );
    }

    // ── #635: surface ライフサイクルを Frame と同じチャネルに直列化する（ADR-0128）─────────────

    /// テスト用の Raster 側 sink：受け取った [`RasterCommand`] を文字列トレースへ記録し、
    /// 「present をスキップすべきか」を surface 状態（Lost/Ready）から判定する。実 backend の
    /// swapchain present と同じ状態機械を、GPU 無しでホスト固定する代理。
    #[derive(Default)]
    struct SurfaceTrace {
        events: Vec<String>,
        surface_ready: bool,
        presented_frames: usize,
    }

    fn drive(trace: &Arc<std::sync::Mutex<SurfaceTrace>>) -> impl FnMut(RasterCommand) {
        let trace = Arc::clone(trace);
        move |cmd| {
            let mut t = trace.lock().unwrap();
            match cmd {
                RasterCommand::Frame(h) => {
                    let dirty = {
                        let mut v: Vec<u64> = h
                            .topology
                            .content_changed()
                            .iter()
                            .map(|i| i.to_u64())
                            .collect();
                        v.sort_unstable();
                        v
                    };
                    // surface が生きているフレームだけ present（Lost 中は raster しても present skip）。
                    if t.surface_ready {
                        t.presented_frames += 1;
                        t.events.push(format!("present {dirty:?}"));
                    } else {
                        t.events.push(format!("skip {dirty:?}"));
                    }
                }
                RasterCommand::Resize { width, height, .. } => {
                    t.events.push(format!("resize {width}x{height}"));
                }
                RasterCommand::SurfaceLost => {
                    t.surface_ready = false;
                    t.events.push("lost".into());
                }
                RasterCommand::RebuildSurface => {
                    t.surface_ready = true;
                    t.events.push("rebuild".into());
                }
            }
        }
    }

    #[test]
    fn surface_lifecycle_and_frames_are_processed_in_order() {
        let trace = Arc::new(std::sync::Mutex::new(SurfaceTrace::default()));
        let rt = RasterThread::spawn(drive(&trace));

        rt.send(RasterCommand::RebuildSurface).unwrap();
        rt.send(RasterCommand::Frame(handoff(&[1]))).unwrap();
        rt.send(RasterCommand::Resize {
            width: 800,
            height: 600,
            content_scale: 1.0,
        })
        .unwrap();
        rt.send(RasterCommand::Frame(handoff(&[2]))).unwrap();
        drop(rt); // 送信済みメッセージを全部処理してから join。

        let t = trace.lock().unwrap();
        assert_eq!(
            t.events,
            vec!["rebuild", "present [1]", "resize 800x600", "present [2]"],
            "Frame と surface ライフサイクルは送信順どおり直列に処理される"
        );
    }

    #[test]
    fn frames_after_surface_lost_skip_present_until_rebuild() {
        // AC: surface 破棄（TerminateWindow）後のフレームは present をスキップし、再構築後に復帰する。
        let trace = Arc::new(std::sync::Mutex::new(SurfaceTrace::default()));
        let rt = RasterThread::spawn(drive(&trace));

        rt.send(RasterCommand::RebuildSurface).unwrap();
        rt.send(RasterCommand::Frame(handoff(&[1]))).unwrap(); // present
        rt.send(RasterCommand::SurfaceLost).unwrap();
        rt.send(RasterCommand::Frame(handoff(&[2]))).unwrap(); // skip（surface 無し）
        rt.send(RasterCommand::RebuildSurface).unwrap();
        rt.send(RasterCommand::Frame(handoff(&[3]))).unwrap(); // present（復帰）
        drop(rt);

        let t = trace.lock().unwrap();
        assert_eq!(
            t.presented_frames, 2,
            "present は surface 生存中の 2 フレームだけ"
        );
        assert_eq!(
            t.events,
            vec![
                "rebuild",
                "present [1]",
                "lost",
                "skip [2]",
                "rebuild",
                "present [3]"
            ],
        );
    }

    #[test]
    fn shutdown_drains_pending_messages_then_disconnects() {
        // AC: 安全に停止する——停止時に送信済みメッセージは処理され、以後の送信は Disconnected。
        let trace = Arc::new(std::sync::Mutex::new(SurfaceTrace::default()));
        let mut rt = RasterThread::spawn(drive(&trace));

        rt.send(RasterCommand::RebuildSurface).unwrap();
        rt.send(RasterCommand::Frame(handoff(&[1]))).unwrap();
        rt.shutdown(); // 送信済みを処理して join。

        assert_eq!(
            trace.lock().unwrap().presented_frames,
            1,
            "停止前の送信済みフレームは処理される"
        );
        assert_eq!(
            rt.send(RasterCommand::Frame(handoff(&[2]))),
            Err(RasterHandoffError::Disconnected),
            "停止後の送信は Disconnected"
        );
    }

    #[test]
    fn rebuild_after_shutdown_uses_a_fresh_thread() {
        // AC: reload（Torimi full reload）で Raster スレッドを安全に停止 → 再構築できる。
        let trace = Arc::new(std::sync::Mutex::new(SurfaceTrace::default()));
        let mut rt = RasterThread::spawn(drive(&trace));
        rt.send(RasterCommand::RebuildSurface).unwrap();
        rt.send(RasterCommand::Frame(handoff(&[1]))).unwrap();
        rt.shutdown();

        // 新しい Raster スレッドを立て直す（同じ sink トレースを共有）。
        let rt2 = RasterThread::spawn(drive(&trace));
        rt2.send(RasterCommand::RebuildSurface).unwrap();
        rt2.send(RasterCommand::Frame(handoff(&[9]))).unwrap();
        drop(rt2);

        let t = trace.lock().unwrap();
        assert_eq!(
            t.presented_frames, 2,
            "停止前 1 + 再構築後 1 の計 2 フレームが present される"
        );
        assert!(
            t.events.contains(&"present [9]".to_string()),
            "再構築後のフレームが処理される"
        );
    }

    #[test]
    fn raster_command_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<RasterCommand>();
        assert_send::<RasterHandoff>();
    }

    #[test]
    fn committed_frame_becomes_one_immutable_owned_handoff() {
        let mut tree = ElementTree::new();
        let root = tree.element_create(0, ElementKind::View);
        tree.set_root(root);

        let frame = tree.commit_rendered_frame(0.0);
        let snapshot = RasterHandoff::from_committed_frame(&frame);
        assert_eq!(
            snapshot.topology.paint_order(),
            frame.layer_topology().paint_order()
        );
        assert_eq!(
            snapshot.topology.raster_bounds(),
            frame.layer_topology().raster_bounds()
        );
        assert_eq!(
            snapshot.topology.content_changed(),
            frame.layer_topology().content_changed()
        );
        assert_eq!(
            snapshot.topology.chrome_changed(),
            frame.layer_topology().chrome_changed()
        );
        assert_eq!(
            snapshot.topology.transform_changed(),
            frame.layer_topology().transform_changed()
        );
        assert_eq!(snapshot.scroll_inputs, frame.scroll_inputs());
        let committed_node_count = snapshot.scene.len();
        drop(frame);

        let child = tree.element_create(1, ElementKind::View);
        tree.element_append_child(root, child);
        let _next = tree.render(16.0);

        assert_eq!(
            snapshot.scene.len(),
            committed_node_count,
            "the UI thread's next-frame mutation must not alter the Raster snapshot"
        );
        assert!(tree.scene_graph().len() > snapshot.scene.len());
    }

    #[test]
    fn renderer_failure_is_terminal_and_never_retried_by_the_scheduler() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let sink_attempts = Arc::clone(&attempts);
        let rt = RasterThread::spawn_fallible(move |_command: RasterCommand| {
            sink_attempts.fetch_add(1, SeqCst);
            Err::<(), _>("selected renderer failed")
        });

        rt.send(RasterCommand::Frame(handoff(&[1]))).unwrap();
        while !rt.has_terminal_failure() {
            std::thread::yield_now();
        }

        assert_eq!(
            rt.send(RasterCommand::Frame(handoff(&[2]))),
            Err(RasterHandoffError::TerminalFailure)
        );
        drop(rt);
        assert_eq!(
            attempts.load(SeqCst),
            1,
            "runtime failure must not trigger renderer retry, fallback, or restart"
        );
    }
}
