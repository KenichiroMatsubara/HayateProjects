//! native の UI/Raster 二スレッド分離（ADR-0128）。
//!
//! core（tree / layout / scene_build / lowering）は ADR-0003 どおり **単一スレッドのまま「UI
//! スレッド」**に留め、#610 で `Send` クリーンな seam の裏に隔離したレイヤキャッシュ＋compositor
//! （Vello raster + wgpu compositor）だけを専用 **Raster スレッド**へ移す（Flutter 同型）。
//!
//! **スレッド境界＝[`RasterHandoff`]（lower 済み SceneGraph ＋ `layer_dirty`）の受け渡し**で、
//! UI スレッドが produce、Raster スレッドが consume する。`Send + Sync` 境界はこのハンドオフに引く。
//! ハンドオフは非ブロッキング channel なので、重い raster が UI スレッドの入力処理を止めない。
//!
//! 実 Vello/wgpu の raster/composite は [`RasterThread::spawn`] に渡す sink が担う（native backend が
//! cache+compositor を所有して Raster スレッド上だけで触る）。本モジュールはスレッドモデルと境界型を
//! host で固定し、出力がシングルスレッド時と同値であることをテストする。

use std::collections::HashSet;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use hayate_core::element::id::ElementId;
use hayate_core::SceneGraph;

/// UI スレッド → Raster スレッドのハンドオフ（ADR-0128）。スレッド境界はこれ 1 つで、lower 済み
/// SceneGraph と全レイヤ（描画順）・`layer_dirty`・`chrome_dirty` を owned で渡す。`Send + Sync` 境界。
pub struct RasterHandoff {
    /// 本フレームの lower 済み SceneGraph（owned スナップショット。境界を越えて move する）。
    pub scene: SceneGraph,
    /// 全 compositing layer（描画順 = ADR-0021）。
    pub layers: Vec<ElementId>,
    /// 本フレームで再 raster すべきレイヤ（#609 の `layer_dirty`）。
    pub layer_dirty: HashSet<ElementId>,
    /// scroll フレームの chrome dirty（#634）。単一 texture 経路では content と union して扱う。
    pub chrome_dirty: HashSet<ElementId>,
}

/// UI スレッド → Raster スレッドのメッセージ（ADR-0128）。フレーム提示だけでなく surface ライフ
/// サイクル（resize / 破棄 / 再作成）も同じ順序付きチャネルで渡し、Raster スレッドが surface と
/// swapchain present を所有する（present をまたぐ順序が壊れないよう 1 本のチャネルに直列化する）。
/// surface ハンドルは backend 固有（Android の `ANativeWindow` 等）なので、再作成は sink 側が握る
/// factory を起動する [`RasterCommand::RebuildSurface`] で表す（型としては unit を運ぶ）。
pub enum RasterCommand {
    /// 1 フレームを raster/composite して present する。
    Frame(RasterHandoff),
    /// surface サイズ変更（swapchain 再構成＋レイヤ texture invalidate）。
    Resize { width: u32, height: u32 },
    /// surface が失われた（Android TerminateWindow）。以後の Frame は present をスキップする。
    SurfaceLost,
    /// surface を再構築する（新規作成 / Miharashi full reload）。sink が握る factory を起動する。
    RebuildSurface,
}

/// ハンドオフ失敗（Raster スレッドが既に終了している）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RasterHandoffError {
    Disconnected,
}

/// 専用 Raster スレッド。UI スレッド（core＝単一スレッドのまま）が produce したメッセージ `M`
/// （典型は [`RasterCommand`]）を受けて raster/composite する。`sink` は cache+compositor
/// （#610 の `Send` クリーン seam）と surface を所有し、Raster スレッド上だけで触る。core 自体は
/// マルチスレッド化しない（ADR-0003 維持）。メッセージは 1 本の channel に直列化されるので、
/// Frame と surface ライフサイクル（resize / lost / rebuild）の相対順序が保たれる。
pub struct RasterThread<M = RasterCommand>
where
    M: Send + 'static,
{
    sender: Option<Sender<M>>,
    handle: Option<JoinHandle<()>>,
}

impl<M> RasterThread<M>
where
    M: Send + 'static,
{
    /// 各メッセージを `sink` で処理する Raster スレッドを起動する。
    pub fn spawn<S>(mut sink: S) -> Self
    where
        S: FnMut(M) + Send + 'static,
    {
        let (sender, receiver): (Sender<M>, Receiver<M>) = mpsc::channel();
        let handle = thread::spawn(move || {
            // sender が全て drop されると recv が Err になりループを抜ける（綺麗な終了）。
            while let Ok(message) = receiver.recv() {
                sink(message);
            }
        });
        Self {
            sender: Some(sender),
            handle: Some(handle),
        }
    }

    /// UI スレッドからメッセージを渡す（非ブロッキング）。raster 完了を待たずに返るので、UI スレッドは
    /// 続けて入力処理・次フレーム生成ができる（重い raster が入力を止めない・ADR-0128）。
    pub fn send(&self, message: M) -> Result<(), RasterHandoffError> {
        match self.sender.as_ref() {
            Some(sender) => sender
                .send(message)
                .map_err(|_| RasterHandoffError::Disconnected),
            None => Err(RasterHandoffError::Disconnected),
        }
    }

    /// Raster スレッドを停止して join する（surface 破棄 / reload で明示的に畳むとき）。以後の
    /// [`send`](Self::send) は `Disconnected` を返す。Drop でも同じ手順で畳まれる。
    pub fn shutdown(&mut self) {
        self.sender.take();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl<M> Drop for RasterThread<M>
where
    M: Send + 'static,
{
    fn drop(&mut self) {
        // sender を先に drop して recv を切り、ワーカーを抜けさせてから join する（join 先行は deadlock）。
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
    use std::sync::Arc;

    fn id(raw: u64) -> ElementId {
        ElementId::from_u64(raw)
    }

    fn handoff(dirty: &[u64]) -> RasterHandoff {
        RasterHandoff {
            scene: SceneGraph::new(),
            layers: dirty.iter().map(|&r| id(r)).collect(),
            layer_dirty: dirty.iter().map(|&r| id(r)).collect(),
            chrome_dirty: HashSet::new(),
        }
    }

    /// 決定的な「描画結果」：dirty レイヤ id を昇順に。シングル/マルチスレッドの parity 比較に使う。
    fn rasterize(h: &RasterHandoff) -> Vec<u64> {
        let mut v: Vec<u64> = h.layer_dirty.iter().map(|i| i.to_u64()).collect();
        v.sort_unstable();
        v
    }

    #[test]
    fn scene_graph_handoff_is_send_and_sync() {
        // ADR-0128: スレッド境界＝SceneGraph ハンドオフが Send + Sync で成立する。
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<SceneGraph>();
        assert_send_sync::<RasterHandoff>();
    }

    #[test]
    fn raster_runs_on_a_separate_thread_from_the_ui_thread() {
        let ui_thread = thread::current().id();
        let raster_thread = Arc::new(std::sync::Mutex::new(None));
        let captured = Arc::clone(&raster_thread);
        let rt = RasterThread::spawn(move |_h: RasterHandoff| {
            *captured.lock().unwrap() = Some(thread::current().id());
        });
        rt.send(handoff(&[1])).unwrap();
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
        let rt = RasterThread::spawn(move |_h: RasterHandoff| {
            gate_rx.recv().unwrap(); // 重い raster 中…
            worker_count.fetch_add(1, SeqCst);
        });

        rt.send(handoff(&[1])).unwrap();

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
        let rt = RasterThread::spawn(move |h: RasterHandoff| {
            out_tx.send(rasterize(&h)).unwrap();
        });
        for f in &frames {
            rt.send(handoff(
                &f.layer_dirty.iter().map(|i| i.to_u64()).collect::<Vec<_>>(),
            ))
            .unwrap();
        }
        let mut threaded = Vec::new();
        for _ in 0..frames.len() {
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
                        let mut v: Vec<u64> = h.layer_dirty.iter().map(|i| i.to_u64()).collect();
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
                RasterCommand::Resize { width, height } => {
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
        rt.send(RasterCommand::Resize { width: 800, height: 600 }).unwrap();
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
        assert_eq!(t.presented_frames, 2, "present は surface 生存中の 2 フレームだけ");
        assert_eq!(
            t.events,
            vec!["rebuild", "present [1]", "lost", "skip [2]", "rebuild", "present [3]"],
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

        assert_eq!(trace.lock().unwrap().presented_frames, 1, "停止前の送信済みフレームは処理される");
        assert_eq!(
            rt.send(RasterCommand::Frame(handoff(&[2]))),
            Err(RasterHandoffError::Disconnected),
            "停止後の送信は Disconnected"
        );
    }

    #[test]
    fn rebuild_after_shutdown_uses_a_fresh_thread() {
        // AC: reload（Miharashi full reload）で Raster スレッドを安全に停止 → 再構築できる。
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
        assert_eq!(t.presented_frames, 2, "停止前 1 + 再構築後 1 の計 2 フレームが present される");
        assert!(t.events.contains(&"present [9]".to_string()), "再構築後のフレームが処理される");
    }

    #[test]
    fn raster_command_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<RasterCommand>();
        assert_send::<RasterHandoff>();
    }
}
