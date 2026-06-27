//! Desktop Platform Front（ADR-0118）。
//!
//! winit single crate で macos / windows / linux を windowing / event-loop / GPU surface の
//! 層に畳み、winit window + vello/wgpu の [`Surface`] 実装 + `App Host::tick` 駆動を最小配線して、
//! 共有 demo fixture（`hayate_demo_fixtures::tasks_tree`）を native window に present する。
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
pub mod pointer_input;

use std::sync::Arc;
use std::time::Instant;

use hayate_app_host::{AppHost, Surface};
use hayate_core::{ImeBridge, ImeBuffer, ImePresentation, SceneGraph, ViewportMetrics};
use hayate_demo_fixtures::tasks_tree;
use hayate_scene_renderer_vello::{
    create_blitter, create_target_view, VelloRenderTarget, VelloSceneRenderer,
};
use wgpu::util::TextureBlitter;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::ModifiersState;
use winit::window::{Window, WindowId};

/// renderer label shown in the demo fixture's AppBar badge — desktop presents with vello.
const RENDERER_LABEL: &str = "vello";

/// native window のタイトル。
pub const WINDOW_TITLE: &str = "Hayate — Tasks (vello)";

/// 既定のウィンドウサイズ（論理 px）。値の調整は人力フォローアップ（ADR-0118）。
pub const DEFAULT_WINDOW_SIZE: (u32, u32) = (1024, 1080);

/// surface の clear color（vello `base_color`）。demo fixture の背景 `#f1ede3` に合わせ、
/// content がカバーしない余白も意図した色で塗る。値の調整は人力フォローアップ（ADR-0118）。
pub const CLEAR_COLOR: [f32; 4] = [0.945, 0.929, 0.890, 1.0];

/// winit の物理サイズ・`scale_factor` を Core の [`ViewportMetrics`] に橋渡しする（ADR-0080）。
///
/// `scale_factor` を 1.0 に潰さず `content_scale` へ素通しするのが HiDPI でぼやけない要点で、
/// 論理ビューポート（`set_viewport` 入力）とバッキングストア（wgpu surface 設定）を同じ規約で導く。
pub fn viewport_metrics(physical_width: u32, physical_height: u32, scale_factor: f64) -> ViewportMetrics {
    ViewportMetrics::from_physical_size(physical_width as i32, physical_height as i32, scale_factor as f32)
}

/// winit の wgpu surface へ present する vello/wgpu の [`Surface`] 実装（ADR-0118）。
///
/// 1 フレームの `SceneGraph` を、vello の `render_to_texture` で offscreen target に焼き、
/// `TextureBlitter` で winit の swapchain surface に blit する。web の vello backend と同型で、
/// 違いは surface の供給元が `HtmlCanvasElement` ではなく winit `Window` である点だけ。
pub struct WindowSurface {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    target_view: wgpu::TextureView,
    blitter: TextureBlitter,
    renderer: VelloSceneRenderer,
    /// 論理→物理の変換係数（HiDPI）。render 時に vello painter を拡大して crisp に描く。
    content_scale: f32,
    /// バッキングストア寸法（物理 px）。
    width: u32,
    height: u32,
}

impl WindowSurface {
    /// winit `Window` から wgpu surface を立て、vello renderer を初期化する。
    /// `metrics` の `buffer_size`（物理 px）で surface を configure し、`content_scale` を保持する。
    pub fn new(window: Arc<Window>, metrics: ViewportMetrics) -> Result<Self, String> {
        pollster::block_on(Self::new_async(window, metrics))
    }

    async fn new_async(window: Arc<Window>, metrics: ViewportMetrics) -> Result<Self, String> {
        let (width, height) = metrics.buffer_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::from_env().unwrap_or(wgpu::Backends::all()),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });

        let surface = instance
            .create_surface(window)
            .map_err(|e| format!("create_surface: {e}"))?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .map_err(|e| format!("no compatible wgpu adapter: {e}"))?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("hayate-desktop"),
                ..Default::default()
            })
            .await
            .map_err(|e| format!("request_device: {e}"))?;

        let mut surface_config = surface
            .get_default_config(&adapter, width, height)
            .ok_or_else(|| "surface not supported by adapter".to_string())?;
        surface_config.usage |= wgpu::TextureUsages::RENDER_ATTACHMENT;
        // vello は offscreen target（`Rgba8Unorm`・非 sRGB）へ sRGB エンコード済みのバイトを書く。
        // surface が *Srgb 形式（Windows の `get_default_config` 既定は `Rgba8UnormSrgb`）だと、
        // blit の書き込みで linear→sRGB エンコードが二重にかかり色が淡く（washed out）見える。
        // surface を非 sRGB 形式に揃えて二重エンコードを防ぐ（web の canvas は既定が非 sRGB なので
        // この問題が出ない）。blitter は下でこの `surface_config.format` で生成するので整合する。
        surface_config.format = surface_config.format.remove_srgb_suffix();
        surface.configure(&device, &surface_config);

        let target_view = create_target_view(&device, width, height);
        let blitter = create_blitter(&device, surface_config.format);
        let renderer = VelloSceneRenderer::new(&device)?;

        Ok(Self {
            device,
            queue,
            surface,
            surface_config,
            target_view,
            blitter,
            renderer,
            content_scale: metrics.content_scale,
            width,
            height,
        })
    }

    /// winit リサイズ時に wgpu surface を再 configure し、offscreen target と content scale を
    /// 更新する。`metrics` は `viewport_metrics()` で物理サイズ・`scale_factor` から導いたもの。
    pub fn resize(&mut self, metrics: ViewportMetrics) {
        let (width, height) = metrics.buffer_size();
        self.content_scale = metrics.content_scale;
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

impl Surface for WindowSurface {
    fn present(&mut self, scene: &SceneGraph) {
        let render = self.renderer.render_scene(
            scene,
            &VelloRenderTarget {
                device: &self.device,
                queue: &self.queue,
                target_view: &self.target_view,
                width: self.width,
                height: self.height,
            },
            CLEAR_COLOR,
            self.content_scale,
        );
        if let Err(e) = render {
            log::error!("vello render_scene failed: {e}");
            return;
        }

        let surface_texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            // surface が陳腐化したら次フレームで再 configure に委ねる。
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.surface_config);
                return;
            }
            wgpu::CurrentSurfaceTexture::Occluded => return,
            other => {
                log::warn!("get_current_texture: {other:?}");
                return;
            }
        };

        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("hayate-desktop-blit"),
            });
        self.blitter
            .copy(&self.device, &mut encoder, &self.target_view, &surface_view);
        self.queue.submit(std::iter::once(encoder.finish()));
        surface_texture.present();
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
    app_host: Option<AppHost<WindowSurface>>,
    start: Option<Instant>,
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

        let attrs = Window::default_attributes()
            .with_title(WINDOW_TITLE)
            .with_inner_size(LogicalSize::new(DEFAULT_WINDOW_SIZE.0, DEFAULT_WINDOW_SIZE.1));
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                log::error!("create_window: {e}");
                event_loop.exit();
                return;
            }
        };

        let size = window.inner_size();
        let metrics = viewport_metrics(size.width, size.height, window.scale_factor());

        let surface = match WindowSurface::new(window.clone(), metrics) {
            Ok(s) => s,
            Err(e) => {
                log::error!("WindowSurface init failed: {e}");
                event_loop.exit();
                return;
            }
        };

        // request_redraw は App Host の唯一の wake 入口（ADR-0117）。winit window に配線する。
        let redraw_window = window.clone();
        let mut app_host = AppHost::new(surface, Box::new(move || redraw_window.request_redraw()));
        // 共有 demo fixture を App Host の tree に載せる（consumer は mount しない・ADR-0118）。
        *app_host.tree_mut() = tasks_tree(RENDERER_LABEL);
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
    fn window_defaults_are_named_constants() {
        assert!(!WINDOW_TITLE.is_empty(), "window title must be set");
        let (w, h) = DEFAULT_WINDOW_SIZE;
        assert!(w > 0 && h > 0, "default window size must be positive, got {w}x{h}");
        assert_eq!(CLEAR_COLOR[3], 1.0, "clear color must be opaque");
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
