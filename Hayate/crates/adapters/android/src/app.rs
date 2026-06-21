//! 描画 + タッチループ（ADR-0087）。対話的な `ElementTree`（`scene_demo`）を
//! `SceneGraph` に lower し、`android-activity` が渡す `ANativeWindow` に
//! 紐づく GPU サーフェスへ毎フレーム提示する。`MotionEvent` は `hayate-core` の
//! 座標ベースのポインタ API に変換され、タップでデモボタンの `:active` 色が
//! 画面上で切り替わる。IME / AccessKit / クリップボードは未実装。

use std::time::{Duration, Instant};

use android_activity::input::{InputEvent, MotionAction};
use android_activity::{AndroidApp, MainEvent, PollEvent};
use hayate_core::{ElementTree, SceneGraph};
use hayate_scene_renderer_vello::{
    create_blitter, create_target_view, VelloRenderTarget, VelloSceneRenderer,
};
use wgpu::util::TextureBlitter;

use hayate_core::ElementId;

use crate::ime_bridge::AndroidImeBridge;
use crate::ime_input::{apply_ime_action, translate_text_input, TextInputState, TextSpan};
use crate::scene_demo::build_demo_tree;
use crate::surface_lifecycle::{
    viewport_for_surface, window_dimensions, SurfaceLifecycleAction, SurfaceLifecycleState,
};
use crate::touch_input::{translate_touch, PointerInput, TouchAction};

/// スモークテスト用の RGBA クリアカラー。
pub const CLEAR_COLOR: [f32; 4] = crate::STAGE_A_CLEAR_COLOR;

pub(crate) struct GpuSurface {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    target_view: wgpu::TextureView,
    blitter: TextureBlitter,
    width: u32,
    height: u32,
    scene_renderer: VelloSceneRenderer,
}

/// JS 駆動経路（ADR-0112, feature=tsubame-js）。Hermes に Tsubame バンドルを載せ、
/// ネイティブ Hayate を `__hayateHost` として注入して描画する。既存のデモ経路
/// （下の `#[cfg(not(...))]` 版）は非破壊で温存し、feature でこちらに分岐する。
#[cfg(feature = "tsubame-js")]
#[no_mangle]
pub fn android_main(app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );
    crate::app_tsubame::run(app);
}

#[cfg(not(feature = "tsubame-js"))]
#[no_mangle]
pub fn android_main(app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );

    let mut gpu: Option<GpuSurface> = None;
    let mut lifecycle = SurfaceLifecycleState::new();
    let mut tree = build_demo_tree();
    let start = Instant::now();
    // 最後に同期した GameTextInput バッファ、それが属するテキスト入力、および
    // ソフトキーボードが現在表示中かどうか（IME, ADR-0094）。キーボードフラグは
    // `AndroidImeBridge` が所有し、target はフォーカス変更時のバッファ
    // ベースラインリセットを駆動する。
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
                                let (w, h) =
                                    window_dimensions(window.width(), window.height());
                                let (vw, vh) = viewport_for_surface(w, h);
                                tree.set_viewport(vw, vh);
                                match pollster::block_on(init_gpu_surface(&window)) {
                                    Ok(surface) => gpu = Some(surface),
                                    Err(err) => {
                                        log::error!(
                                            "hayate-adapter-android: GPU init failed: {err}"
                                        )
                                    }
                                }
                            }
                        }
                        SurfaceLifecycleAction::DestroySurface => gpu = None,
                        SurfaceLifecycleAction::ResizeSurface { width, height } => {
                            if let Some(surface) = gpu.as_mut() {
                                surface.resize(width, height);
                            }
                            let (vw, vh) = viewport_for_surface(width, height);
                            tree.set_viewport(vw, vh);
                        }
                        SurfaceLifecycleAction::Quit => quit = true,
                        SurfaceLifecycleAction::NoOp => {}
                    }
                }
            }
        });

        process_touch_input(&app, &mut tree);
        sync_ime(
            &app,
            &mut tree,
            &mut ime_state,
            &mut ime_target,
            &mut ime_keyboard_shown,
        );

        if let Some(surface) = gpu.as_mut() {
            // 単調増加クロックでレイアウトとカーソル点滅を駆動し、lower した
            // シーンを提示する（`hayate-adapter-web` の `render` に対応）。
            let timestamp_ms = start.elapsed().as_secs_f64() * 1000.0;
            let scene = tree.render(timestamp_ms);
            if let Err(err) = surface.render_frame(scene) {
                log::error!("hayate-adapter-android: render failed: {err}");
            }
        }
    }
}

/// GameTextInput をフォーカス中の TextInput に同期する（IME, ADR-0094）。
///
/// ソフトキーボードの表示可否は core（[`ElementTree::drive_ime`]）が編集可否から
/// 一度だけ決定し、[`AndroidImeBridge`] が反映する。このラッパー自身は
/// `show_soft_input` / `hide_soft_input` を呼ばない。タップは当たったもの
/// （ボタン・素のテキスト・ビュー）をフォーカスするが、キーボードを起こすのは
/// テキスト入力だけ。生のフォーカスでキーボードを起こすと全タップで上がって
/// しまうため、判定を core に押し込むことで web アダプタと修正を共有する。
/// ここに残るのはフォーカス中入力に対する生 GameTextInput バッファの変換のみで、
/// diff/apply ロジックはホストでテスト可能な [`crate::ime_input`] にある。
/// このラッパーは `android-activity` のテキスト入力 API への薄いグルー。
pub(crate) fn sync_ime(
    app: &AndroidApp,
    tree: &mut ElementTree,
    prev: &mut TextInputState,
    prev_target: &mut Option<ElementId>,
    keyboard_shown: &mut bool,
) {
    // 抽象を通した表示制御。core が編集可否でゲートし、bridge が
    // キーボードを上げ下げする。
    let mut bridge = AndroidImeBridge::new(app, keyboard_shown);
    tree.drive_ime(&mut bridge);

    let target = tree.focused_text_input();
    if *prev_target != target {
        *prev_target = target;
        // 新規フォーカスは空のベースラインバッファから始める。
        *prev = TextInputState::default();
    }

    let Some(target) = target else {
        return;
    };

    // GameTextInput は全バッファと任意の composing span（`text` へのバイト
    // オフセット）を報告する。これを NDK 非依存の型にミラーして diff を取る。
    // android-activity 0.6 では `text_input_state()` が状態を直接返す
    // （クロージャ形式なし）ので、そこから NDK 非依存のミラーを構築する。
    let state = app.text_input_state();
    let next = TextInputState {
        text: state.text.clone(),
        compose_region: state.compose_region.map(|span| TextSpan {
            start: span.start,
            end: span.end,
        }),
    };

    if next != *prev {
        for action in translate_text_input(prev, &next) {
            apply_ime_action(tree, target, &action);
        }
        *prev = next;
    }
}

/// 保留中の `MotionEvent` を捌き、`tree` の座標ベースのポインタ API を駆動する。
///
/// 単一ポインタのタップ/ドラッグのみ（ADR-0082）。マルチタッチジェスチャや
/// スクロール慣性（ADR-0046）は対象外。イベントごとの計算はホストでテスト可能な
/// [`translate_touch`] にあり、このラッパーは薄い NDK グルー。
pub(crate) fn process_touch_input(app: &AndroidApp, tree: &mut ElementTree) {
    let mut iter = match app.input_events_iter() {
        Ok(iter) => iter,
        Err(err) => {
            log::error!("hayate-adapter-android: input_events_iter failed: {err}");
            return;
        }
    };

    loop {
        let read = iter.next(|event| {
            if let InputEvent::MotionEvent(motion) = event {
                if let Some(action) = motion_action_to_touch(motion.action()) {
                    let pointer = motion.pointer_at_index(motion.pointer_index());
                    match translate_touch(action, pointer.x(), pointer.y()) {
                        PointerInput::Down { x, y } => tree.on_pointer_down(x, y),
                        PointerInput::Move { x, y } => {
                            let _ = tree.on_pointer_move(x, y);
                        }
                        PointerInput::Up { x, y } => tree.on_pointer_up(x, y),
                    }
                }
            }
            android_activity::InputStatus::Unhandled
        });
        if !read {
            break;
        }
    }
}

/// Android の `MotionAction` を単一ポインタの [`TouchAction`] に対応付ける。
/// 基本のタップ/ドラッグ集合外（ホバー・スクロール・ボタン等）は `None`。
fn motion_action_to_touch(action: MotionAction) -> Option<TouchAction> {
    match action {
        MotionAction::Down | MotionAction::PointerDown => Some(TouchAction::Down),
        MotionAction::Move => Some(TouchAction::Move),
        MotionAction::Up | MotionAction::PointerUp => Some(TouchAction::Up),
        MotionAction::Cancel => Some(TouchAction::Cancel),
        _ => None,
    }
}

pub(crate) async fn init_gpu_surface(
    window: &ndk::native_window::NativeWindow,
) -> Result<GpuSurface, String> {
    let (width, height) = window_dimensions(window.width(), window.height());

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });

    // `SurfaceTargetUnsafe::from_window` は `raw_display_handle` を常に `None` に
    // するため、`new_without_display_handle()` の Instance と組み合わせると wgpu が
    // `MissingDisplayHandle` で失敗する（黒画面の原因）。Android の display handle を
    // 明示して `RawHandle` を直接構築する。
    use wgpu::rwh::{AndroidDisplayHandle, HasWindowHandle, RawDisplayHandle};
    let raw_window_handle = window
        .window_handle()
        .map_err(|e| format!("window_handle: {e}"))?
        .as_raw();

    // SAFETY: `window` はこのアダプタの生存期間中サーフェスより長く生きる
    // （`InitWindow` で再生成、`TerminateWindow` で破棄）。
    let surface = unsafe {
        instance
            .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(RawDisplayHandle::Android(AndroidDisplayHandle::new())),
                raw_window_handle,
            })
            .map_err(|e| format!("create_surface_unsafe: {e}"))?
    };

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        })
        .await
        .map_err(|e| format!("no compatible wgpu adapter: {e}"))?;

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("hayate-android"),
            ..Default::default()
        })
        .await
        .map_err(|e| format!("request_device: {e}"))?;

    let mut surface_config = surface
        .get_default_config(&adapter, width, height)
        .ok_or("surface not supported by adapter")?;
    surface_config.usage |= wgpu::TextureUsages::RENDER_ATTACHMENT;
    surface.configure(&device, &surface_config);

    let surface_format = surface_config.format;
    let target_view = create_target_view(&device, width, height);
    let blitter = create_blitter(&device, surface_format);
    let scene_renderer = VelloSceneRenderer::new(&device)?;

    Ok(GpuSurface {
        device,
        queue,
        surface,
        surface_config,
        target_view,
        blitter,
        width,
        height,
        scene_renderer,
    })
}

impl GpuSurface {
    pub(crate) fn render_frame(&mut self, scene: &SceneGraph) -> Result<(), String> {
        let target = VelloRenderTarget {
            device: &self.device,
            queue: &self.queue,
            target_view: &self.target_view,
            width: self.width,
            height: self.height,
        };
        self.scene_renderer
            .render_scene(scene, &target, CLEAR_COLOR, 1.0)?;

        let surface_texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Occluded => return Ok(()),
            other => return Err(format!("get_current_texture: {other:?}")),
        };

        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("hayate_android_blit"),
            });
        self.blitter
            .copy(&self.device, &mut encoder, &self.target_view, &surface_view);
        self.queue.submit(std::iter::once(encoder.finish()));
        surface_texture.present();
        Ok(())
    }

    pub(crate) fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 || (width == self.width && height == self.height) {
            return;
        }
        self.width = width;
        self.height = height;
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
        self.target_view = create_target_view(&self.device, width, height);
    }
}
