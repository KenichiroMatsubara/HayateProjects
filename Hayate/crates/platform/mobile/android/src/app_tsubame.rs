//! JS 駆動の描画 + 入力ループ（ADR-0112, feature=tsubame-js）。**device 未検証**。
//!
//! 既存の `app::android_main`（デモツリー直挿し）を非破壊で温存したまま、こちらは
//! ネイティブ Hayate を `__hayateHost` として Hermes に注入し、dev-server から**実行時
//! ネットワーク fetch** した Tsubame バンドルを eval して描画する（Torimi, #532）。源は
//! `bundle_source` に切り出してホストで契約テストする。eval シームは不変。
//!
//! #533 で full reload と protocol version 整合を移植した（Web #529/#530 と対称）:
//!   - boot（fetch → ランタイム構築/eval → version 突き合わせ）は `torimi_reload::boot_runtime`。
//!     不一致／取得失敗はランタイムの pump に進めず明示エラーにする（謎クラッシュ回避）。
//!   - dev-server の WS `reload` を `torimi_reload::subscribe_reload`（device connect は
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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use android_activity::{AndroidApp, AndroidAppWaker, MainEvent, PollEvent};
use hayate_app_host::FrameContinuation;
use hayate_core::{ElementId, ElementTree};

use crate::app::{frame_handoff, process_touch_input, sync_ime, RasterHandle};
use crate::bundle_source;
use crate::demo_manifest;
use crate::dev_server_target;
use crate::device_log::{self, DeviceLog, KotlinLogPort};
use crate::frame_schedule::AndroidFrameScheduler;
use crate::hermes_bridge::{make_bridge, new_hermes_app, HermesApp};
use crate::reload_socket::{connect_reload_ws, ReloadWsSocket};
use crate::surface_lifecycle::{window_dimensions, SurfaceLifecycleAction, SurfaceLifecycleState};
use crate::torimi_reload::{
    boot_runtime, subscribe_reload, BootError, ReloadSocket, SubscribeReloadOptions,
};
use crate::touch_scroll::TouchScrollState;
use hayate_core::element::ime_reconcile::TextInputState;
use hayate_layer_compositor::RasterCommand;
use hayate_performance_observability::{
    FrameCounters, FrameDeadline, PerformanceObservability, PerformancePhase,
    DEFAULT_REFRESH_RATE_HZ,
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

/// eval 済みバンドルが立てた protocol version（`__torimiProtocolVersion`）を読み、`Option<u32>`
/// に正規化する。C++ 側は未埋め込み / 非数値を負値で返す（`@torimi/protocol-handshake` の
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
            "Torimi: protocol version 不一致のため mount しません — {}（host v{}, bundle {:?}）",
            mismatch.message, mismatch.host_version, mismatch.bundle_version,
        ),
        BootError::Fetch(err) => {
            format!("Torimi: dev-server からのバンドル取得に失敗（mount しません）: {err:?}")
        }
    };
    log::error!("{message}");
    message
}

/// host イベントの記録時刻（端末側 wall-clock epoch ms）。Device Log の `ts` に載る（#789）。
/// フラッシュ間隔の monotonic clock（`start.elapsed`）とは別軸。
fn now_epoch_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0)
}

/// Arm one resource timer only while a Device Log batch is buffered. The timer never commits a
/// frame; it merely wakes the blocked Android loop so `DeviceLog::tick` can flush or schedule one
/// retry. With an empty buffer there is no timer and the application remains fully idle.
fn arm_device_log_flush_wake(
    has_buffered_entries: bool,
    armed: &Arc<AtomicBool>,
    waker: &AndroidAppWaker,
) {
    if !has_buffered_entries
        || armed
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
    {
        return;
    }
    let armed = Arc::clone(armed);
    let waker = waker.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(
            device_log::FLUSH_INTERVAL_MS as u64,
        ));
        armed.store(false, Ordering::Release);
        waker.wake();
    });
}

pub(crate) fn run(app: AndroidApp) {
    let event_waker = app.create_waker();
    let device_log_flush_wake_armed = Arc::new(AtomicBool::new(false));
    // このホスト（decoder）に焼き込んだ wire 版数。Web ホストの HOST_PROTOCOL_VERSION と同じ
    // source of truth（`@hayate/protocol-spec` の manifest version）をネイティブ decoder
    // （生成物）から取る（#530/#533 共有）。
    let host_version = hayate_core::wire::PROTOCOL_VERSION;

    // 接続先の決定（#534 / #743）。端末 UI（Kotlin の EditText / QR）が internal data dir に書いた URL を
    // 読み戻し、`demo_manifest::plan_boot` で boot 経路を分ける：
    //   - URL 入力済み（QR 含む）／debug の未入力（エミュレータ loopback）→ 単一バンドル直 boot（既存経路・不変）。
    //   - release の**接続先未設定の初回起動** → 公開 Demo Endpoint（ADR-0003）の Demo Manifest を
    //     OS スタック（#740）で取り、**先頭デモを自動ロード**する（ゼロ入力でデモが動く）。
    // 解決した 1 つの target が、バンドル fetch（HTTP）と reload 購読（WS）の**両方**を駆動する（保持）。
    // マニフェスト取得/解釈の失敗は明示エラー＋ URL 入力経路への誘導にして謎クラッシュにしない。
    let entered = dev_server_target::read_entered_url(app.internal_data_path().as_deref());
    let plan = demo_manifest::plan_boot(entered.as_deref(), !cfg!(debug_assertions));
    // Device Log を送るのは bundle 取得元が Dev Server（`Direct`）のときだけ。Demo Endpoint 経由
    // （`ManifestAutoload`）はログ送信自体をしない（CONTEXT.md「Device Log」・ADR-0005）。
    let log_origin = match &plan {
        demo_manifest::BootPlan::Direct(_) => device_log::BundleOrigin::DevServer,
        demo_manifest::BootPlan::ManifestAutoload(_) => device_log::BundleOrigin::DemoEndpoint,
    };
    let (target, autoload_error): (dev_server_target::DevServerTarget, Option<String>) = match plan
    {
        demo_manifest::BootPlan::Direct(t) => (t, None),
        demo_manifest::BootPlan::ManifestAutoload(endpoint) => {
            match demo_manifest::first_boot_target_fetched(&endpoint) {
                Ok(demo_target) => (demo_target, None),
                // 取得/解釈失敗：reload は Demo Endpoint origin に張ったまま（同一 origin・path 非依存）、
                // boot は下で明示エラー表示のうえ current=None に留める（URL 入力画面へ誘導）。
                Err(err) => (endpoint, Some(err.message())),
            }
        }
    };

    // Device Log シーム（#787-789・ADR-0005）。Device ID はインストール単位でローカル永続化（初回だけ
    // ランダム生成、再起動後も同じ）、Device Label は端末モデル名。送信先 base（scheme/host/port）は
    // 解決済み target を共有する（bundle fetch / reload と同じ配信点）。reload を跨いで seq 連番・Device ID
    // を継続させるため `Rc<RefCell>` で持ち、boot ごとに作り直す bridge へ同じ `Rc` を渡す。
    let device_id = device_log::load_or_create_device_id(
        app.internal_data_path().as_deref(),
        device_log::random_device_id,
    );
    let device_log = Rc::new(RefCell::new(DeviceLog::with_origin(
        device_id,
        device_log::device_label(),
        KotlinLogPort::new(target.clone()),
        0.0,
        log_origin,
    )));

    // 1 boot：dev-server からバンドルを取得 → Hermes ランタイムを構築（= eval。eval シームは
    // 不変 `new_hermes_app(make_bridge(tree.clone()), bundle)`）→ バンドルの protocol version を
    // 読みホスト版数と突き合わせる。一致時のみランタイムを返す（#532 の源 + #530 の突き合わせ）。
    // full reload は単にこれをもう一度呼ぶだけで、tree ごと作り直されて state が飛ぶ。
    let device_log_for_boot = Rc::clone(&device_log);
    let boot = || {
        boot_runtime(
            host_version,
            || bundle_source::fetch_from(&target),
            |bundle: &str| {
                let tree: Rc<RefCell<ElementTree>> = Rc::new(RefCell::new(ElementTree::new()));
                // reload を跨ぐ Device Log シームは同じ Rc を共有する（seq 連番・Device ID 継続）。
                let hermes = new_hermes_app(
                    make_bridge(tree.clone(), Rc::clone(&device_log_for_boot)),
                    bundle,
                );
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
    // マニフェスト取得/解釈に失敗した初回起動（`autoload_error`）は boot せず、その明示エラーを出して
    // URL 入力経路へ誘導する（謎クラッシュにしない・#743）。
    let mut current: Option<Runtime> = if let Some(message) = autoload_error {
        // Demo Endpoint 経路の manifest 失敗。host イベントとして合流させる（送信は Demo Endpoint
        // 経由なので実際には出ないが、合流点を一様にして扱いを分岐させない・#789）。
        device_log.borrow_mut().record_host(
            device_log::LogLevel::Error,
            message.clone(),
            now_epoch_ms(),
        );
        crate::error_overlay::show_error(&message);
        None
    } else {
        match boot() {
            Ok(runtime) => Some(runtime),
            Err(error) => {
                // bundle 取得失敗・protocol version 不一致は host イベント（source: "host"）として
                // Device Log に合流し、即時フラッシュ経路（#788）で USB なしに dev-server へ届く（#789）。
                let message = report_boot_error(&error);
                device_log.borrow_mut().record_host(
                    device_log::LogLevel::Error,
                    message.clone(),
                    now_epoch_ms(),
                );
                crate::error_overlay::show_error(&message);
                None
            }
        }
    };

    // 直近の viewport。reload で tree を作り直したら、新 tree に再適用して描画サイズを引き継ぐ。
    let mut last_viewport: Option<(f32, f32)> = None;

    // ── full reload 購読（#533）────────────────────────────────────────────────
    // WS `reload` 受信で reload フラグを立て、poll ループが拾って再 boot する。WS の blocking read は
    // 背景スレッド（reload_socket）が担い、受信 wake 後に main が pump() で排出する（単一スレッド契約）。
    let reload_requested = Rc::new(Cell::new(false));
    let reload_flag = Rc::clone(&reload_requested);
    // connect が張った具体 socket を main 用に保持する（pump 駆動のため。再接続で差し替わる）。
    let reload_socket_slot: Rc<RefCell<Option<Rc<ReloadWsSocket>>>> = Rc::new(RefCell::new(None));
    let slot_for_connect = Rc::clone(&reload_socket_slot);
    let waker_for_connect = event_waker.clone();
    // backoff 再接続は main で起こす必要がある（open() が !Send な Rc/RefCell に触るため）。タイマー
    // 背景の one-shot timer は event loop を wake するだけで、「期限つき保留」の発火自体は main が行う。
    let pending_reconnect: Rc<RefCell<Option<(Box<dyn FnOnce()>, Instant)>>> =
        Rc::new(RefCell::new(None));
    let pending_for_schedule = Rc::clone(&pending_reconnect);
    let waker_for_reconnect = event_waker.clone();

    let _reload_subscription = subscribe_reload(SubscribeReloadOptions {
        // バンドル fetch と同じ target を共有する＝同じ配信点を指す（https 由来なら wss・#742）。
        target: target.clone(),
        on_reload: Box::new(move || reload_flag.set(true)),
        connect: Box::new(move |url| {
            let socket = connect_reload_ws(url, waker_for_connect.clone());
            *slot_for_connect.borrow_mut() = Some(Rc::clone(&socket));
            socket as Rc<dyn ReloadSocket>
        }),
        schedule_reconnect: Box::new(move |fire, delay| {
            *pending_for_schedule.borrow_mut() = Some((fire, Instant::now() + delay));
            let waker = waker_for_reconnect.clone();
            std::thread::spawn(move || {
                std::thread::sleep(delay);
                waker.wake();
            });
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

    // ADR-0154: Choreographer is the only frame clock. The scheduler owns at most one posted
    // one-shot callback and folds every wake before vsync into it.
    let frame_scheduler = AndroidFrameScheduler::new();
    frame_scheduler.request_frame();
    // This remains an inert value in ordinary release/debug builds. The profileable benchmark
    // variant enables the Cargo feature and turns it into a fixed-capacity reporter.
    let observability = PerformanceObservability::new();

    while !quit {
        // このイテレーションで lifecycle イベントを観測したか（surface 作成/破棄/resize は
        // いずれも再描画が要る wake 源）。closure から scheduler を操作せず Cell 経由で集約する。
        let lifecycle_wake = Cell::new(false);
        app.poll_events(None, |event| {
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
                            let (vw, vh) = crate::app::effective_insets(&app, w, h)
                                .layout_viewport(w, h, scale);
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
                                let (vw, vh) = crate::app::effective_insets(&app, w, h)
                                    .layout_viewport(w, h, scale);
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
                                // Renderer Selection Policy（skia → vello の一方向 fallback、
                                // issue #801/#802）越しに初期化し、対応する Raster スレッドを
                                // 起動する（#635 の move-after-creation はどちらの経路でも内部
                                // で行う）。
                                raster = crate::app::init_and_spawn_raster(&window, scale);
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
                            let (vw, vh) = crate::app::effective_insets(&app, width, height)
                                .layout_viewport(width, height, scale);
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
            frame_scheduler.request_frame();
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
                "Torimi: reload を受信 — Hermes ランタイムを再構築します（full reload。state は飛びます）"
            );
            match boot() {
                Ok(runtime) => {
                    if let Some((vw, vh)) = last_viewport {
                        runtime.tree.borrow_mut().set_viewport(vw, vh);
                    }
                    current = Some(runtime);
                    crate::error_overlay::clear_error();
                    // 新ツリーは未描画。冷間始動を要求して最初のフレームを必ず出す。
                    frame_scheduler.request_frame();
                }
                Err(error) => {
                    // reload 後の boot 失敗（取得失敗・version 不一致）も host イベントとして合流させる（#789）。
                    let message = report_boot_error(&error);
                    device_log.borrow_mut().record_host(
                        device_log::LogLevel::Error,
                        message.clone(),
                        now_epoch_ms(),
                    );
                    crate::error_overlay::show_error(&message);
                    current = None;
                }
            }
        }

        // Device Log の定期フラッシュ（#787）。monotonic clock（boot 起点の経過 ms）で間隔を計り、
        // 溜まっていれば 1 バッチにまとめて送る。runtime の有無に関わらず、OS / resource /
        // Choreographer のいずれかで起きたイテレーションで呼ぶ。idle を定期 polling で起こすことはしない。
        device_log
            .borrow_mut()
            .tick(start.elapsed().as_secs_f64() * 1000.0);
        arm_device_log_flush_wake(
            device_log.borrow().has_buffered_entries(),
            &device_log_flush_wake_armed,
            &event_waker,
        );

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
            frame_scheduler.request_frame();
            // JS 側の frame ループは自前の armed 状態（`HayateRenderer` の `pendingFrame`）を
            // 持ち、native の on-demand scheduler が起きただけでは再武装されない
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
            frame_scheduler.request_frame();
        }

        // Input/lifecycle/resource wakes only arm Choreographer. Commit at most once for an actual
        // vsync callback, using its frameTimeNanos as the sole frame timestamp.
        let Some(timestamp_ms) = frame_scheduler.take_frame_timestamp_ms() else {
            continue;
        };
        if raster
            .as_ref()
            .is_some_and(RasterHandle::has_terminal_failure)
        {
            continue;
        }
        let mut observation =
            observability.begin_frame(FrameDeadline::from_refresh_rate_hz(DEFAULT_REFRESH_RATE_HZ));

        // 2. JS フレーム（flush→render→poll_events を JS 内で回す）。ホストブリッジの `render` が
        //    `tree.render`（レイアウト + 保持シーン lower）を 1 回だけ走らせる。
        let app_host_started = observation.is_enabled().then(Instant::now);
        observation.measure(PerformancePhase::CoreCommit, || {
            runtime.hermes.pin_mut().pump_frame(timestamp_ms);
        });
        // `pump_frame` 中の resource/microtask completion が JS の one-shot callback を
        // 再武装した場合も、次の vsync を一度だけ要求する。
        if runtime.hermes.pin_mut().consume_wants_pump() {
            frame_scheduler.request_frame();
        }

        // 3. present（保持シーンを再取得して GPU 提示）。`tree.render` を再実行せず、JS フレームが
        //    lower 済みの保持シーン（`scene_graph()`）をそのまま提示する＝tick 1 回 = 1 render
        //    （ADR-0126 の二重 render 解消）。raster gating の入力は JS フレーム内の `tree.render`
        //    が捕捉した frame_layers / frame_layer_dirty（#632）。JS の flush（apply_mutations）が
        //    立てた dirty も render 内捕捉なので取りこぼさない。
        if let Some(rt) = raster.as_ref() {
            // 保持シーンの owned スナップショットを Raster スレッドへ送る（#635）。UI スレッドは
            // raster を待たず、続けて次の JS フレーム / 入力処理へ進める（ADR-0128）。
            let tree_ref = runtime.tree.borrow();
            let frame = tree_ref.committed_frame();
            observation.set_counters(FrameCounters {
                nodes: frame.snapshot().len() as u32,
                layers: frame.layer_topology().paint_order().len() as u32,
                dirty_layers: frame.layer_topology().content_changed().len() as u32,
                cache_hits: 0,
                cache_misses: 0,
                allocations: 0,
                ..FrameCounters::default()
            });
            let _ = observation.measure(PerformancePhase::RendererSubmit, || {
                rt.send(frame_handoff(&frame))
            });
        }

        // 描画後に残る pending visual work（進行中 transition / カーソル点滅 / スクロール物理）を
        // 継続として記録する。残れば native loop と JS の one-shot frame callback の両方を
        // 再武装する。Core が直接開始したスクロール物理は JS mutation を伴わないため、native
        // loop だけを継続しても `pumpFrame` が空振りし、端の overscroll 位置で次の入力まで
        // 固まることがある。無ければ両方とも idle へ落ちる。
        let continuation = {
            let tree = runtime.tree.borrow();
            FrameContinuation::after_commit(&tree.committed_frame())
        };
        if continuation.requests_frame() {
            runtime.hermes.pin_mut().request_redraw();
            frame_scheduler.request_frame();
        }
        if let Some(started) = app_host_started {
            observation.record_phase(
                PerformancePhase::AppHost,
                started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64,
            );
        }
        observation.finish();
        if let Some(report) = observability.periodic_summary() {
            log::info!(
                target: "HayatePerf",
                "window samples={} total_p95_ns={} frames_over_2x={} app_host_p95_ns={} core_commit_p95_ns={} scene_lowering_p95_ns={} layer_presentation_p95_ns={} renderer_submit_p95_ns={} renderer_present_p95_ns={} nodes={} layers={} dirty={} cache_hit={} cache_miss={} allocations={} cpu_resident_bytes={} gpu_resident_bytes={} resource_evictions={} resource_rebuild_cost={}",
                report.sample_count,
                report.total_p95_ns,
                report.frames_over_two_intervals,
                report.phase_p95_ns[PerformancePhase::AppHost as usize],
                report.phase_p95_ns[PerformancePhase::CoreCommit as usize],
                report.phase_p95_ns[PerformancePhase::SceneLowering as usize],
                report.phase_p95_ns[PerformancePhase::LayerPresentation as usize],
                report.phase_p95_ns[PerformancePhase::RendererSubmit as usize],
                report.phase_p95_ns[PerformancePhase::RendererPresent as usize],
                report.counters.nodes,
                report.counters.layers,
                report.counters.dirty_layers,
                report.counters.cache_hits,
                report.counters.cache_misses,
                report.counters.allocations,
                report.counters.cpu_resident_bytes,
                report.counters.gpu_resident_bytes,
                report.counters.resource_evictions,
                report.counters.resource_rebuild_cost,
            );
        }
        // JS console output may have been recorded by `pump_frame` after the tick near the top of
        // this iteration. Arm its single resource wake now without scheduling another render.
        arm_device_log_flush_wake(
            device_log.borrow().has_buffered_entries(),
            &device_log_flush_wake_armed,
            &event_waker,
        );
    }
}
