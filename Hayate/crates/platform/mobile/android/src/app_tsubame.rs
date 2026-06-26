//! JS 駆動の描画 + 入力ループ（ADR-0112, feature=tsubame-js）。**device 未検証**。
//!
//! 既存の `app::android_main`（デモツリー直挿し）を非破壊で温存したまま、こちらは
//! ネイティブ Hayate を `__hayateHost` として Hermes に注入し、dev-server から**実行時
//! ネットワーク fetch** した Tsubame バンドルを eval して描画する（Miharashi, #532）。源は
//! `bundle_source` に切り出してホストで契約テストする。eval シームは不変。
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
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use android_activity::{AndroidApp, MainEvent, PollEvent};
use hayate_core::{ElementId, ElementTree};

use crate::app::{init_gpu_surface, process_touch_input, sync_ime, GpuSurface};
use crate::bundle_source;
use crate::hermes_bridge::{make_bridge, new_hermes_app, HermesApp};
use hayate_core::element::ime_reconcile::TextInputState;
use crate::surface_lifecycle::{
    viewport_for_surface, window_dimensions, SurfaceLifecycleAction, SurfaceLifecycleState,
};

pub(crate) fn run(app: AndroidApp) {
    // バンドル源は dev-server からの実行時ネットワーク fetch（#532）。取得した JS ソースを
    // そのまま下の eval シームへ渡す。
    let bundle = match bundle_source::fetch_dev_bundle() {
        Ok(src) => src,
        Err(err) => {
            log::error!("hayate-adapter-android: dev-server からのバンドル取得に失敗: {err:?}");
            return;
        }
    };

    // JS ホストと共有する ElementTree。
    let tree: Rc<RefCell<ElementTree>> = Rc::new(RefCell::new(ElementTree::new()));

    // Hermes ランタイムを起動し、ネイティブ Hayate を __hayateHost として注入。
    // バンドルが `globalThis.__tsubame`（pumpFrame）を公開する。resize は native→tree
    // 直結（下の set_viewport）で JS を経路から外した（ADR-0080 を native へ延長, #475）。
    let mut hermes: cxx::UniquePtr<HermesApp> = new_hermes_app(make_bridge(tree.clone()), &bundle);

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
                                tree.borrow_mut().set_viewport(vw, vh);
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
                            tree.borrow_mut().set_viewport(vw, vh);
                        }
                        SurfaceLifecycleAction::Quit => quit = true,
                        SurfaceLifecycleAction::NoOp => {}
                    }
                }
            }
        });

        // 1. 入力（native→tree 直結, 既存資産を共有）。
        process_touch_input(&app, &mut tree.borrow_mut());
        sync_ime(
            &app,
            &mut tree.borrow_mut(),
            &mut ime_state,
            &mut ime_target,
            &mut ime_keyboard_shown,
        );

        let timestamp_ms = start.elapsed().as_secs_f64() * 1000.0;

        // 2. JS フレーム（flush→render→poll_events を JS 内で回す）。
        hermes.pin_mut().pump_frame(timestamp_ms);

        // 3. present（保持シーンを取り出して GPU 提示）。
        if let Some(surface) = gpu.as_mut() {
            let mut tree_ref = tree.borrow_mut();
            let scene = tree_ref.render(timestamp_ms);
            if let Err(err) = surface.render_frame(scene) {
                log::error!("hayate-adapter-android: render failed: {err}");
            }
        }
    }
}
