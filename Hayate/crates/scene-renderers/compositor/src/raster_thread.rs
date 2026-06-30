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
/// SceneGraph と全レイヤ（描画順）・`layer_dirty` を owned で渡す。`Send + Sync` 境界。
pub struct RasterHandoff {
    /// 本フレームの lower 済み SceneGraph（owned スナップショット。境界を越えて move する）。
    pub scene: SceneGraph,
    /// 全 compositing layer（描画順 = ADR-0021）。
    pub layers: Vec<ElementId>,
    /// 本フレームで再 raster すべきレイヤ（#609 の `layer_dirty`）。
    pub layer_dirty: HashSet<ElementId>,
}

/// ハンドオフ失敗（Raster スレッドが既に終了している）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RasterHandoffError {
    Disconnected,
}

/// 専用 Raster スレッド。UI スレッド（core＝単一スレッドのまま）が produce した [`RasterHandoff`] を
/// 受けて raster/composite する。`sink` は cache+compositor（#610 の `Send` クリーン seam）を所有し、
/// Raster スレッド上だけで触る。core 自体はマルチスレッド化しない（ADR-0003 維持）。
pub struct RasterThread {
    sender: Option<Sender<RasterHandoff>>,
    handle: Option<JoinHandle<()>>,
}

impl RasterThread {
    /// 各ハンドオフを `sink` で処理する Raster スレッドを起動する。
    pub fn spawn<S>(mut sink: S) -> Self
    where
        S: FnMut(RasterHandoff) + Send + 'static,
    {
        let (sender, receiver): (Sender<RasterHandoff>, Receiver<RasterHandoff>) = mpsc::channel();
        let handle = thread::spawn(move || {
            // sender が全て drop されると recv が Err になりループを抜ける（綺麗な終了）。
            while let Ok(handoff) = receiver.recv() {
                sink(handoff);
            }
        });
        Self {
            sender: Some(sender),
            handle: Some(handle),
        }
    }

    /// UI スレッドからハンドオフを渡す（非ブロッキング）。raster 完了を待たずに返るので、UI スレッドは
    /// 続けて入力処理・次フレーム生成ができる（重い raster が入力を止めない・ADR-0128）。
    pub fn handoff(&self, handoff: RasterHandoff) -> Result<(), RasterHandoffError> {
        match self.sender.as_ref() {
            Some(sender) => sender
                .send(handoff)
                .map_err(|_| RasterHandoffError::Disconnected),
            None => Err(RasterHandoffError::Disconnected),
        }
    }
}

impl Drop for RasterThread {
    fn drop(&mut self) {
        // sender を先に drop して recv を切り、ワーカーを抜けさせてから join する（join 先行は deadlock）。
        self.sender.take();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
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
        let rt = RasterThread::spawn(move |_h| {
            *captured.lock().unwrap() = Some(thread::current().id());
        });
        rt.handoff(handoff(&[1])).unwrap();
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
        let rt = RasterThread::spawn(move |_h| {
            gate_rx.recv().unwrap(); // 重い raster 中…
            worker_count.fetch_add(1, SeqCst);
        });

        rt.handoff(handoff(&[1])).unwrap();

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
        let rt = RasterThread::spawn(move |h| {
            out_tx.send(rasterize(&h)).unwrap();
        });
        for f in &frames {
            rt.handoff(handoff(
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
}
