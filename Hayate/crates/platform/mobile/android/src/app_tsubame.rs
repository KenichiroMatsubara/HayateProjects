//! JS 駆動の描画 + 入力ループ（ADR-0112, feature=tsubame-js）。**device 未検証**。
//!
//! 既存の `app::android_main`（デモツリー直挿し）を非破壊で温存したまま、こちらは
//! ネイティブ Hayate を `__hayateHost` として Hermes に注入し、dev-server から**実行時
//! ネットワーク fetch** した Tsubame バンドルを eval して描画する（Miharashi, #532）。源は
//! `bundle_source` に切り出してホストで契約テストする。eval シームは不変。
//!
//! #533 で full reload と protocol version 整合を移植した（Web #529/#530 と対称）:
//!   - boot（fetch → ランタイム構築/eval → version 突き合わせ）は `miharashi_reload::boot_runtime`。
//!     不一致／取得失敗はランタイムの pump に進めず明示エラーにする（謎クラッシュ回避）。
//!   - dev-server の WS `reload` を `miharashi_reload::subscribe_reload`（device connect は
//!     `reload_socket`）で購読し、受信ごとに **Hermes ランタイムを作り直して再 eval** する
//!     （full reload。tree も作り直すので state は飛ぶ）。
//!   - 突き合わせの純ロジックと再構築の orchestration はホストで契約テスト済み。実描画と reload
//!     体験はローカル実機で検証する（本 issue 外）。
//!
//! 1 フレームの順序（ADR-0112）:
//!   1. 入力: タッチ/IME は native→tree 直結（既存 `app::process_touch_input` /
//!      `app::sync_ime` を共有）。JS は経由しない。
//!   2. JS フレーム: `HermesApp::pump_frame(ts)` が `__tsubame.pumpFrame` を呼び、
//!      バンドル内の CanvasRenderer が flush(apply_mutations)→render→poll_events を
//!      回す。`render` は host 経由で `tree.render`（レイアウト + 保持シーン lower）。
//!   3. present: ここで tree を再 render してシーンを取り出し GPU 提示する。
//!
//! tree は `Rc<RefCell<ElementTree>>` で JS ホストと共有する（単一スレッド,
//! ADR-0003）。借用は各ステップで非重複（JS が返ってから present で借りる）。
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use android_activity::{AndroidApp, MainEvent, PollEvent};
use hayate_core::{ElementId, ElementTree};

use crate::app::{
    frame_handoff, init_gpu_surface, process_touch_input, spawn_raster_thread, sync_ime,
    RasterHandle,
};
use crate::touch_scroll::TouchScrollState;
use hayate_layer_compositor::RasterCommand;
use crate::bundle_source;
use crate::frame_schedule::OnDemandFrameLoop;
use crate::dev_server_target;
use crate::hermes_bridge::{make_bridge, new_hermes_app, HermesApp};
use crate::miharashi_reload::{
    boot_runtime, subscribe_reload, BootError, ReloadSocket, SubscribeReloadOptions,
};
use crate::reload_socket::{connect_reload_ws, ReloadWsSocket};
use hayate_core::element::ime_reconcile::TextInputState;
use crate::surface_lifecycle::{
    safe_window_dimensions, viewport_for_surface, window_dimensions, SurfaceLifecycleAction,
    SurfaceLifecycleState,
};

/// 1 boot 分の Hermes ランタイムと、それと共有する ElementTree。full reload では丸ごと作り直して
/// state を捨てる（CONTEXT.md「Reload」）。scroll-view 上のタッチドラッグ→スクロール
/// ジェスチャ（ADR-0082）は特定 tree の `ElementId` にロックされるので、tree と運命を
/// 共にする——reload で古い ID を握ったまま残らないよう、ここにも同じタイミングで
/// 作り直す。
struct Runtime {
    hermes: cxx::UniquePtr<HermesApp>,
    tree: Rc<RefCell<ElementTree>>,
    touch_scroll: TouchScrollState,
}

/// eval 済みバンドルが立てた protocol version（`__miharashiProtocolVersion`）を読み、`Option<u32>`
/// に正規化する。C++ 側は未埋め込み / 非数値を負値で返す（`@miharashi/protocol-handshake` の
/// `readBundleProtocolVersion` が `undefined` を返すのと同型）。
fn read_bundle_protocol_version(hermes: &cxx::UniquePtr<HermesApp>) -> Option<u32> {
    let version = hermes.protocol_version();
    if version.is_finite() && version >= 0.0 {
        Some(version as u32)
    } else {
        None
    }
}

/// boot 失敗をログと画面向けの読める文言にする（mount もクラッシュもさせない・#530）。不一致は
/// 両版数を、取得失敗はその種別を出す。pump には進めない（current=None のまま）。返した文言は
/// 呼び出し側が `error_overlay::show_error` でそのまま画面に描画する——Web ホストの built-in
/// error panel と対称に、consumer（アプリ側）の実装にも Hayate/GPU パイプラインにも依存せず
/// 画面に出す保証（Hayate 自身の初期化が壊れていても呼べるネイティブ View オーバーレイ）。
fn report_boot_error(error: &BootError) -> String {
    let message = match error {
        BootError::ProtocolMismatch(mismatch) => format!(
            "Miharashi: protocol version 不一致のため mount しません — {}（host v{}, bundle {:?}）",
            mismatch.message, mismatch.host_version, mismatch.bundle_version,
        ),
        BootError::Fetch(err) => format!(
            "Miharashi: dev-server からのバンドル取得に失敗（mount しません）: {err:?}"
        ),
    };
    log::error!("{message}");
    message
}

pub(crate) fn run(app: AndroidApp) {
    // このホスト（decoder）に焼き込んだ wire 版数。Web ホストの HOST_PROTOCOL_VERSION と同じ
    // source of truth（`@hayate/protocol-spec` の manifest version）をネイティブ decoder
    // （生成物）から取る（#530/#533 共有）。
    let host_version = hayate_core::wire::PROTOCOL_VERSION;

    // 接続先 dev-server：端末 UI（Kotlin の EditText）が internal data dir に書いた URL を読み戻して
    // 1 つの target に解決する（未入力 / 不正なら既定 = エミュレータ loopback、#534）。この target が
    // バンドル fetch（HTTP）と reload 購読（WS）の**両方**を駆動する＝同じ dev-server を指す（保持）。
    // 毎 boot/reload で読み直されるので、再接続でも入力値が効く（再接続）。
    let target = dev_server_target::resolve_entered(app.internal_data_path().as_deref());

    // 1 boot：dev-server からバンドルを取得 → Hermes ランタイムを構築（= eval。eval シームは
    // 不変 `new_hermes_app(make_bridge(tree.clone()), bundle)`）→ バンドルの protocol version を
    // 読みホスト版数と突き合わせる。一致時のみランタイムを返す（#532 の源 + #530 の突き合わせ）。
    // full reload は単にこれをもう一度呼ぶだけで、tree ごと作り直されて state が飛ぶ。
    let boot = || {
        boot_runtime(
            host_version,
            || bundle_source::fetch_from(&target),
            |bundle: &str| {
                let tree: Rc<RefCell<ElementTree>> = Rc::new(RefCell::new(ElementTree::new()));
                let hermes = new_hermes_app(make_bridge(tree.clone()), bundle);
                Runtime {
                    hermes,
                    tree,
                    touch_scroll: TouchScrollState::new(),
                }
            },
            |runtime: &Runtime| read_bundle_protocol_version(&runtime.hermes),
        )
    };

    // 現在駆動中のランタイム（boot 失敗 / 不一致のあいだは None で、pump せず明示エラーのまま回す）。
    let mut current: Option<Runtime> = match boot() {
        Ok(runtime) => Some(runtime),
        Err(error) => {
            crate::error_overlay::show_error(&report_boot_error(&error));
            None
        }
    };

    // 直近の viewport。reload で tree を作り直したら、新 tree に再適用して描画サイズを引き継ぐ。
    let mut last_viewport: Option<(f32, f32)> = None;

    // ── full reload 購読（#533）────────────────────────────────────────────────
    // WS `reload` 受信で reload フラグを立て、poll ループが拾って再 boot する。WS の blocking read は
    // 背景スレッド（reload_socket）が担い、main は毎フレーム pump() で排出する（単一スレッド契約）。
    let reload_requested = Rc::new(Cell::new(false));
    let reload_flag = Rc::clone(&reload_requested);
    // connect が張った具体 socket を main 用に保持する（pump 駆動のため。再接続で差し替わる）。
    let reload_socket_slot: Rc<RefCell<Option<Rc<ReloadWsSocket>>>> = Rc::new(RefCell::new(None));
    let slot_for_connect = Rc::clone(&reload_socket_slot);
    // backoff 再接続は main で起こす必要がある（open() が !Send な Rc/RefCell に触るため）。タイマー
    // スレッドではなく「期限つき保留」を main ループが拾って発火する。
    let pending_reconnect: Rc<RefCell<Option<(Box<dyn FnOnce()>, Instant)>>> =
        Rc::new(RefCell::new(None));
    let pending_for_schedule = Rc::clone(&pending_reconnect);

    let _reload_subscription = subscribe_reload(SubscribeReloadOptions {
        // バンドル fetch と同じ target を共有する＝同じ配信点を指す（https 由来なら wss・#742）。
        target: target.clone(),
        on_reload: Box::new(move || reload_flag.set(true)),
        connect: Box::new(move |url| {
            let socket = connect_reload_ws(url);
            *slot_for_connect.borrow_mut() = Some(Rc::clone(&socket));
            socket as Rc<dyn ReloadSocket>
        }),
        schedule_reconnect: Box::new(move |fire, delay| {
            *pending_for_schedule.borrow_mut() = Some((fire, Instant::now() + delay));
        }),
    });

    // 実機発音検証の最小導線（ADR-0117 / #562）。起動時に一度だけ、Core の `AudioOutput`
    // 契約越しに 440Hz のテストトーンを数百 ms 鳴らし、logcat に証跡を残す。発音バックエンドの
    // native 呼び出しはバッファが realtime に消費されるまでブロックし得るので、描画ループを
    // 止めないよう専用スレッドで鳴らす（出力の生成・駆動はスレッド内で完結＝Send 不要）。
    std::thread::spawn(|| {
        log::info!(
            "hayate-adapter-android: 起動テストトーン再生 — {}Hz を {}ms（AudioOutput 経由）",
            crate::test_tone::DEFAULT_FREQUENCY_HZ,
            crate::test_tone::TEST_TONE_DURATION_MS,
        );
        let mut output = crate::audio_output::AudioTrackOutput::default();
        crate::test_tone::play_test_tone(
            &mut output,
            hayate_core::AudioFormat::DEFAULT,
            crate::test_tone::TEST_TONE_DURATION_MS,
        );
        log::info!("hayate-adapter-android: 起動テストトーン再生 完了");
    });

    // #635: raster/composite は専用 Raster スレッド（ADR-0128）。UI スレッドは handoff を送るだけ。
    let mut raster: Option<RasterHandle> = None;
    let mut lifecycle = SurfaceLifecycleState::new();
    let start = Instant::now();

    // IME 状態（既存 sync_ime と共有）。
    let mut ime_state = TextInputState::default();
    let mut ime_target: Option<ElementId> = None;
    let mut ime_keyboard_shown = false;
    let mut quit = false;

    // ADR-0126: 無条件 pump を撤廃し on-demand 化する。冷間始動を要求した状態で開始し、以後は
    // wake（入力到着・lifecycle・reload）と継続（描画後に残る visual_dirty）のあるときだけ
    // pump+present する。idle ではフレームを 1 枚も出さない（`poll_events` の最大 16ms ブロックで
    // OS イベント待ちに落ちる）。
    let mut frame_loop = OnDemandFrameLoop::started();

    while !quit {
        // このイテレーションで lifecycle イベントを観測したか（surface 作成/破棄/resize は
        // いずれも再描画が要る wake 源）。closure からは frame_loop を可変借用できないので Cell 経由。
        let lifecycle_wake = Cell::new(false);
        app.poll_events(Some(Duration::from_millis(16)), |event| {
            if let PollEvent::Main(main_event) = event {
                lifecycle_wake.set(true);
                let lifecycle_event = match main_event {
                    MainEvent::InitWindow { .. } => {
                        Some(crate::surface_lifecycle::SurfaceLifecycleEvent::InitWindow)
                    }
                    MainEvent::TerminateWindow { .. } => {
                        Some(crate::surface_lifecycle::SurfaceLifecycleEvent::TerminateWindow)
                    }
                    MainEvent::WindowResized { .. } => app.native_window().map(|window| {
                        let (width, height) = window_dimensions(window.width(), window.height());
                        crate::surface_lifecycle::SurfaceLifecycleEvent::WindowResized {
                            width,
                            height,
                        }
                    }),
                    MainEvent::Destroy => {
                        Some(crate::surface_lifecycle::SurfaceLifecycleEvent::Destroy)
                    }
                    // システムUI（ステータスバー/ナビゲーションバー/ソフトキーボード）の実表示
                    // 領域が変わったときに届く（surface 自体の init/resize/destroy ではない）。
                    // 四論理イベントの state machine には乗せず、ここで直接ビューポートだけ
                    // 更新する（最下点固定ピクセルずれバグの修正）。
                    MainEvent::ContentRectChanged { .. } => {
                        if let Some(window) = app.native_window() {
                            let scale = crate::surface_lifecycle::content_scale(&app);
                            let (w, h) = window_dimensions(window.width(), window.height());
                            let rect = app.content_rect();
                            let (safe_w, safe_h) = safe_window_dimensions(
                                w, h, rect.left, rect.top, rect.right, rect.bottom,
                            );
                            let (vw, vh) = viewport_for_surface(safe_w, safe_h, scale);
                            last_viewport = Some((vw, vh));
                            if let Some(runtime) = current.as_ref() {
                                let mut tree = runtime.tree.borrow_mut();
                                tree.set_viewport(vw, vh);
                                // CreateSurface/ResizeSurface と同じ理由（下のコメント参照）で、
                                // viewport 変更を即座にレイアウトへ反映させるため明示的に render
                                // を起こす。
                                let _ = tree.render(start.elapsed().as_secs_f64() * 1000.0);
                            }
                        }
                        None
                    }
                    _ => None,
                };

                if let Some(event) = lifecycle_event {
                    match lifecycle.handle(event) {
                        SurfaceLifecycleAction::CreateSurface => {
                            if let Some(window) = app.native_window() {
                                let scale = crate::surface_lifecycle::content_scale(&app);
                                let (w, h) = window_dimensions(window.width(), window.height());
                                let rect = app.content_rect();
                                let (safe_w, safe_h) = safe_window_dimensions(
                                    w, h, rect.left, rect.top, rect.right, rect.bottom,
                                );
                                let (vw, vh) = viewport_for_surface(safe_w, safe_h, scale);
                                last_viewport = Some((vw, vh));
                                if let Some(runtime) = current.as_ref() {
                                    let mut tree = runtime.tree.borrow_mut();
                                    tree.set_viewport(vw, vh);
                                    // `set_viewport` はメトリクスを差し替えるだけでレイアウトは
                                    // 再計算しない。JS 駆動の on-demand フレームループ（ADR-0126）
                                    // は resize を関知しないため（issue #475: resize は native→tree
                                    // 直結で JS 経路から外れている）、ここで明示的に `render` を
                                    // 起こさないと直近の pumpFrame が焼き込んだ古いビューポートの
                                    // レイアウトのまま固まる（起動直後は Hermes 側のデフォルト viewport
                                    // で 1 回 render 済みのため、実サイズ確定前に描いた小さいレイアウトが
                                    // 永久に残る）。
                                    let _ = tree.render(start.elapsed().as_secs_f64() * 1000.0);
                                }
                                match pollster::block_on(init_gpu_surface(&window, scale)) {
                                    // 生成した surface を Raster スレッドへ move（#635）。
                                    Ok(surface) => raster = Some(spawn_raster_thread(surface)),
                                    Err(err) => log::error!(
                                        "hayate-adapter-android: GPU init failed: {err}"
                                    ),
                                }
                            }
                        }
                        // surface 破棄：Raster スレッドを drop → 送信済みを処理して join（安全停止）。
                        SurfaceLifecycleAction::DestroySurface => raster = None,
                        SurfaceLifecycleAction::ResizeSurface { width, height } => {
                            let scale = crate::surface_lifecycle::content_scale(&app);
                            if let Some(rt) = raster.as_ref() {
                                let _ = rt.send(RasterCommand::Resize {
                                    width,
                                    height,
                                    content_scale: scale,
                                });
                            }
                            let rect = app.content_rect();
                            let (safe_w, safe_h) = safe_window_dimensions(
                                width, height, rect.left, rect.top, rect.right, rect.bottom,
                            );
                            let (vw, vh) = viewport_for_surface(safe_w, safe_h, scale);
                            last_viewport = Some((vw, vh));
                            if let Some(runtime) = current.as_ref() {
                                let mut tree = runtime.tree.borrow_mut();
                                tree.set_viewport(vw, vh);
                                // CreateSurface と同じ理由（上のコメント参照）で、viewport 変更を
                                // 即座にレイアウトへ反映させるため明示的に render を起こす。
                                let _ = tree.render(start.elapsed().as_secs_f64() * 1000.0);
                            }
                        }
                        SurfaceLifecycleAction::Quit => quit = true,
                        SurfaceLifecycleAction::NoOp => {}
                    }
                }
            }
        });

        // lifecycle イベント（surface 作成/破棄/resize/RedrawNeeded）は再描画が要る wake 源。
        if lifecycle_wake.get() {
            frame_loop.request_wake();
        }

        // reload WS の受信を排出し（on_reload がフラグを立てる / on_close が再接続を保留する）、
        // 期限の来た backoff 再接続を main で起こす。
        if let Some(socket) = reload_socket_slot.borrow().as_ref() {
            socket.pump();
        }
        // 期限が来た再接続だけ owned で取り出し、borrow を手放してから発火する（fire() = open() が
        // 再び pending_reconnect に触り得るため、借用を跨がせない）。
        let due_reconnect = if pending_reconnect
            .borrow()
            .as_ref()
            .is_some_and(|(_, due)| Instant::now() >= *due)
        {
            pending_reconnect.borrow_mut().take()
        } else {
            None
        };
        if let Some((fire, _)) = due_reconnect {
            fire();
        }

        // full reload：WS `reload` を受けていたら Hermes ランタイムを作り直して新バンドルを再 eval
        // する（tree ごと作り直すので state は飛ぶ）。不一致／取得失敗は明示エラーで pump させない。
        if reload_requested.replace(false) {
            log::info!(
                "Miharashi: reload を受信 — Hermes ランタイムを再構築します（full reload。state は飛びます）"
            );
            match boot() {
                Ok(mut runtime) => {
                    if let Some((vw, vh)) = last_viewport {
                        runtime.tree.borrow_mut().set_viewport(vw, vh);
                    }
                    current = Some(runtime);
                    crate::error_overlay::clear_error();
                    // 新ツリーは未描画。冷間始動を要求して最初のフレームを必ず出す。
                    frame_loop.request_wake();
                }
                Err(error) => {
                    crate::error_overlay::show_error(&report_boot_error(&error));
                    current = None;
                }
            }
        }

        // ランタイム未確立（boot 失敗 / 不一致 / reload 待ち）なら入力も JS pump もしない
        // （謎クラッシュ回避）。エラー表示自体は上の `error_overlay` が Hayate/GPU パイプラインを
        // 経由せず既に出しているので、ここでは何もしなくてよい。
        let Some(runtime) = current.as_mut() else {
            continue;
        };

        // 1. 入力（native→tree 直結, 既存資産を共有）。入力は Platform Adapter の責務で
        //    idle でも毎イテレーション排出する（これが入力 wake の出所, ADR-0080/0126）。到着した
        //    タッチ/IME は on-demand ループを冷間始動する。
        let touch_woke = process_touch_input(
            &app,
            &mut runtime.tree.borrow_mut(),
            &mut runtime.touch_scroll,
            start.elapsed().as_secs_f64() * 1000.0,
        );
        let ime_woke = sync_ime(
            &app,
            &mut runtime.tree.borrow_mut(),
            &mut ime_state,
            &mut ime_target,
            &mut ime_keyboard_shown,
        );
        if touch_woke || ime_woke {
            frame_loop.request_wake();
            // JS 側の frame ループは自前の armed 状態（`HayateRenderer` の `pendingFrame`）を
            // 持ち、native の on-demand ループ（`frame_loop`）が起きただけでは再武装されない
            // （issue #475 で resize は native→tree 直結にしたが、`pumpFrame` 自体は JS が
            // armed でないと何もしない一発コールバック契約のまま）。Web は自前配線した
            // ポインタ/編集 listener から `set_request_redraw` を叩いて揃えている
            // （hayate-renderer.ts の `start()` 参照）。Android は入力を native→tree 直結で
            // 処理するため同じ配線が無く、初回フレーム以降ずっと `pendingFrame` が null の
            // まま＝タップ/スクロールが一切効かなくなっていた。ここで揃える。
            runtime.hermes.pin_mut().request_redraw();
        }
        // JS の frame ループが armed になった（`requestFrame` が呼ばれた）ことを都度拾う。
        // web の requestAnimationFrame と違い、Android の on-demand ループには自走クロックが
        // 無い。click ハンドラが `setStyle` 等を呼ぶと `scheduleFrame` が自己再武装するが、
        // その変更は次の `flush` でしか native tree に反映されない。この wake が無いと、次の
        // native 入力が来るまで pump が二度と起きず、タップの結果が永久に描画へ反映されない
        // （register_listener/dispatch 自体は成功しているのに見た目が変わらない不具合の原因）。
        if runtime.hermes.pin_mut().consume_wants_pump() {
            frame_loop.request_wake();
        }

        // wake も継続 pending も無ければ idle：このフレームは pump も present もしない
        //（ADR-0126: idle で 0 フレーム）。`poll_events` の最大 16ms ブロックが次の OS イベントを待つ。
        if !frame_loop.wants_frame() {
            continue;
        }

        let timestamp_ms = start.elapsed().as_secs_f64() * 1000.0;

        // 2. JS フレーム（flush→render→poll_events を JS 内で回す）。ホストブリッジの `render` が
        //    `tree.render`（レイアウト + 保持シーン lower）を 1 回だけ走らせる。
        runtime.hermes.pin_mut().pump_frame(timestamp_ms);

        // 3. present（保持シーンを再取得して GPU 提示）。`tree.render` を再実行せず、JS フレームが
        //    lower 済みの保持シーン（`scene_graph()`）をそのまま提示する＝tick 1 回 = 1 render
        //    （ADR-0126 の二重 render 解消）。raster gating の入力は JS フレーム内の `tree.render`
        //    が捕捉した frame_layers / frame_layer_dirty（#632）。JS の flush（apply_mutations）が
        //    立てた dirty も render 内捕捉なので取りこぼさない。
        if let Some(rt) = raster.as_ref() {
            // 保持シーンの owned スナップショットを Raster スレッドへ送る（#635）。UI スレッドは
            // raster を待たず、続けて次の JS フレーム / 入力処理へ進める（ADR-0128）。
            let tree_ref = runtime.tree.borrow();
            let _ = rt.send(frame_handoff(&tree_ref));
        }

        // 描画後に残る pending visual work（進行中 transition / カーソル点滅 / スクロール物理）を
        // 継続として記録する。残れば次イテレーションで自走し、無ければ idle へ落ちる。
        let pending = runtime.tree.borrow().has_pending_visual_work();
        frame_loop.note_frame_rendered(pending);
    }
}
