//! 描画 + タッチループ（ADR-0087）。対話的な `ElementTree`（`scene_demo`）を
//! `SceneGraph` に lower し、`android-activity` が渡す `ANativeWindow` に
//! 紐づく GPU サーフェスへ毎フレーム提示する。`MotionEvent` は `hayate-core` の
//! 座標ベースのポインタ API に変換され、タップでデモボタンの `:active` 色が
//! 画面上で切り替わる。IME / AccessKit / クリップボードは未実装。

use std::collections::HashSet;
use std::time::{Duration, Instant};

use android_activity::input::{InputEvent, MotionAction};
use android_activity::{AndroidApp, MainEvent, PollEvent};
use hayate_core::{ElementTree, SceneGraph};
use hayate_layer_compositor::{PresentPlanner, RasterCommand, RasterHandoff, RasterThread};
use hayate_scene_renderer_vello::{
    create_blitter, create_target_view, VelloRenderTarget, VelloSceneRenderer,
};
use wgpu::util::TextureBlitter;

use hayate_core::ElementId;

use crate::ime_bridge::AndroidImeBridge;
use crate::safe_area::SafeAreaInsets;
use crate::scene_demo::build_demo_tree;
use crate::surface_lifecycle::{window_dimensions, SurfaceLifecycleAction, SurfaceLifecycleState};
use crate::touch_input::{translate_touch, TouchAction};
use crate::touch_scroll::TouchScrollState;
use hayate_core::element::ime_reconcile::{
    apply_ime_action, translate_text_input, TextInputState, TextSpan,
};

/// スモークテスト用の RGBA クリアカラー。
pub const CLEAR_COLOR: [f32; 4] = crate::STAGE_A_CLEAR_COLOR;

/// 未捕捉 panic を logcat へ明示ログし、ネイティブ View オーバーレイ（`error_overlay`）にも
/// 出してから既定フックへ委譲する。既定の panic hook はメッセージを stderr に書くだけで、
/// Android アプリの stderr は logcat にリダイレクトされていないため、`.unwrap()` 等からの
/// panic はログにも画面にも何も出ず「エラーメッセージなく落ちる」ように見えていた。
/// オーバーレイは Hayate（このアプリの GPU 描画パイプライン）を一切経由しないネイティブ
/// Android View なので、panic の原因が描画パイプライン自身にあっても表示できる。
fn install_panic_logger() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message =
            format!("hayate-adapter-android: 未捕捉 panic でアプリが異常終了します — {info}");
        log::error!("{message}");
        crate::error_overlay::show_error(&message);
        default_hook(info);
    }));
}

pub(crate) struct GpuSurface {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    /// vello が毎 dirty フレーム全面 raster するオフスクリーンターゲット（Web の
    /// `VelloSurfaceHost` と同型、#687）。サーフェスへは `blitter` で blit する。
    target_view: wgpu::TextureView,
    blitter: TextureBlitter,
    width: u32,
    height: u32,
    /// present 側 raster gating（#632、#687 で単一 root 経路に揃えた）。`plan` が dirty /
    /// 未キャッシュなら raster、clean なら composite-only（前回 present 済みの target_view を
    /// そのまま維持——実際は blit そのものをスキップして触らない）。
    planner: PresentPlanner,
    /// シーン全体を毎 dirty フレーム再描画する vello レンダラ。per-layer レイヤキャッシュ＋専用
    /// wgpu compositor（ADR-0125/0127/0128）は #680 の実機回帰の温床だったため撤去し、同一
    /// ハードウェアで高速だった Web の現行経路（`vello.rs::SelectedBackend`）と揃えた（#687）。
    /// per-layer 実装コード自体は将来の Web 検証issue向けに削除していない。
    scene_renderer: VelloSceneRenderer,
    /// 論理px→物理pxの倍率（DPI 対応）。`render_scene` へ毎フレーム渡す。
    content_scale: f32,
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
    install_panic_logger();
    crate::app_tsubame::run(app);
}

#[cfg(not(feature = "tsubame-js"))]
#[no_mangle]
pub fn android_main(app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );
    install_panic_logger();

    // #635: raster/composite は専用 Raster スレッドが所有する GpuSurface で走る（ADR-0128）。UI
    // スレッドは handoff を送るだけで raster 完了を待たない。surface 作成/破棄はこのハンドルの
    // Some/None で表す（None にすると drop → 安全に join、= surface 破棄で Raster スレッド停止）。
    let mut raster: Option<RasterHandle> = None;
    let mut lifecycle = SurfaceLifecycleState::new();
    let mut tree = build_demo_tree();
    // scroll-view 上のタッチドラッグ→スクロールジェスチャ（ADR-0082）。フレームを
    // またいで 1 つの scroll-view にロックされた状態を保持する（`touch_scroll` 参照）。
    let mut touch_scroll = TouchScrollState::new();
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
                    // システムUI（ステータスバー/ナビゲーションバー/ソフトキーボード）の
                    // 実表示領域が変わったときに届く（surface 自体の init/resize/destroy では
                    // ない）。四論理イベントの state machine には乗せず、ここで直接ビューポート
                    // だけ更新する（最下点固定ピクセルずれバグの修正）。
                    MainEvent::ContentRectChanged { .. } => {
                        if let Some(window) = app.native_window() {
                            let scale = crate::surface_lifecycle::content_scale(&app);
                            let (vw, vh) = safe_viewport(&app, &window, scale);
                            tree.set_viewport(vw, vh);
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
                                let (vw, vh) = safe_viewport(&app, &window, scale);
                                tree.set_viewport(vw, vh);
                                // Renderer Selection Policy（skia → vello の一方向 fallback、
                                // issue #801/#802）越しに初期化し、対応する Raster スレッドを
                                // 起動する（move-after-creation はどちらの経路でも内部で行う）。
                                raster = init_and_spawn_raster(&window, scale);
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
                            let (vw, vh) = effective_insets(&app, width, height)
                                .layout_viewport(width, height, scale);
                            tree.set_viewport(vw, vh);
                        }
                        SurfaceLifecycleAction::Quit => quit = true,
                        SurfaceLifecycleAction::NoOp => {}
                    }
                }
            }
        });

        // 単調増加クロック。スクロールのリリース速度推定（指サンプルのタイムスタンプ）と
        // 下の `tree.render` の両方がこれを共有する（フレーム内で同一時刻とみなす）。
        let timestamp_ms = start.elapsed().as_secs_f64() * 1000.0;
        process_touch_input(&app, &mut tree, &mut touch_scroll, timestamp_ms);
        sync_ime(
            &app,
            &mut tree,
            &mut ime_state,
            &mut ime_target,
            &mut ime_keyboard_shown,
        );

        if let Some(rt) = raster.as_ref() {
            // 単調増加クロックでレイアウトとカーソル点滅を駆動し、lower した
            // シーンを提示する（`hayate-adapter-web` の `render` に対応）。
            let _ = tree.render(timestamp_ms);
            // render() が捕捉した保持シーン + frame_layers / frame_layer_dirty / chrome_dirty を
            // owned handoff にして Raster スレッドへ送る（#635）。UI スレッドは raster を待たず、
            // 続けて入力処理・次フレーム生成へ進める（ADR-0128）。
            let _ = rt.send(frame_handoff(&tree));
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
/// diff/apply ロジックは core 所有の [`hayate_core::element::ime_reconcile`] にある。
/// このラッパーは `android-activity` のテキスト入力 API への薄いグルー。
/// IME 状態を core と同期し、編集入力 wake が起きたかを返す（ADR-0126）。focus 対象の変化や
/// 新規 IME アクション適用は idle の on-demand ループを冷間始動すべき入力到着なので、呼び元は
/// 戻り値で `request_wake` する。
pub(crate) fn sync_ime(
    app: &AndroidApp,
    tree: &mut ElementTree,
    prev: &mut TextInputState,
    prev_target: &mut Option<ElementId>,
    keyboard_shown: &mut bool,
) -> bool {
    // 抽象を通した表示制御。core が編集可否でゲートし、bridge が
    // キーボードを上げ下げする。
    let mut bridge = AndroidImeBridge::new(app, keyboard_shown);
    tree.drive_ime(&mut bridge);

    let target = tree.focused_text_input();
    let mut woke = false;
    if *prev_target != target {
        *prev_target = target;
        // 新規フォーカスは空のベースラインバッファから始める。
        *prev = TextInputState::default();
        woke = true;
    }

    let Some(target) = target else {
        return woke;
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
        // キャレット/選択を確定テキスト座標へ写像してコアへ反映する。これが無いと
        // preedit/確定が常に末尾へ落ちる（ADR-0094: 末尾キャレット前提の解消）。
        selection: Some(TextSpan {
            start: state.selection.start,
            end: state.selection.end,
        }),
    };

    if next != *prev {
        for action in translate_text_input(prev, &next) {
            apply_ime_action(tree, target, &action);
        }
        *prev = next;
        woke = true;
    }
    woke
}

/// 保留中の `MotionEvent` を捌き、`tree` の座標ベースのポインタ API を駆動する。
///
/// 単一ポインタのタップ/ドラッグのみ（ADR-0082）。マルチタッチジェスチャは対象外。
/// イベントごとの計算はホストでテスト可能な [`translate_touch`] と
/// [`TouchScrollState`]（`touch_scroll` モジュール）にあり、このラッパーは薄い
/// NDK グルー。scroll-view 上のドラッグ→スクロール（slop 判定・ラバーバンド・
/// リリース慣性の起動）は `state` が Web アダプタと同じ配線パターンで駆動する
/// （ADR-0082、`hayate_core::scroll` の純物理・純判定を消費）。
/// 保留中の `MotionEvent` を排出して `tree` のポインタ API を駆動する。少なくとも 1 つの
/// タッチアクションをディスパッチしたら `true` を返す（ADR-0126 の入力 wake 源 — idle の
/// on-demand ループを冷間始動するため、呼び元はこの戻り値で `request_wake` する）。
pub(crate) fn process_touch_input(
    app: &AndroidApp,
    tree: &mut ElementTree,
    scroll: &mut TouchScrollState,
    now_ms: f64,
) -> bool {
    let mut iter = match app.input_events_iter() {
        Ok(iter) => iter,
        Err(err) => {
            log::error!("hayate-adapter-android: input_events_iter failed: {err}");
            return false;
        }
    };

    // タッチ座標は物理pxで届くが、レイアウト/ヒットテストは論理px空間（`safe_viewport` と同じ
    // content_scale）で動く。揃えないと高密度端末でタップ位置がずれる。
    let content_scale = crate::surface_lifecycle::content_scale(app);
    // b2（edge-to-edge, issue #794・ADR-0144）: 描画は安全領域インセット分だけ内側へずれているので、
    // タッチもウィンドウ座標から左/上インセットを差し引いてから論理px化する。Kotlin 側の
    // MotionEvent 平行移動（旧 offsetLocation）は撤去し、補正を Rust 側へ一本化した。
    let insets = crate::safe_area::pushed_insets().unwrap_or_default();

    let mut dispatched = false;
    loop {
        let read = iter.next(|event| {
            if let InputEvent::MotionEvent(motion) = event {
                if let Some(action) = motion_action_to_touch(motion.action()) {
                    let pointer = motion.pointer_at_index(motion.pointer_index());
                    let (cx, cy) = insets.correct_touch(pointer.x(), pointer.y());
                    let x = cx / content_scale;
                    let y = cy / content_scale;
                    scroll.apply(tree, translate_touch(action, x, y), now_ms);
                    dispatched = true;
                }
            }
            android_activity::InputStatus::Unhandled
        });
        if !read {
            break;
        }
    }
    dispatched |= scroll.advance_press(tree, now_ms);
    dispatched
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

/// 現在有効な安全領域インセット（edge-to-edge / b2, issue #794・ADR-0144）。
///
/// Kotlin から JNI で push された値（`safe_area::pushed_insets`）を一次ソースにする。まだ一度も
/// push されていない（初回レイアウト前など）ときだけ、`content_rect()` 由来のインセットに
/// フォールバックする——`content_rect()` はフルウィンドウを返す端末があり信頼できないため、
/// あくまで push が来るまでの保険（`MainActivity` は onCreate 後に rootWindowInsets スナップ
/// ショットを push するので、この保険期間は短い）。
pub(crate) fn effective_insets(
    app: &AndroidApp,
    window_width: u32,
    window_height: u32,
) -> SafeAreaInsets {
    if let Some(pushed) = crate::safe_area::pushed_insets() {
        return pushed;
    }
    let rect = app.content_rect();
    SafeAreaInsets::from_content_rect(
        window_width,
        window_height,
        rect.left,
        rect.top,
        rect.right,
        rect.bottom,
    )
}

/// ネイティブウィンドウ全体の物理サイズと安全領域インセットから、レイアウトに渡す論理
/// ビューポートを導く。GPU surface のサイズはウィンドウ全体のままで変えない（b2：edge-to-edge。
/// `window_dimensions` を別途使う）——ここで縮めるのはレイアウトが使うビューポートだけ。
/// 上端がステータスバー裏に潜り最下点がナビゲーションバー裏へずれるバグの修正（issue #794）。
fn safe_viewport(
    app: &AndroidApp,
    window: &ndk::native_window::NativeWindow,
    content_scale: f32,
) -> (f32, f32) {
    let (w, h) = window_dimensions(window.width(), window.height());
    effective_insets(app, w, h).layout_viewport(w, h, content_scale)
}

pub(crate) async fn init_gpu_surface(
    window: &ndk::native_window::NativeWindow,
    content_scale: f32,
) -> Result<GpuSurface, String> {
    let (width, height) = window_dimensions(window.width(), window.height());

    // 描画バックエンド（Vulkan/GL）・AA 方式（Area/MSAA8/MSAA16）は intent extra 由来の
    // ランタイム選択（#795・ADR-0145）。既定は Vulkan + Area（名前付き定数）。Nothing Phone 3a
    // （Adreno 710）の描画破綻切り分けを再ビルドなしで回すためのスイッチ。
    let backend = crate::render_config::effective_backend();
    let aa = crate::render_config::effective_aa();
    log::info!(
        "hayate-adapter-android: render config — backend={} aa={}",
        backend.as_str(),
        aa.as_str()
    );

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: backend.to_wgpu(),
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

    // 選択された GPU アダプタ情報（名前・ドライバ）を logcat に出す（#795）。実機実験の記録と
    // 上流報告（wgpu/naga）にそのまま使えるようにする。
    let info = adapter.get_info();
    log::info!(
        "hayate-adapter-android: GPU adapter — name={} backend={:?} driver={} driver_info={}",
        info.name,
        info.backend,
        info.driver,
        info.driver_info
    );

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
    // UI 色は CSS hex 由来の sRGB エンコード済みバイト値をそのまま格納する規約（Web と同じ、
    // ADR-0125 レイヤ合成パス）。`get_default_config` は sRGB 対応 swapchain フォーマットを
    // 優先して返すため、そのまま使うと GPU が store 時にもう一段 sRGB エンコードを掛けてしまい
    // 色が白っぽく退色する（二重ガンマ）。Web の canvas 既定フォーマットは非 sRGB（バイト値
    // そのまま格納）なので、Android でも対応していれば非 sRGB を明示選択して揃える。
    let capabilities = surface.get_capabilities(&adapter);
    surface_config.format = crate::surface_lifecycle::prefer_non_srgb_format(
        surface_config.format,
        &capabilities.formats,
    );
    surface_config.usage |= wgpu::TextureUsages::RENDER_ATTACHMENT;
    surface.configure(&device, &surface_config);

    let target_view = create_target_view(&device, width, height);
    let blitter = create_blitter(&device, surface_config.format);
    // AA 方式を注入して構築する（#795）。選んだ方式のパイプラインだけをコンパイルし、warmup /
    // render も同じ config で回す（web/iOS は既定 Area のまま）。
    let mut scene_renderer = VelloSceneRenderer::new_with_options(&device, None, aa)?;
    // init 直後・最初の実アプリフレーム前に vello パイプラインを warmup する（#644/ADR-0130a）。
    // warmup 失敗は boot を落とさず、初回フレームで従来どおりコンパイル遅延が出るだけで続行する
    // （Web の SelectedBackend::init と同じ扱い、#687）。
    if let Err(err) = scene_renderer.warmup(&device, &queue) {
        log::warn!("hayate-adapter-android: vello warmup skipped: {err}");
    }

    Ok(GpuSurface {
        device,
        queue,
        surface,
        surface_config,
        target_view,
        blitter,
        width,
        height,
        planner: PresentPlanner::new(),
        scene_renderer,
        content_scale,
    })
}

impl GpuSurface {
    /// 1 フレームの提示（#632・#687 で Web の単一 root 経路と揃えた）。`layers` / `layer_dirty` /
    /// `transform_dirty` / `chrome_dirty` は core が `render()` で捕捉した `frame_layers()` /
    /// `frame_layer_dirty()` / `frame_layer_transform_dirty()` / `frame_layer_chrome_dirty()`。
    ///
    /// per-layer quad 合成を持たないため、3 つの dirty 集合すべてを保守的に union して raster
    /// トリガとする（`canvas.rs::present_frame` の単一 root 分岐と同じ理由）。`plan().needs_raster`
    /// なら `VelloSceneRenderer::render_scene` でシーン全体を 1 回再描画してから present し、
    /// `note_full_raster` で記録する。dirty でなければ raster も present も呼ばない
    /// （composite-only フレームは vello を一切起動しない）。
    pub(crate) fn render_frame(
        &mut self,
        scene: &SceneGraph,
        layers: &[ElementId],
        layer_dirty: &HashSet<ElementId>,
        transform_dirty: &HashSet<ElementId>,
        chrome_dirty: &HashSet<ElementId>,
    ) -> Result<(), String> {
        let mut raster_trigger: HashSet<ElementId> = layer_dirty.clone();
        raster_trigger.extend(transform_dirty.iter().copied());
        raster_trigger.extend(chrome_dirty.iter().copied());
        let plan = self.planner.plan(layers, &raster_trigger);
        if !plan.needs_raster {
            return Ok(());
        }

        let target = VelloRenderTarget {
            device: &self.device,
            queue: &self.queue,
            target_view: &self.target_view,
            width: self.width,
            height: self.height,
        };
        // b2（edge-to-edge, issue #794・ADR-0144）: GPU ターゲットはフルウィンドウのまま、シーンを
        // 安全領域インセット分だけ右下へ平行移動する。バー裏の空き領域は vello が base_color
        // （= ルート背景色 CLEAR_COLOR）でターゲット全面をクリアするのでそのまま塗られる。JNI push が
        // まだ無いフレームは (0,0)（フルウィンドウ描画）で、直後の rootWindowInsets スナップショット
        // push で安全領域へ収まる。
        let (origin_x, origin_y) = crate::safe_area::pushed_insets()
            .map(|insets| insets.scene_origin(self.content_scale))
            .unwrap_or((0.0, 0.0));
        self.scene_renderer.render_scene_with_offset(
            scene,
            &target,
            CLEAR_COLOR,
            self.content_scale,
            origin_x,
            origin_y,
        )?;
        self.present_target()?;
        self.planner.note_full_raster(layers);
        Ok(())
    }

    /// オフスクリーンの `target_view` をサーフェスへ blit して present する（Web の
    /// `VelloSurfaceHost::present_target` と同型、#687）。
    fn present_target(&mut self) -> Result<(), String> {
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
                label: Some("hayate_blit"),
            });
        self.blitter
            .copy(&self.device, &mut encoder, &self.target_view, &surface_view);
        self.queue.submit(std::iter::once(encoder.finish()));
        surface_texture.present();
        Ok(())
    }

    pub(crate) fn resize(&mut self, width: u32, height: u32, content_scale: f32) {
        let content_scale = content_scale.max(1.0);
        if width == 0
            || height == 0
            || (width == self.width && height == self.height && content_scale == self.content_scale)
        {
            return;
        }
        self.width = width;
        self.height = height;
        self.content_scale = content_scale;
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
        // オフスクリーンターゲットはサーフェスサイズなので作り直し＝キャッシュ面を失うので
        // invalidate する（invalidate しないと clean フレームが古いサイズの内容を blit し続ける）。
        self.target_view = create_target_view(&self.device, width, height);
        self.planner.invalidate();
    }
}

/// UI スレッドが握る Raster スレッドのハンドル（ADR-0128・#635）。`GpuSurface`（wgpu surface +
/// cache + compositor）を Raster スレッドへ move して所有させ、UI スレッドは [`RasterCommand`] を
/// 送るだけ（raster/composite は UI スレッド外で走る）。surface 作成は window ハンドルを要するため
/// UI スレッドで行い、生成後の surface をここで move する（move-after-creation。window をスレッド
/// 境界へ送らない）。TerminateWindow / reload では `Option` を `None` にして drop → 安全に join。
pub(crate) type RasterHandle = RasterThread<RasterCommand>;

/// 生成済み `GpuSurface` を所有する Raster スレッドを起動する（#635）。sink は Frame を present、
/// Resize を surface 再構成へ写す。surface が失われている間（SurfaceLost〜RebuildSurface）は
/// present をスキップする——このデモ経路では surface 破棄 = スレッドごと drop なので、Lost/Rebuild
/// は tsubame 経路（surface を握ったまま再アタッチする将来拡張）向けに状態だけ持つ。
pub(crate) fn spawn_raster_thread(mut surface: GpuSurface) -> RasterHandle {
    let mut surface_ready = true;
    RasterThread::spawn(move |cmd: RasterCommand| match cmd {
        RasterCommand::Frame(handoff) => {
            if !surface_ready {
                return; // surface 無し＝present をスキップ（次の RebuildSurface で復帰）。
            }
            let RasterHandoff {
                scene,
                layers,
                layer_dirty,
                transform_dirty,
                chrome_dirty,
                scroll_inputs: _,
            } = handoff;
            if let Err(err) = surface.render_frame(
                &scene,
                &layers,
                &layer_dirty,
                &transform_dirty,
                &chrome_dirty,
            ) {
                log::error!("hayate-adapter-android: raster-thread render failed: {err}");
            }
        }
        RasterCommand::Resize {
            width,
            height,
            content_scale,
        } => surface.resize(width, height, content_scale),
        RasterCommand::SurfaceLost => surface_ready = false,
        RasterCommand::RebuildSurface => surface_ready = true,
    })
}

/// 生成済み `SkiaGpuSurface`（CPU raster + ANativeWindow 直接 present）を所有する Raster
/// スレッドを起動する（issue #802）。`spawn_raster_thread`（vello/wgpu）と対で、同じ
/// `RasterCommand` チャネル契約を共有する——呼び出し側（`init_and_spawn_raster`）はどちらが
/// 返っても同じ `RasterHandle` として扱える。
pub(crate) fn spawn_skia_raster_thread(
    mut surface: crate::skia_window::SkiaGpuSurface,
) -> RasterHandle {
    let mut surface_ready = true;
    let mut terminal_failure = false;
    RasterThread::spawn(move |cmd: RasterCommand| match cmd {
        RasterCommand::Frame(handoff) => {
            if !surface_ready || terminal_failure {
                return; // surface 無し＝present をスキップ（次の RebuildSurface で復帰）。
            }
            let RasterHandoff {
                scene,
                layers,
                layer_dirty,
                transform_dirty,
                chrome_dirty,
                scroll_inputs,
            } = handoff;
            if let Err(err) = surface.render_frame(
                &scene,
                &layers,
                &layer_dirty,
                &transform_dirty,
                &chrome_dirty,
                &scroll_inputs,
            ) {
                log_terminal_skia_failure(&err);
                terminal_failure = true;
            }
        }
        RasterCommand::Resize {
            width,
            height,
            content_scale,
        } => surface.resize(width, height, content_scale),
        RasterCommand::SurfaceLost => surface_ready = false,
        RasterCommand::RebuildSurface => surface_ready = true,
    })
}

/// 生成済み `SkiaGlSurface`（Ganesh GL/EGL raster + `eglSwapBuffers` present）を所有する
/// Raster スレッドを起動する（issue #803）。`spawn_skia_raster_thread`（CPU raster）と対で、
/// 同じ `RasterCommand` チャネル契約を共有する。EGL コンテキストはこのスレッドに束縛される
/// （`SkiaGlSurface::render_frame` が初回フレームで make-current する）。
pub(crate) fn spawn_skia_gl_raster_thread(
    mut surface: crate::skia_gl_window::SkiaGlSurface,
) -> RasterHandle {
    let mut surface_ready = true;
    let mut terminal_failure = false;
    RasterThread::spawn(move |cmd: RasterCommand| match cmd {
        RasterCommand::Frame(handoff) => {
            if !surface_ready || terminal_failure {
                return; // surface 無し＝present をスキップ（次の RebuildSurface で復帰）。
            }
            let RasterHandoff {
                scene,
                layers,
                layer_dirty,
                transform_dirty,
                chrome_dirty,
                scroll_inputs,
            } = handoff;
            if let Err(err) = surface.render_frame(
                &scene,
                &layers,
                &layer_dirty,
                &transform_dirty,
                &chrome_dirty,
                &scroll_inputs,
            ) {
                log_terminal_skia_failure(&err);
                terminal_failure = true;
            }
        }
        RasterCommand::Resize {
            width,
            height,
            content_scale,
        } => surface.resize(width, height, content_scale),
        RasterCommand::SurfaceLost => surface_ready = false,
        RasterCommand::RebuildSurface => surface_ready = true,
    })
}

fn classify_skia_runtime_failure(
    error: &str,
) -> hayate_app_host::renderer_selection::RendererSelectionReason {
    use hayate_app_host::renderer_selection::RendererSelectionReason;

    let message = error.to_ascii_lowercase();
    if message.contains("surface")
        || message.contains("context")
        || message.contains("egl")
        || message.contains("anativewindow")
    {
        RendererSelectionReason::SurfaceLost
    } else {
        RendererSelectionReason::RendererInitFailed
    }
}

fn log_terminal_skia_failure(error: &str) {
    let reason = classify_skia_runtime_failure(error);
    log::error!("terminal scene renderer failure: skia ({reason:?}): {error}");
}

/// Renderer Selection Policy（issue #801/#802、spec §4 REND-15）越しにレンダラを初期化し、
/// 対応する Raster スレッドを起動する。既定順序は skia → vello の一方向 fallback
/// （[`hayate_app_host::renderer_selection::NATIVE_RENDERER_ORDER`]）。intent extra
/// （`hayate.renderer`、[`crate::renderer_config::forced_renderer`]）で再ビルドなしに強制指定
/// できる。選択・却下・失敗はどれも logcat に出す（`RendererSelectionReason` 語彙、
/// `hayate_app_host::render_host::RenderHost::init_with_policy` と同じ文言——Android は
/// `RasterThread` 所有の都合でこの薄いオーケストレーションを自前で持つ）。
///
/// 全滅時は `None`（呼び元は surface 無しで続行する——既存の GPU init 失敗ハンドリングと同じ
/// 「boot は落とさない」扱い、#795）。
pub(crate) fn init_and_spawn_raster(
    window: &ndk::native_window::NativeWindow,
    content_scale: f32,
) -> Option<RasterHandle> {
    use hayate_app_host::renderer_selection::{
        native_renderer_selection_policy, RendererCapabilities, SceneRendererKind,
    };

    let policy = native_renderer_selection_policy(
        crate::renderer_config::VELLO_LINKED,
        crate::renderer_config::forced_renderer(),
    );
    // ネイティブでは GPU（wgpu adapter）の有無は init を試すまで分からないため常に true を渡し、
    // 失敗は下の一方向 fallback ループが拾う（desktop の RenderHostSurface::init と同じ規約）。
    let plan = policy.choose(RendererCapabilities {
        webgpu_available: true,
    });

    for rejection in plan.rejected() {
        log::info!(
            "scene renderer rejected: {} ({:?})",
            rejection.renderer.name(),
            rejection.reason
        );
    }

    for &kind in plan.attempt_order() {
        match kind {
            SceneRendererKind::Vello => {
                match pollster::block_on(init_gpu_surface(window, content_scale)) {
                    Ok(surface) => {
                        log::info!(
                            "selected scene renderer: {}",
                            SceneRendererKind::Vello.name()
                        );
                        return Some(spawn_raster_thread(surface));
                    }
                    Err(err) => {
                        // "GPU init failed" は #795 の既存契約テストが固定する文言——GL で
                        // adapter/device 取得に失敗する端末でも boot は落ちず、ここで次候補
                        // （skia）へ一方向 fallback する。
                        log::warn!("hayate-adapter-android: GPU init failed: {err}");
                    }
                }
            }
            SceneRendererKind::Skia => {
                // skia 内 surface（raster/GL）は intent extra（`hayate.skia_surface`）由来の
                // ランタイム選択（issue #803・ADR-0146 §3）。GL（Ganesh/EGL）は EGL 不調端末で
                // 初期化に失敗しうるため、失敗理由をログに残して skia raster へ一方向 fallback
                // する（boot は落とさない——renderer init fallback と同じ姿勢）。
                let surface_kind = crate::renderer_config::effective_skia_surface();
                log::info!(
                    "hayate-adapter-android: skia surface config: {}",
                    surface_kind.as_str()
                );
                if surface_kind == crate::renderer_config::SkiaSurfaceKind::Gl {
                    match crate::skia_gl_window::init_skia_gl_surface(window, content_scale) {
                        Ok(surface) => {
                            log::info!(
                                "selected scene renderer: {}",
                                SceneRendererKind::Skia.name()
                            );
                            log::info!("hayate-adapter-android: skia surface: gl (Ganesh/EGL)");
                            return Some(spawn_skia_gl_raster_thread(surface));
                        }
                        Err(err) => {
                            log::warn!(
                                "hayate-adapter-android: skia GL surface init failed: {err} — \
                                 falling back to skia raster"
                            );
                        }
                    }
                }
                match crate::skia_window::init_skia_surface(window, content_scale) {
                    Ok(surface) => {
                        log::info!(
                            "selected scene renderer: {}",
                            SceneRendererKind::Skia.name()
                        );
                        log::info!("hayate-adapter-android: skia surface: raster (CPU)");
                        return Some(spawn_skia_raster_thread(surface));
                    }
                    Err(err) => {
                        log::warn!("hayate-adapter-android: skia surface init failed: {err}");
                    }
                }
            }
            other => {
                // NATIVE_RENDERER_ORDER は vello/skia のみを含むはずだが、将来の拡張に備えて
                // 明示的に警告する（無音での取りこぼしを防ぐ）。
                log::warn!(
                    "hayate-adapter-android: renderer {} is not wired on Android",
                    other.name()
                );
            }
        }
    }

    log::error!("hayate-adapter-android: no scene renderer could be initialized");
    None
}

/// UI スレッドが握る保持シーンから 1 フレーム分の owned ハンドオフを組む（#635）。scene は境界を
/// 越えて move するので clone（ADR-0128：スレッド境界＝lower 済み SceneGraph の owned スナップショット）。
pub(crate) fn frame_handoff(tree: &ElementTree) -> RasterCommand {
    RasterCommand::Frame(RasterHandoff {
        scene: tree.scene_graph().clone(),
        layers: tree.frame_layers().to_vec(),
        layer_dirty: tree.frame_layer_dirty().clone(),
        transform_dirty: tree.frame_layer_transform_dirty().clone(),
        chrome_dirty: tree.frame_layer_chrome_dirty().clone(),
        scroll_inputs: tree.frame_scroll_compositor_inputs(),
    })
}
