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

use crate::app::{init_gpu_surface, process_touch_input, sync_ime, GpuSurface};
use crate::bundle_source;
use crate::hermes_bridge::{make_bridge, new_hermes_app, HermesApp};
use crate::miharashi_reload::{
    boot_runtime, subscribe_reload, BootError, ReloadSocket, SubscribeReloadOptions,
};
use crate::reload_socket::{connect_reload_ws, ReloadWsSocket};
use hayate_core::element::ime_reconcile::TextInputState;
use crate::surface_lifecycle::{
    viewport_for_surface, window_dimensions, SurfaceLifecycleAction, SurfaceLifecycleState,
};

/// 1 boot 分の Hermes ランタイムと、それと共有する ElementTree。full reload では丸ごと作り直して
/// state を捨てる（CONTEXT.md「Reload」）。
struct Runtime {
    hermes: cxx::UniquePtr<HermesApp>,
    tree: Rc<RefCell<ElementTree>>,
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

/// boot 失敗を明示ログにする（mount もクラッシュもさせない・#530）。不一致は両版数を、取得失敗は
/// その種別を出す。pump には進めない（current=None のまま）。
fn report_boot_error(error: &BootError) {
    match error {
        BootError::ProtocolMismatch(mismatch) => log::error!(
            "Miharashi: protocol version 不一致のため mount しません — {}（host v{}, bundle {:?}）",
            mismatch.message,
            mismatch.host_version,
            mismatch.bundle_version,
        ),
        BootError::Fetch(err) => log::error!(
            "Miharashi: dev-server からのバンドル取得に失敗（mount しません）: {err:?}"
        ),
    }
}

pub(crate) fn run(app: AndroidApp) {
    // このホスト（decoder）に焼き込んだ wire 版数。Web ホストの HOST_PROTOCOL_VERSION と同じ
    // source of truth（`@hayate/protocol-spec` の manifest version）をネイティブ decoder
    // （生成物）から取る（#530/#533 共有）。
    let host_version = hayate_core::wire::PROTOCOL_VERSION;

    // 1 boot：dev-server からバンドルを取得 → Hermes ランタイムを構築（= eval。eval シームは
    // 不変 `new_hermes_app(make_bridge(tree.clone()), bundle)`）→ バンドルの protocol version を
    // 読みホスト版数と突き合わせる。一致時のみランタイムを返す（#532 の源 + #530 の突き合わせ）。
    // full reload は単にこれをもう一度呼ぶだけで、tree ごと作り直されて state が飛ぶ。
    let boot = || {
        boot_runtime(
            host_version,
            bundle_source::fetch_dev_bundle,
            |bundle: &str| {
                let tree: Rc<RefCell<ElementTree>> = Rc::new(RefCell::new(ElementTree::new()));
                let hermes = new_hermes_app(make_bridge(tree.clone()), bundle);
                Runtime { hermes, tree }
            },
            |runtime: &Runtime| read_bundle_protocol_version(&runtime.hermes),
        )
    };

    // 現在駆動中のランタイム（boot 失敗 / 不一致のあいだは None で、pump せず明示エラーのまま回す）。
    let mut current: Option<Runtime> = match boot() {
        Ok(runtime) => Some(runtime),
        Err(error) => {
            report_boot_error(&error);
            None
        }
    };

    // 直近の viewport。reload で tree を作り直したら、新 tree に再適用して描画サイズを引き継ぐ。
    let mut last_viewport: Option<(u32, u32)> = None;

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
        host: bundle_source::DEV_SERVER_HOST.to_owned(),
        port: bundle_source::DEV_SERVER_PORT,
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

    let mut gpu: Option<GpuSurface> = None;
    let mut lifecycle = SurfaceLifecycleState::new();
    let start = Instant::now();

    // IME 状態（既存 sync_ime と共有）。
    let mut ime_state = TextInputState::default();
    let mut ime_target: Option<ElementId> = None;
    let mut ime_keyboard_shown = false;
    let mut quit = false;

    while !quit {
        app.poll_events(Some(Duration::from_millis(16)), |event| {
            if let PollEvent::Main(main_event) = event {
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
                    _ => None,
                };

                if let Some(event) = lifecycle_event {
                    match lifecycle.handle(event) {
                        SurfaceLifecycleAction::CreateSurface => {
                            if let Some(window) = app.native_window() {
                                let (w, h) = window_dimensions(window.width(), window.height());
                                let (vw, vh) = viewport_for_surface(w, h);
                                last_viewport = Some((vw, vh));
                                if let Some(runtime) = current.as_ref() {
                                    runtime.tree.borrow_mut().set_viewport(vw, vh);
                                }
                                match pollster::block_on(init_gpu_surface(&window)) {
                                    Ok(surface) => gpu = Some(surface),
                                    Err(err) => log::error!(
                                        "hayate-adapter-android: GPU init failed: {err}"
                                    ),
                                }
                            }
                        }
                        SurfaceLifecycleAction::DestroySurface => gpu = None,
                        SurfaceLifecycleAction::ResizeSurface { width, height } => {
                            if let Some(surface) = gpu.as_mut() {
                                surface.resize(width, height);
                            }
                            let (vw, vh) = viewport_for_surface(width, height);
                            last_viewport = Some((vw, vh));
                            if let Some(runtime) = current.as_ref() {
                                runtime.tree.borrow_mut().set_viewport(vw, vh);
                            }
                        }
                        SurfaceLifecycleAction::Quit => quit = true,
                        SurfaceLifecycleAction::NoOp => {}
                    }
                }
            }
        });

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
                }
                Err(error) => {
                    report_boot_error(&error);
                    current = None;
                }
            }
        }

        // ランタイム未確立（boot 失敗 / 不一致 / reload 待ち）なら入力も描画もしない（謎クラッシュ回避）。
        let Some(runtime) = current.as_mut() else {
            continue;
        };

        // 1. 入力（native→tree 直結, 既存資産を共有）。
        process_touch_input(&app, &mut runtime.tree.borrow_mut());
        sync_ime(
            &app,
            &mut runtime.tree.borrow_mut(),
            &mut ime_state,
            &mut ime_target,
            &mut ime_keyboard_shown,
        );

        let timestamp_ms = start.elapsed().as_secs_f64() * 1000.0;

        // 2. JS フレーム（flush→render→poll_events を JS 内で回す）。
        runtime.hermes.pin_mut().pump_frame(timestamp_ms);

        // 3. present（保持シーンを取り出して GPU 提示）。
        if let Some(surface) = gpu.as_mut() {
            let mut tree_ref = runtime.tree.borrow_mut();
            let scene = tree_ref.render(timestamp_ms);
            if let Err(err) = surface.render_frame(scene) {
                log::error!("hayate-adapter-android: render failed: {err}");
            }
        }
    }
}
