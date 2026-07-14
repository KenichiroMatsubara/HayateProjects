//! Desktop Platform Front（ADR-0118 → issue #801）。
//!
//! winit single crate で macos / windows / linux を windowing / event-loop / GPU surface の
//! 層に畳み、winit window + Render Host（[`hayate_app_host::render_host::RenderHost`]、
//! ADR-0068/0132 の hoist をネイティブへ延長）+ `App Host::tick` 駆動を配線して、
//! 共有 demo fixture（`hayate_demo_fixtures::tasks_tree`）を native window に present する。
//! レンダラは Renderer Selection Policy（spec §4 REND-15）が選ぶ — 既定順序は
//! skia → vello の一方向 init fallback（ADR-0149）、
//! env / CLI フラグ（[`renderer_config`]）で再ビルドなしに強制指定できる。skia raster は
//! softbuffer による wgpu 非依存の CPU present（[`skia_window`]）なので、GPU が使えない
//! 環境でも desktop が起動する。
//! 静的 1 枚を見せる tracer bullet（issue #505）に、winit pointer 入力（`CursorMoved` /
//! `MouseInput`）を Core の座標 pointer dispatch（`PointerKind = Mouse`）へ配線して
//! hover / active / focus を効かせ（issue #506）、さらに winit keyboard 入力
//! （`KeyboardInput` / `ModifiersChanged`）を `key_to_edit_intent` 経由で `apply_edit_intent`
//! と `on_text_input` へ配線して編集（キャレット移動・選択・削除・入力）を効かせる（issue #507）。
//! さらに winit IME 入力（`WindowEvent::Ime`）を `ime_input` 経由で Core 所有の増分 IME
//! モデル（`ImeCommand` → `apply_command` → `apply_ime_action`・ADR-0117）へ配線し、focus/blur に
//! 応じた IME enable/disable と変換候補ウィンドウのキャレット追従（`set_ime_cursor_area`）を
//! Core の `drive_ime`（[`ImeBridge`]）経由で効かせる（issue #508）。

pub mod ime_input;
pub mod keyboard_input;
#[cfg(feature = "backend-vello")]
pub mod pipeline_disk_cache;
pub mod pointer_input;
pub mod renderer_config;
pub mod skia_present;
pub mod skia_window;
#[cfg(feature = "backend-vello")]
pub mod vello_window;

use std::sync::Arc;
use std::time::Instant;

use anyhow::{anyhow, Error};
use hayate_app_host::render_host::{RenderHost, RendererInit, SceneRenderer};
use hayate_app_host::renderer_selection::{
    native_renderer_selection_policy, RendererCapabilities, RendererSelectionReason,
    SceneRendererKind,
};
use hayate_app_host::{AppHost, PresentTarget};
use hayate_core::{ImeBridge, ImeBuffer, ImePresentation, SceneGraph, Surface, ViewportMetrics};
use hayate_demo_fixtures::tasks_tree;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::ModifiersState;
use winit::window::{Window, WindowId};

use skia_window::SkiaWindowRenderer;

/// native window のタイトル（選択レンダラ名は起動後に [`window_title_for`] で付く）。
pub const WINDOW_TITLE: &str = "Hayate — Tasks";

/// 選択されたレンダラを含む window タイトル。レンダラは Renderer Selection Policy が
/// 実行時に決めるため、タイトルの確定も起動後になる。
pub fn window_title_for(kind: SceneRendererKind) -> String {
    format!("{WINDOW_TITLE} ({})", kind.name())
}

/// 既定のウィンドウサイズ（論理 px）。値の調整は人力フォローアップ（ADR-0118）。
pub const DEFAULT_WINDOW_SIZE: (u32, u32) = (1024, 1080);

/// surface の clear color（vello `base_color`）。demo fixture の背景 `#f1ede3` に合わせ、
/// content がカバーしない余白も意図した色で塗る。値の調整は人力フォローアップ（ADR-0118）。
pub const CLEAR_COLOR: [f32; 4] = [0.945, 0.929, 0.890, 1.0];

/// 起動時の winit `WindowAttributes`。**非表示（`visible: false`）で作るのが要点**で、初回
/// フレームの present 後に `RedrawRequested` ハンドラが一度だけ `set_visible(true)` する。
/// 可視のまま作ると、GPU 初期化（wgpu adapter/device・vello シェーダコンパイル）と初回
/// フレームが終わるまで OS の未描画ウィンドウ（暗い画面）が約 1 秒見えてしまう。
pub fn initial_window_attributes() -> winit::window::WindowAttributes {
    Window::default_attributes()
        .with_title(WINDOW_TITLE)
        .with_inner_size(LogicalSize::new(DEFAULT_WINDOW_SIZE.0, DEFAULT_WINDOW_SIZE.1))
        .with_visible(false)
}

/// winit の物理サイズ・`scale_factor` を Core の [`ViewportMetrics`] に橋渡しする（ADR-0080）。
///
/// `scale_factor` を 1.0 に潰さず `content_scale` へ素通しするのが HiDPI でぼやけない要点で、
/// 論理ビューポート（`set_viewport` 入力）とバッキングストア（wgpu surface 設定）を同じ規約で導く。
pub fn viewport_metrics(physical_width: u32, physical_height: u32, scale_factor: f64) -> ViewportMetrics {
    ViewportMetrics::from_physical_size(physical_width as i32, physical_height as i32, scale_factor as f32)
}

/// [`hayate_core::Surface`] の desktop 実装 — winit `Window` の薄い clone ハンドル。
/// `RenderHost` はレンダラー初期化を試すたびにこれを複製して [`DesktopRendererInit`] へ
/// 渡し、実行時フォールバックでの再サイズ確認（物理 px）に使う（web の
/// `WebCanvasSurface(HtmlCanvasElement)` と同型、ADR-0132 スライス3）。
#[derive(Clone)]
pub struct DesktopWindowSurface {
    window: Arc<Window>,
}

impl Surface for DesktopWindowSurface {
    fn width(&self) -> u32 {
        self.window.inner_size().width
    }

    fn height(&self) -> u32 {
        self.window.inner_size().height
    }
}

/// レンダラーがこのビルドにリンクされていないときの init エラー文言。
/// [`classify_error_message`] が `DisabledByPolicy` へ分類する（コンパイル構成の帰結で
/// あって実行時失敗ではないため、runtime fallback 対象にならない語彙を選ぶ）。
const NOT_LINKED_MSG: &str = "renderer not linked into this desktop build";

/// desktop アダプタの init エラー分類（ADR-0132: `classify_init_error` は adapter 個別
/// 実装のまま共有しない）。エラー文字列だけに依存する純関数で、実 GPU なしでテストする。
/// swapchain の喪失・陳腐化だけを `SurfaceLost` とし、それ以外の初期化・描画失敗は
/// `RendererInitFailed`（どちらも runtime fallback 対象の語彙・ADR-0050）。
pub fn classify_error_message(message: &str) -> RendererSelectionReason {
    if message.contains(NOT_LINKED_MSG) {
        RendererSelectionReason::DisabledByPolicy
    } else if message.contains("surface lost") || message.contains("Outdated") {
        RendererSelectionReason::SurfaceLost
    } else {
        RendererSelectionReason::RendererInitFailed
    }
}

/// desktop アダプタによる [`RendererInit`] 実装（issue #801）。`RenderHost` はこれ越しに
/// しか winit / wgpu / softbuffer の具体資源へ触れない。vello はプランの先頭（preferred
/// default）、skia raster が一方向 fallback 先（spec §4 REND-15）。
pub struct DesktopRendererInit;

impl DesktopRendererInit {
    fn build(
        &self,
        kind: SceneRendererKind,
        surface: &DesktopWindowSurface,
    ) -> Result<Box<dyn SceneRenderer>, Error> {
        match kind {
            #[cfg(feature = "backend-vello")]
            SceneRendererKind::Vello => Ok(Box::new(vello_window::VelloWindowRenderer::new_sync(
                surface.window.clone(),
            )?)),
            SceneRendererKind::Skia => {
                Ok(Box::new(SkiaWindowRenderer::new(surface.window.clone())?))
            }
            other => Err(anyhow!("{NOT_LINKED_MSG}: {}", other.name())),
        }
    }
}

impl RendererInit<DesktopWindowSurface> for DesktopRendererInit {
    async fn try_init(
        &self,
        kind: SceneRendererKind,
        surface: DesktopWindowSurface,
    ) -> Result<Box<dyn SceneRenderer>, Error> {
        match kind {
            // 非同期 init は vello（wgpu adapter/device 要求）だけが本来の async。
            #[cfg(feature = "backend-vello")]
            SceneRendererKind::Vello => Ok(Box::new(
                vello_window::VelloWindowRenderer::new_async(surface.window.clone()).await?,
            )),
            _ => self.build(kind, &surface),
        }
    }

    fn try_init_sync_for_fallback(
        &self,
        kind: SceneRendererKind,
        surface: DesktopWindowSurface,
    ) -> Result<Box<dyn SceneRenderer>, Error> {
        self.build(kind, &surface)
    }

    fn classify_init_error(
        &self,
        _kind: SceneRendererKind,
        error: &Error,
    ) -> RendererSelectionReason {
        classify_error_message(&error.to_string())
    }
}

/// Render Host を [`PresentTarget`] として App Host に差し込む desktop の提示面
/// （issue #801）。毎フレームの `SceneGraph` を `RenderHost::render_scene` へ流すだけで、
/// 実行時失敗時の skia raster への一方向 fallback は `RenderHost` が処理する（ADR-0050）。
pub struct RenderHostSurface {
    host: RenderHost<DesktopWindowSurface, DesktopRendererInit>,
}

impl RenderHostSurface {
    /// Renderer Selection Policy を通してレンダラを選び初期化する。`forced` は env / CLI
    /// からの強制指定（[`renderer_config`]）。選択・却下は `RenderHost` が
    /// `RendererSelectionReason` 語彙で stderr（`log`）に出す。
    pub fn init(
        window: Arc<Window>,
        forced: Option<SceneRendererKind>,
    ) -> Result<Self, Error> {
        let policy = native_renderer_selection_policy(renderer_config::VELLO_LINKED, forced);
        // ネイティブでは GPU（wgpu adapter）の有無は init を試すまで分からないため
        // capability は常に true を渡し、失敗は init フェーズの一方向 fallback が拾う。
        let capabilities = RendererCapabilities {
            webgpu_available: true,
        };
        let host = pollster::block_on(RenderHost::init_with_policy(
            DesktopWindowSurface { window },
            policy,
            capabilities,
            DesktopRendererInit,
        ))?;
        Ok(Self { host })
    }

    /// 現在アクティブなレンダラ種別（demo fixture のバッジ・window タイトル用）。
    pub fn renderer_kind(&self) -> SceneRendererKind {
        self.host.kind()
    }

    /// winit リサイズの反映。物理 px と HiDPI 係数を `RenderHost` 経由でレンダラへ渡す。
    pub fn resize(&mut self, metrics: ViewportMetrics) {
        let (width, height) = metrics.buffer_size();
        self.host.resize(width, height, metrics.content_scale);
    }
}

impl PresentTarget for RenderHostSurface {
    fn present(&mut self, scene: &SceneGraph) {
        if let Err(e) = self.host.render_scene(scene, CLEAR_COLOR) {
            log::error!("render_scene failed (no further fallback): {e}");
        }
    }
}

/// Core の [`ImePresentation`] を winit `Window` の IME 状態へ反映する [`ImeBridge`]
/// （ADR-0069・issue #508）。編集可否（キーボードを*出すか*）と候補ウィンドウの位置
/// （*どこに*）は Core の `drive_ime` が一元的に決め、本アダプタは winit へ転写するだけ
/// （プラットフォーム個別の振る舞い乖離を防ぐ）。
struct WinitImeBridge<'a> {
    window: &'a Window,
}

impl ImeBridge for WinitImeBridge<'_> {
    fn present(&mut self, presentation: ImePresentation) {
        match presentation {
            // text-input がフォーカスされている: IME を許可し、候補窓をキャレットへ向ける。
            ImePresentation::Shown { bounds } => {
                self.window.set_ime_allowed(true);
                let (position, size) = ime_input::ime_cursor_area(bounds);
                self.window.set_ime_cursor_area(position, size);
            }
            // 編集可能要素が非フォーカス: IME を無効化する（変換中バッファは winit が破棄）。
            ImePresentation::Hidden => self.window.set_ime_allowed(false),
        }
    }
}

/// winit `ApplicationHandler`。OS の窓を開き、共有 fixture を載せた `App Host` を駆動する
/// Platform Front（ADR-0118）。フレーム継続判定は App Host が所有し、ここはスケジューリング
/// （`request_redraw` 配線・`RedrawRequested` → `tick`・resize 反映）だけを担う。
#[derive(Default)]
pub struct DesktopApp {
    window: Option<Arc<Window>>,
    app_host: Option<AppHost<RenderHostSurface>>,
    start: Option<Instant>,
    /// 初回フレーム present 済みでウィンドウを可視化したか。非表示で作成したウィンドウを
    /// 最初の `tick`（= 初回 present）直後に一度だけ `set_visible(true)` するためのラッチ。
    shown: bool,
    /// 直近の `CursorMoved` 由来の論理（レイアウト）ポインタ座標。winit の `MouseInput` は
    /// 座標を運ばないので、press/release dispatch にこれを載せる。
    last_pointer_pos: (f32, f32),
    /// 直近の `ModifiersChanged` 由来の修飾キー状態。winit の `KeyboardInput` は修飾を
    /// 個別イベントで運ぶので、ここに持ち回って各キー押下の keymap 判定に載せる。
    modifiers: ModifiersState,
    /// IME の増分入力モデル（[`ImeCommand`](hayate_core::ImeCommand) → `apply_command`）が
    /// フレームをまたいで保持するローカルバッファ（ADR-0117）。winit の `Ime` イベントを
    /// ここへ畳んで最小のコア編集に変換する。
    ime_buffer: ImeBuffer,
}

impl DesktopApp {
    /// 経過時間 [ms]。`tick(timestamp_ms)` へ渡す単調増加のフレームタイムスタンプ。
    fn timestamp_ms(&self) -> f64 {
        self.start
            .map(|t| t.elapsed().as_secs_f64() * 1000.0)
            .unwrap_or(0.0)
    }
}

impl ApplicationHandler for DesktopApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let window = match event_loop.create_window(initial_window_attributes()) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                log::error!("create_window: {e}");
                event_loop.exit();
                return;
            }
        };

        let size = window.inner_size();
        let metrics = viewport_metrics(size.width, size.height, window.scale_factor());

        // レンダラ強制指定（env / CLI・再ビルド不要、ADR-0138/0140/0145 の流儀）。
        let forced = renderer_config::forced_renderer(
            std::env::args().skip(1),
            std::env::var(renderer_config::RENDERER_ENV_VAR).ok().as_deref(),
        );
        if let Some(kind) = forced {
            log::info!(
                "renderer forced via {} / {}: {}",
                renderer_config::RENDERER_CLI_FLAG,
                renderer_config::RENDERER_ENV_VAR,
                kind.name()
            );
        }

        // Renderer Selection Policy（skia → vello の一方向 init fallback、spec §4 REND-15）を
        // 通してレンダラを初期化する。選択・却下理由は RenderHost が stderr ログに出す。
        let surface = match RenderHostSurface::init(window.clone(), forced) {
            Ok(s) => s,
            Err(e) => {
                log::error!("no scene renderer could be initialized: {e}");
                event_loop.exit();
                return;
            }
        };
        let renderer_kind = surface.renderer_kind();
        window.set_title(&window_title_for(renderer_kind));

        // request_redraw は App Host の唯一の wake 入口（ADR-0117）。winit window に配線する。
        let redraw_window = window.clone();
        let mut app_host = AppHost::new(surface, Box::new(move || redraw_window.request_redraw()));
        // 共有 demo fixture を App Host の tree に載せる（consumer は mount しない・ADR-0118）。
        // AppBar のバッジには選択されたレンダラ名を出す。
        *app_host.tree_mut() = tasks_tree(renderer_kind.name());
        let (vw, vh) = metrics.viewport_size();
        app_host.tree_mut().set_viewport(vw, vh);

        self.start = Some(Instant::now());
        self.window = Some(window.clone());
        self.app_host = Some(app_host);
        // 初回フレームを要求する（以降は静的なので App Host が idle に落ちる）。
        window.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let (Some(window), Some(app_host)) =
                    (self.window.as_ref(), self.app_host.as_mut())
                {
                    let metrics = viewport_metrics(size.width, size.height, window.scale_factor());
                    // wgpu surface を再 configure し、論理ビューポートを set_viewport に反映。
                    app_host.surface_mut().resize(metrics);
                    let (vw, vh) = metrics.viewport_size();
                    app_host.tree_mut().set_viewport(vw, vh);
                    window.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                let ts = self.timestamp_ms();
                if let (Some(window), Some(app_host)) =
                    (self.window.as_ref(), self.app_host.as_mut())
                {
                    app_host.tick(ts);
                    // 初回フレームを present し終えてから可視化する（暗転防止・
                    // `initial_window_attributes` 参照）。直後にもう 1 フレーム要求するのは、
                    // 非表示中の swapchain が present を `Occluded` 等で落とすプラットフォーム
                    // でも、可視化後に必ず 1 枚描き直して空ウィンドウを残さないため。
                    if !self.shown {
                        self.shown = true;
                        window.set_visible(true);
                        window.request_redraw();
                    }
                    // tick 後（レイアウト確定後）に IME を一度駆動する。focus/blur の enable/
                    // disable と、候補ウィンドウのキャレット追従（`set_ime_cursor_area`）を
                    // Core の `drive_ime` が決め、`WinitImeBridge` が winit へ転写する
                    // （issue #508）。全入力は request_redraw でここに合流するので、これが
                    // IME 同期の唯一地点になる。
                    let mut bridge = WinitImeBridge { window };
                    app_host.tree().drive_ime(&mut bridge);
                }
            }
            // ポインタ入力（winit → Core の座標 pointer dispatch、`PointerKind = Mouse`・
            // issue #506）。純粋写像 seam（`pointer_input`）で dispatch 引数へ変換し、tree へ
            // 適用したうえで `request_redraw` でフレームを起こす（入力到着が唯一の wake 入口・
            // ADR-0117）。keyboard / IME は別スライス。
            WindowEvent::CursorMoved { position, .. } => {
                if let (Some(window), Some(app_host)) =
                    (self.window.as_ref(), self.app_host.as_mut())
                {
                    let dispatch =
                        pointer_input::cursor_moved_to_dispatch(position, window.scale_factor());
                    if let pointer_input::PointerDispatch::Move { x, y } = dispatch {
                        self.last_pointer_pos = (x, y);
                    }
                    pointer_input::apply_pointer_dispatch(app_host.tree_mut(), dispatch);
                    window.request_redraw();
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if let (Some(window), Some(app_host)) =
                    (self.window.as_ref(), self.app_host.as_mut())
                {
                    if let Some(dispatch) =
                        pointer_input::mouse_input_to_dispatch(state, button, self.last_pointer_pos)
                    {
                        pointer_input::apply_pointer_dispatch(app_host.tree_mut(), dispatch);
                        window.request_redraw();
                    }
                }
            }
            // 修飾キー状態は個別イベントで届く。次の KeyboardInput の keymap 判定のため
            // 持ち回る（winit の `KeyboardInput` は修飾を運ばない）。
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }
            // キーボード入力（winit → Core の編集シーム・issue #507）。press のみを純粋
            // 写像 seam（`keyboard_input`）へ通し、編集コマンドは focus 中 text-input に
            // `apply_edit_intent`、印字可能文字は `on_text_input` で適用したうえで
            // `request_redraw` でフレームを起こす（入力到着が唯一の wake 入口・ADR-0117）。
            // repeat も auto-repeat 編集のため受ける。IME は別スライス。
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == winit::event::ElementState::Pressed {
                    if let (Some(window), Some(app_host)) =
                        (self.window.as_ref(), self.app_host.as_mut())
                    {
                        keyboard_input::apply_key_input(
                            app_host.tree_mut(),
                            &event.logical_key,
                            event.text.as_deref(),
                            self.modifiers,
                        );
                        window.request_redraw();
                    }
                }
            }
            // IME 入力（winit `Ime` → Core 増分 IME モデル・issue #508）。`Enabled` /
            // `Preedit` / `Commit` / `Disabled` を `ime_input` の純粋写像 seam で `ImeCommand`
            // に変換し、フレームをまたぐ `ime_buffer` へ畳んで focus 中 text-input に適用する。
            // request_redraw で次フレームを起こすと RedrawRequested 側で候補窓位置も同期する。
            WindowEvent::Ime(ime) => {
                if let (Some(window), Some(app_host)) =
                    (self.window.as_ref(), self.app_host.as_mut())
                {
                    ime_input::apply_ime_input(app_host.tree_mut(), &ime, &mut self.ime_buffer);
                    window.request_redraw();
                }
            }
            WindowEvent::CursorLeft { .. } => {
                // ポインタが窓面を離れたら hover をクリアする（座標非依存）。さもないと
                // 最後に hover した要素が `:hover` のまま固まる。
                if let (Some(window), Some(app_host)) =
                    (self.window.as_ref(), self.app_host.as_mut())
                {
                    app_host.tree_mut().on_pointer_leave();
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

/// desktop demo を起動する。OS event loop を立て、`DesktopApp` を駆動する。
///
/// 静的 1 枚を見せるだけなので `ControlFlow::Wait`（イベント待ち・idle で CPU を使わない）。
pub fn run() -> Result<(), winit::error::EventLoopError> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = DesktopApp::default();
    event_loop.run_app(&mut app)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_window_is_hidden_until_first_present() {
        // 起動時の暗転（未描画ウィンドウが ~1s 見える）の回帰テスト。非表示で作るのが
        // 暗転防止の前半で、後半（初回 present 後の `set_visible(true)`）は live event loop
        // が要るためヘッドレスには固定できない（`RedrawRequested` ハンドラ参照）。
        let attrs = initial_window_attributes();
        assert!(!attrs.visible, "window must start hidden (dark-screen prevention)");
        assert_eq!(attrs.title, WINDOW_TITLE);
    }

    #[test]
    fn window_defaults_are_named_constants() {
        assert!(!WINDOW_TITLE.is_empty(), "window title must be set");
        let (w, h) = DEFAULT_WINDOW_SIZE;
        assert!(w > 0 && h > 0, "default window size must be positive, got {w}x{h}");
        assert_eq!(CLEAR_COLOR[3], 1.0, "clear color must be opaque");
    }

    #[test]
    fn window_title_carries_the_selected_renderer() {
        // レンダラは selection policy が実行時に決めるため、タイトルも実行時に確定する。
        assert_eq!(window_title_for(SceneRendererKind::Vello), "Hayate — Tasks (vello)");
        assert_eq!(window_title_for(SceneRendererKind::Skia), "Hayate — Tasks (skia)");
    }

    #[test]
    fn init_errors_classify_into_the_selection_reason_vocabulary() {
        // 観測可能な語彙（RendererSelectionReason）への分類（ADR-0132: adapter 個別実装）。
        // 「リンクされていない」はコンパイル構成の帰結 → DisabledByPolicy（runtime
        // fallback 対象にしない）。それ以外の init/runtime 失敗は fallback 対象。
        assert_eq!(
            classify_error_message("renderer not linked into this desktop build: vello"),
            RendererSelectionReason::DisabledByPolicy,
        );
        assert_eq!(
            classify_error_message("swapchain surface lost"),
            RendererSelectionReason::SurfaceLost,
        );
        assert_eq!(
            classify_error_message("surface not supported by adapter"),
            RendererSelectionReason::RendererInitFailed,
            "init 段階の surface 非対応は喪失ではなく初期化失敗",
        );
        assert_eq!(
            classify_error_message("no compatible wgpu adapter: NotFound"),
            RendererSelectionReason::RendererInitFailed,
        );
        assert!(
            hayate_app_host::renderer_selection::is_runtime_fallback_reason(
                classify_error_message("no compatible wgpu adapter: NotFound")
            ),
            "vello init failure must drive the one-way fallback to skia"
        );
    }

    #[test]
    fn viewport_metrics_carries_hidpi_scale_factor() {
        // winit の scale_factor を content_scale に素通しする（1.0 に潰すと HiDPI でぼやける）。
        let m = viewport_metrics(1960, 2120, 2.0);
        assert_eq!(m.content_scale, 2.0);
        // 物理サイズ = バッキングストア、論理 = 物理 / scale。
        assert_eq!(m.buffer_size(), (1960, 2120));
        assert_eq!(m.viewport_size(), (980.0, 1060.0));
    }
}
