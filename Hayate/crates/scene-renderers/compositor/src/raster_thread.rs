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
//! キューは無制限だが FIFO ではない——[`Coalesce`] が「まだ未処理の直近メッセージへ、新着を
//! 安全に合成してよいか」を決める。`Frame` は連続する限り最新の SceneGraph へ差し替えつつ、
//! `layer_dirty`/`chrome_dirty` は union で畳む（古いスクロールオフセットを積み上げて後から
//! 秒単位で「再生」しない一方、合成で消えた古いフレームが持っていた「このレイヤは要 raster」
//! という情報は失わない——上書きにすると #680 と同系統の「穴あきキャッシュが直らず要素が
//! 消えたまま」を raster バックログ側でも再現してしまう）。Resize/SurfaceLost/RebuildSurface の
//! ようなライフサイクル境界は絶対に合成しない——`Coalesce` は直前と直後が両方 `Frame` のときだけ
//! 合成し、境界を挟んだ相対順序はこれまで通り保たれる（元は #635 の単純な mpsc FIFO だったが、
//! raster が入力より遅くなった瞬間に無制限バックログとして溜まり、表示が数秒遅れて「追いつく」
//! 形で見えていた——診断の詳細は該当 issue のコミットメッセージ参照）。
//!
//! 実 Vello/wgpu の raster/composite は [`RasterThread::spawn`] に渡す sink が担う（native backend が
//! cache+compositor を所有して Raster スレッド上だけで触る）。本モジュールはスレッドモデルと境界型を
//! host で固定し、出力がシングルスレッド時と同値であることをテストする。

use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};

use hayate_core::element::id::ElementId;
use hayate_core::{SceneGraph, ScrollCompositorInput};

/// UI スレッド → Raster スレッドのハンドオフ（ADR-0128）。スレッド境界はこれ 1 つで、lower 済み
/// SceneGraph と全レイヤ（描画順）・`layer_dirty`・`chrome_dirty` を owned で渡す。`Send + Sync` 境界。
pub struct RasterHandoff {
    /// 本フレームの lower 済み SceneGraph（owned スナップショット。境界を越えて move する）。
    pub scene: SceneGraph,
    /// 全 compositing layer（描画順 = ADR-0021）。
    pub layers: Vec<ElementId>,
    /// 本フレームで再 raster すべきレイヤ（#609 の `layer_dirty`）。
    pub layer_dirty: HashSet<ElementId>,
    /// transform 係数だけが変わったレイヤ（#633）。単一 root 経路は per-layer quad 合成を
    /// 持たないため、content dirty と union して保守的に raster トリガへ含める（#687）。
    pub transform_dirty: HashSet<ElementId>,
    /// scroll フレームの chrome dirty（#634）。単一 texture 経路では content と union して扱う。
    pub chrome_dirty: HashSet<ElementId>,
    /// Core が commit 時に捕捉した scroll facts。Raster スレッド側で overscan geometry へ射影する。
    pub scroll_inputs: Vec<ScrollCompositorInput>,
}

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

impl Coalesce for RasterCommand {
    fn merge(&mut self, incoming: Self) -> Result<(), Self> {
        // Frame が連続するときだけ合成してよい（Resize/SurfaceLost/RebuildSurface を跨いだ
        // 相対順序は壊せない——surface ライフサイクルの正しさが崩れる）。SceneGraph/layers は
        // 最新のものに差し替えるが、`layer_dirty`/`chrome_dirty` は union する——上書きすると、
        // 合成されて消えた古い方が持っていた「このレイヤは穴あきキャッシュを直すため要
        // raster」という情報が失われ、raster が詰まっている間にマークされた dirty が
        // そのまま失われて要素が画面から消えたまま戻らなくなる（#680 と同系統の退行）。
        match (&mut *self, incoming) {
            (RasterCommand::Frame(existing), RasterCommand::Frame(new)) => {
                existing.scene = new.scene;
                existing.layers = new.layers;
                existing.layer_dirty.extend(new.layer_dirty);
                existing.transform_dirty.extend(new.transform_dirty);
                existing.chrome_dirty.extend(new.chrome_dirty);
                existing.scroll_inputs = new.scroll_inputs;
                for input in &mut existing.scroll_inputs {
                    input.content_dirty |= existing.layer_dirty.contains(&input.layer);
                }
                Ok(())
            }
            (_, incoming) => Err(incoming),
        }
    }
}

/// ハンドオフ失敗（Raster スレッドが既に終了している）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RasterHandoffError {
    Disconnected,
}

/// `M` の新着メッセージを、まだ raster スレッドに拾われていない直近のキュー末尾へ安全に
/// 合成してよいか（＝古い方を単に捨てるのではなく、両者の情報を失わず 1 件へ畳めるか）を決める。
///
/// 合成できるときは `self` を新しい状態へ更新して `Ok(())` を返す（キューには積み増さない）。
/// 合成できないときは `incoming` をそのまま `Err` で返す（呼び出し側がキューへ積む）。常に
/// `Err(incoming)` を返す実装は無制限 FIFO と同値（旧来の挙動）。
///
/// **正しさの要件**: 合成後の 1 件は、合成前の 2 件を順番に処理したのと観測可能な結果が
/// 一致しなければならない。例えば [`RasterCommand::Frame`] は最新の SceneGraph へ差し替える
/// だけでなく、`layer_dirty`/`chrome_dirty` を union する——ここを上書きにすると、消えた古い
/// 方が持っていた「このレイヤは要 raster」という情報が失われ、詰まっている間にマークされた
/// dirty がそのまま失われて要素が画面から消えたまま戻らなくなる（#680 と同系統の退行）。
pub trait Coalesce: Sized {
    fn merge(&mut self, incoming: Self) -> Result<(), Self>;
}

impl Coalesce for RasterHandoff {
    fn merge(&mut self, incoming: Self) -> Result<(), Self> {
        Err(incoming)
    }
}

struct QueueState<M> {
    queue: VecDeque<M>,
    closed: bool,
}

struct Shared<M> {
    state: Mutex<QueueState<M>>,
    condvar: Condvar,
}

/// 専用 Raster スレッド。UI スレッド（core＝単一スレッドのまま）が produce したメッセージ `M`
/// （典型は [`RasterCommand`]）を受けて raster/composite する。`sink` は cache+compositor
/// （#610 の `Send` クリーン seam）と surface を所有し、Raster スレッド上だけで触る。core 自体は
/// マルチスレッド化しない（ADR-0003 維持）。キューは [`Coalesce`] が許す範囲でのみ合成され、それ以外の
/// 相対順序（Frame と surface ライフサイクルの順序を含む）は 1 本のキューに直列化されたまま保たれる。
pub struct RasterThread<M = RasterCommand>
where
    M: Send + 'static,
{
    shared: Option<Arc<Shared<M>>>,
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
        let shared = Arc::new(Shared {
            state: Mutex::new(QueueState {
                queue: VecDeque::new(),
                closed: false,
            }),
            condvar: Condvar::new(),
        });
        let worker_shared = Arc::clone(&shared);
        let handle = thread::spawn(move || loop {
            let message = {
                let mut state = worker_shared.state.lock().unwrap();
                loop {
                    if let Some(message) = state.queue.pop_front() {
                        break Some(message);
                    }
                    if state.closed {
                        break None;
                    }
                    state = worker_shared.condvar.wait(state).unwrap();
                }
            };
            match message {
                Some(message) => sink(message),
                // closed かつキューが空＝送信側が全て drop された（綺麗な終了）。
                None => break,
            }
        });
        Self {
            shared: Some(shared),
            handle: Some(handle),
        }
    }

    /// UI スレッドからメッセージを渡す（非ブロッキング）。raster 完了を待たずに返るので、UI スレッドは
    /// 続けて入力処理・次フレーム生成ができる（重い raster が入力を止めない・ADR-0128）。
    ///
    /// キュー末尾がまだ raster に拾われておらず、かつ `M::merge` が合成に成功したときは、
    /// 積み増さずにその場で合成する——raster が詰まっても「stale なフレームの山を後から順に
    /// 再生して数秒遅れて追いつく」のではなく、詰まりが解けた瞬間に最新の状態へ飛ぶ。合成は
    /// 両者の情報を失わず 1 件へ畳む（`Coalesce` の正しさの要件）ので、キューが短くなっても
    /// dirty 情報が失われることはない。
    pub fn send(&self, message: M) -> Result<(), RasterHandoffError>
    where
        M: Coalesce,
    {
        let Some(shared) = self.shared.as_ref() else {
            return Err(RasterHandoffError::Disconnected);
        };
        let mut state = shared
            .state
            .lock()
            .map_err(|_| RasterHandoffError::Disconnected)?;
        if state.closed {
            return Err(RasterHandoffError::Disconnected);
        }
        let mut message = message;
        if let Some(last) = state.queue.back_mut() {
            match last.merge(message) {
                Ok(()) => {
                    shared.condvar.notify_one();
                    return Ok(());
                }
                Err(returned) => message = returned,
            }
        }
        state.queue.push_back(message);
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

impl<M> Drop for RasterThread<M>
where
    M: Send + 'static,
{
    fn drop(&mut self) {
        // closed を先に立てて worker がキュー drain 後に抜けられるようにしてから join する
        // （join 先行は deadlock）。
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
    use std::sync::{mpsc, Arc};

    fn id(raw: u64) -> ElementId {
        ElementId::from_u64(raw)
    }

    fn handoff(dirty: &[u64]) -> RasterHandoff {
        RasterHandoff {
            scene: SceneGraph::new(),
            layers: dirty.iter().map(|&r| id(r)).collect(),
            layer_dirty: dirty.iter().map(|&r| id(r)).collect(),
            transform_dirty: HashSet::new(),
            chrome_dirty: HashSet::new(),
            scroll_inputs: Vec::new(),
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
        rt.send(RasterCommand::Resize { width: 800, height: 600, content_scale: 1.0 }).unwrap();
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
        assert_eq!(t.presented_frames, 2, "停止前 1 + 再構築後 1 の計 2 フレームが present される");
        assert!(t.events.contains(&"present [9]".to_string()), "再構築後のフレームが処理される");
    }

    #[test]
    fn raster_command_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<RasterCommand>();
        assert_send::<RasterHandoff>();
    }

    #[test]
    fn merging_frames_unions_transform_dirty_instead_of_overwriting() {
        // #687: transform_dirty は layer_dirty/chrome_dirty と同じ穴あきキャッシュ問題を持つ
        // （単一 root 経路の raster トリガに union される、canvas.rs 参照）。コアレス時に
        // 上書きすると、合成で消えた古いフレームの「transform だけ変わった」情報が失われる。
        let mut a = RasterCommand::Frame(handoff_with_transform_dirty(&[1]));
        let b = RasterCommand::Frame(handoff_with_transform_dirty(&[2]));
        if a.merge(b).is_err() {
            panic!("consecutive Frame commands must coalesce");
        }

        let RasterCommand::Frame(merged) = a else {
            panic!("merge must keep the command a Frame");
        };
        let mut ids: Vec<u64> = merged.transform_dirty.iter().map(|i| i.to_u64()).collect();
        ids.sort_unstable();
        assert_eq!(
            ids,
            vec![1, 2],
            "transform_dirty must union across coalesced frames, not overwrite"
        );
    }

    fn handoff_with_transform_dirty(dirty: &[u64]) -> RasterHandoff {
        let mut h = handoff(&[]);
        h.transform_dirty = dirty.iter().map(|&r| id(r)).collect();
        h
    }

    #[test]
    fn merging_frames_keeps_old_scroll_content_dirty_on_the_latest_geometry() {
        let mut old = handoff(&[7]);
        old.scroll_inputs.push(ScrollCompositorInput {
            layer: id(7),
            absolute_top: 0.0,
            viewport_height: 100.0,
            scroll_offset: 0.0,
            max_scroll_offset: 500.0,
            content_dirty: true,
        });
        let mut latest = handoff(&[]);
        latest.scroll_inputs.push(ScrollCompositorInput {
            layer: id(7),
            absolute_top: 0.0,
            viewport_height: 100.0,
            scroll_offset: 40.0,
            max_scroll_offset: 500.0,
            content_dirty: false,
        });

        let mut merged = RasterCommand::Frame(old);
        assert!(merged.merge(RasterCommand::Frame(latest)).is_ok());
        let RasterCommand::Frame(merged) = merged else { unreachable!() };
        assert_eq!(merged.scroll_inputs[0].scroll_offset, 40.0);
        assert!(merged.scroll_inputs[0].content_dirty);
    }

    // ── backlog coalescing（raster が入力より遅くなったときの挙動）────────────────────────

    #[test]
    fn slow_raster_drops_backlog_instead_of_replaying_it() {
        // AC: raster が詰まっている間に大量の Frame が届いても、詰まりが解けたとき無制限 FIFO の
        // ように全件を順に再生してはいけない——1 件に一本化され、表示は「今」へ飛ぶ。旧実装
        // （単純な mpsc）だとこのテストは 52 件（gate 分 1 + 送信 51 件）を全部 present し、表示が
        // 実入力から数秒分遅れて「追いつく」形で見える不具合を再現してしまう。
        //
        // 一本化は SceneGraph/layers を最新のものへ差し替えるだけでなく、`layer_dirty` を
        // union しなければならない——合成で消えた古いフレームの dirty を単純上書きで捨てると、
        // そのフレームでしかマークされなかった「このレイヤは穴あきキャッシュを直すため要
        // raster」という情報が失われ、#680 と同系統の「要素が消えたまま戻らない」を raster
        // バックログ側で再現してしまう。
        let gate_hit = Arc::new((std::sync::Mutex::new(false), std::sync::Condvar::new()));
        let gate_for_sink = Arc::clone(&gate_hit);
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let release_rx = std::sync::Mutex::new(release_rx);
        let processed: Arc<std::sync::Mutex<Vec<Vec<u64>>>> =
            Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink_processed = Arc::clone(&processed);
        let mut first = true;

        let rt = RasterThread::spawn(move |cmd: RasterCommand| {
            if let RasterCommand::Frame(h) = &cmd {
                let v = rasterize(h);
                if first {
                    first = false;
                    // raster が最初の 1 件を掴んだことをテストスレッドへ知らせ、テストスレッドが
                    // 「詰まっている間の」後続送信を全部終えるまでここで足止めする。
                    let (lock, cvar) = &*gate_for_sink;
                    *lock.lock().unwrap() = true;
                    cvar.notify_all();
                    let _ = release_rx.lock().unwrap().recv();
                }
                sink_processed.lock().unwrap().push(v);
            }
        });

        rt.send(RasterCommand::Frame(handoff(&[0]))).unwrap();

        // raster が最初の 1 件を掴んでブロックするまで待つ。
        {
            let (lock, cvar) = &*gate_hit;
            let mut hit = lock.lock().unwrap();
            while !*hit {
                hit = cvar.wait(hit).unwrap();
            }
        }

        // raster が詰まっている間に、後続フレームを大量に送る（1 件ずつ別々の layer を dirty に）。
        for i in 1..=50u64 {
            rt.send(RasterCommand::Frame(handoff(&[i]))).unwrap();
        }

        release_tx.send(()).unwrap();
        drop(rt); // 残り（合成後の最終 1 件）を drain させてから join。

        let done = processed.lock().unwrap();
        assert_eq!(
            done.len(),
            2,
            "raster 呼び出しは「詰まっていた最初の 1 件」+「合成された最終 1 件」の 2 回だけ\
             （51 件を順に再生する退行を防ぐ）"
        );
        assert_eq!(done[0], vec![0], "1 件目はゲートで足止めした最初の送信そのまま");
        assert_eq!(
            done[1],
            (1..=50).collect::<Vec<u64>>(),
            "合成された 1 件の layer_dirty は詰まっている間に届いた 50 件全ての union のはず\
             ——上書きで一部が失われていない"
        );
    }
}
