//! 描画 + タッチ + IME ループの iOS グルー（ADR-0114）。
//!
//! Android の `app.rs` に対応する薄いプラットフォーム配線。ただし iOS ではイベントループ
//! を Rust が回さず、Swift ホスト（`HayateView`）が UIScene ライフサイクル・`UITouch`・
//! UITextInput・CADisplayLink を所有し、ここで公開する `extern "C"` FFI を叩く。Rust は
//! `ElementTree` を immutable committed frame に lower し、Swift が渡す `CAMetalLayer` に紐づく wgpu
//! Metal サーフェスへ提示する。decode/diff/apply のロジックはホストでテスト可能な
//! シーム（`surface_lifecycle` / `touch_input` / `ime_input`）にあり、本モジュールはその
//! 呼び出しと FFI 整形に限る。
//!
//! Mac/iOS SDK の無い環境ではこのファイルはコンパイルされない（`#[cfg(target_os="ios")]`、
//! Android の device 専用 `app.rs` と同じ）。実機/シミュレータ検証はローカルで行う。

use std::ffi::{c_char, c_void, CStr};
use std::time::Instant;

use hayate_core::{CommittedFrame, ElementId, ElementTree};
use hayate_layer_compositor::{
    scroll_layer_geometry_from_inputs, tunables, CompositeQuad, GpuBudget, LayerCompositor,
    LayerPresentation, LayerPresentationAdapter, LayerPresentationFrame, LayerRasterizer,
    PlacementPlan, RasterJob, RasterJobKind,
};
use hayate_scene_renderer_vello::layer_compositor::{
    CompositeTarget, VelloLayerRasterizer, WgpuQuadCompositor,
};

use crate::ime_bridge::IosImeBridge;
use crate::ime_input::{apply_command, apply_ime_action, ImeBuffer, ImeCommand};
use crate::scene_demo::build_demo_tree;
use crate::surface_lifecycle::{surface_metrics, window_dimensions};
use crate::touch_input::{translate_touch, PointerInput, TouchAction};

/// スモークテスト用の RGBA クリアカラー。
pub const CLEAR_COLOR: [f32; 4] = crate::STAGE_A_CLEAR_COLOR;

struct GpuSurface {
    device: wgpu::Device,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    width: u32,
    height: u32,
    content_scale: f32,
    presentation: LayerPresentation,
    rasterizer: VelloLayerRasterizer,
    compositor: WgpuQuadCompositor,
}

/// 1 ビューぶんのアダプタ状態。Swift が `hayate_ios_app_new` で sized な `CAMetalLayer`
/// から作り、CADisplayLink ごとに `hayate_ios_render` を呼ぶ。
struct IosApp {
    tree: ElementTree,
    gpu: GpuSurface,
    /// CADisplayLink タイムスタンプ用の単調クロック起点。
    start: Instant,
    /// UITextInput のローカルバッファ（確定 + marked）。
    ime_buf: ImeBuffer,
    /// 現在 IME を流している TextInput。フォーカス変化でバッファをリセットする。
    ime_target: Option<ElementId>,
    /// ソフトキーボードが現在表示中か（`IosImeBridge` が所有）。
    ime_keyboard_shown: bool,
    content_scale: f32,
}

impl IosApp {
    fn set_viewport_from(&mut self, px_width: i32, px_height: i32) {
        let (vw, vh) = surface_metrics(px_width, px_height, self.content_scale).viewport_size();
        self.tree.set_viewport(vw, vh);
    }

    /// CADisplayLink の 1 tick: IME を反映し、ツリーを lower して提示する。
    fn render(&mut self) {
        // ソフトキーボードの表示可否は core が編集可否から決め、bridge が反映する。
        // フォーカスが TextInput 間で変わったらローカルバッファをベースラインに戻す。
        {
            let mut bridge = IosImeBridge::new(&mut self.ime_keyboard_shown);
            self.tree.drive_ime(&mut bridge);
        }
        let target = self.tree.focused_text_input();
        if self.ime_target != target {
            self.ime_target = target;
            self.ime_buf = ImeBuffer::new();
        }

        let timestamp_ms = self.start.elapsed().as_secs_f64() * 1000.0;
        let frame = self.tree.commit_rendered_frame(timestamp_ms);
        if let Err(err) = self.gpu.render_frame(&frame) {
            log::error!("hayate-adapter-ios: render failed: {err}");
        }
    }

    /// UITextInput コマンドをフォーカス中の TextInput に適用する。
    fn ime_command(&mut self, command: ImeCommand) {
        let Some(target) = self.ime_target.or_else(|| self.tree.focused_text_input()) else {
            return;
        };
        self.ime_target = Some(target);
        for action in apply_command(&mut self.ime_buf, command) {
            apply_ime_action(&mut self.tree, target, &action);
        }
    }
}

/// Swift の `AppDelegate` が起動時に一度呼ぶフック（ロガー初期化）。Android の
/// `android_main` 冒頭に対応する名前付きエントリ。実際のフレーム駆動は `HayateView` が
/// `hayate_ios_*` 経由で行う。
#[no_mangle]
pub extern "C" fn ios_main() {
    // iOS では `oslog`/`os_log` が望ましいが、groundwork では env_logger 非依存の簡易初期化に留め、
    // ログバックエンドの選定は実機検証時に行う。
    log::info!("hayate-adapter-ios: ios_main");
}

/// sized な `CAMetalLayer`（InitWindow）からアダプタ状態を作る。`metal_layer` は Swift の
/// `CAMetalLayer` ポインタ、`scale` は `UIScreen.scale`（Retina）。失敗時は null を返す。
///
/// # Safety
/// `metal_layer` は有効な `CAMetalLayer` を指し、本アダプタの生存期間中サーフェスより長く
/// 生きること（Swift 側がビューと共に保持する）。
#[no_mangle]
pub unsafe extern "C" fn hayate_ios_app_new(metal_layer: *mut c_void, scale: f32) -> *mut c_void {
    if metal_layer.is_null() {
        log::error!("hayate-adapter-ios: null CAMetalLayer");
        return std::ptr::null_mut();
    }

    let mut tree = build_demo_tree();
    let content_scale = scale.max(1.0);

    let gpu = match pollster::block_on(init_gpu_surface(metal_layer, content_scale)) {
        Ok(gpu) => gpu,
        Err(err) => {
            log::error!("hayate-adapter-ios: GPU init failed: {err}");
            return std::ptr::null_mut();
        }
    };

    let (vw, vh) =
        surface_metrics(gpu.width as i32, gpu.height as i32, content_scale).viewport_size();
    tree.set_viewport(vw, vh);

    let app = Box::new(IosApp {
        tree,
        gpu,
        start: Instant::now(),
        ime_buf: ImeBuffer::new(),
        ime_target: None,
        ime_keyboard_shown: false,
        content_scale,
    });
    Box::into_raw(app) as *mut c_void
}

/// アダプタ状態を解放する（Destroy / sceneDidDisconnect）。
///
/// # Safety
/// `app` は `hayate_ios_app_new` が返したポインタで、二重解放しないこと。
#[no_mangle]
pub unsafe extern "C" fn hayate_ios_app_free(app: *mut c_void) {
    if !app.is_null() {
        drop(Box::from_raw(app as *mut IosApp));
    }
}

/// ドローアブルがリサイズした（WindowResized）。寸法は物理 px（points × scale）。
///
/// # Safety
/// `app` は `hayate_ios_app_new` が返した有効なポインタであること。
#[no_mangle]
pub unsafe extern "C" fn hayate_ios_resize(app: *mut c_void, width: i32, height: i32, scale: f32) {
    let Some(app) = (app as *mut IosApp).as_mut() else {
        return;
    };
    app.content_scale = scale.max(1.0);
    let (w, h) = window_dimensions(width, height);
    app.gpu.resize(w, h, app.content_scale);
    app.set_viewport_from(w as i32, h as i32);
}

/// 単一ポインタのタッチ。phase: 0=Down 1=Move 2=Up 3=Cancel、座標はビュー points。
///
/// # Safety
/// `app` は `hayate_ios_app_new` が返した有効なポインタであること。
#[no_mangle]
pub unsafe extern "C" fn hayate_ios_touch(app: *mut c_void, phase: i32, x: f32, y: f32) {
    let Some(app) = (app as *mut IosApp).as_mut() else {
        return;
    };
    let Some(action) = touch_phase_to_action(phase) else {
        return;
    };
    match translate_touch(action, x, y) {
        PointerInput::Down { x, y } => app.tree.on_pointer_down(x, y),
        PointerInput::Move { x, y } => {
            let _ = app.tree.on_pointer_move(x, y);
        }
        PointerInput::Up { x, y } => app.tree.on_pointer_up(x, y),
        PointerInput::Cancel => app.tree.on_pointer_cancel(),
    }
}

/// UITextInput コマンド。kind: 0=Insert 1=DeleteBackward 2=SetMarked 3=Unmark。
/// `text` は UTF-8（DeleteBackward/Unmark は null）。
///
/// # Safety
/// `app` は有効なポインタ、`text` は null か有効な NUL 終端 UTF-8 文字列であること。
#[no_mangle]
pub unsafe extern "C" fn hayate_ios_ime(app: *mut c_void, kind: i32, text: *const c_char) {
    let Some(app) = (app as *mut IosApp).as_mut() else {
        return;
    };
    let text = if text.is_null() {
        String::new()
    } else {
        match CStr::from_ptr(text).to_str() {
            Ok(s) => s.to_string(),
            Err(_) => {
                log::error!("hayate-adapter-ios: non-UTF-8 IME text");
                return;
            }
        }
    };
    let command = match kind {
        0 => ImeCommand::Insert(text),
        1 => ImeCommand::DeleteBackward,
        2 => ImeCommand::SetMarked(text),
        3 => ImeCommand::Unmark,
        other => {
            log::error!("hayate-adapter-ios: unknown IME kind {other}");
            return;
        }
    };
    app.ime_command(command);
}

/// CADisplayLink の 1 tick で 1 フレーム描画・提示する。
///
/// # Safety
/// `app` は `hayate_ios_app_new` が返した有効なポインタであること。
#[no_mangle]
pub unsafe extern "C" fn hayate_ios_render(app: *mut c_void, _timestamp_ms: f64) {
    if let Some(app) = (app as *mut IosApp).as_mut() {
        app.render();
    }
}

/// FFI の touch phase を単一ポインタの [`TouchAction`] に対応付ける。範囲外は `None`。
fn touch_phase_to_action(phase: i32) -> Option<TouchAction> {
    match phase {
        0 => Some(TouchAction::Down),
        1 => Some(TouchAction::Move),
        2 => Some(TouchAction::Up),
        3 => Some(TouchAction::Cancel),
        _ => None,
    }
}

async fn init_gpu_surface(
    metal_layer: *mut c_void,
    content_scale: f32,
) -> Result<GpuSurface, String> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::METAL,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });

    // Swift が所有する `CAMetalLayer` から直接 wgpu Metal サーフェスを張る。raw-window-handle
    // を経由しないので display handle 不要（Android の `RawHandle` 経路に対する iOS の素直な道）。
    // SAFETY: `metal_layer` は Swift がビューと共に保持する有効な `CAMetalLayer`。
    let surface = unsafe {
        instance
            .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(metal_layer))
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
            label: Some("hayate-ios"),
            ..Default::default()
        })
        .await
        .map_err(|e| format!("request_device: {e}"))?;

    // CAMetalLayer の現在のドローアブル寸法でサーフェスを構成する。Swift が layoutSubviews で
    // drawableSize を設定済み。get_default_config の既定寸法を採用し、以後 resize で追従する。
    let caps = surface.get_capabilities(&adapter);
    let format = caps.formats[0];
    // 初期寸法は Swift から渡る最初の resize で確定するため、暫定 1×1 で構成して即 resize に委ねる。
    let (width, height) = (1u32, 1u32);
    let mut surface_config = surface
        .get_default_config(&adapter, width, height)
        .ok_or("surface not supported by adapter")?;
    surface_config.format = format;
    surface_config.usage |= wgpu::TextureUsages::RENDER_ATTACHMENT;
    surface.configure(&device, &surface_config);

    let rasterizer =
        VelloLayerRasterizer::new(device.clone(), queue.clone(), width, height, content_scale)?;
    let mut compositor = WgpuQuadCompositor::new(device.clone(), queue.clone());
    compositor.set_content_scale(content_scale);
    compositor.warmup();

    Ok(GpuSurface {
        device,
        surface,
        surface_config,
        width,
        height,
        content_scale,
        presentation: LayerPresentation::new(),
        rasterizer,
        compositor,
    })
}

impl GpuSurface {
    fn render_frame(&mut self, frame: &CommittedFrame) -> Result<(), String> {
        let scroll_geometry = scroll_layer_geometry_from_inputs(frame.scroll_inputs());
        let mut adapter = IosLayerPresentationAdapter {
            rasterizer: &mut self.rasterizer,
            compositor: &mut self.compositor,
            device: &self.device,
            surface: &self.surface,
            surface_config: &self.surface_config,
            clear: CLEAR_COLOR,
        };
        self.presentation
            .present(
                LayerPresentationFrame {
                    snapshot: frame.snapshot(),
                    topology: frame.layer_topology(),
                    scroll_geometry: &scroll_geometry,
                },
                &mut adapter,
            )
            .map_err(|error| format!("layer presentation: {error:?}"))?;
        let budget = GpuBudget::from_viewports(
            self.width,
            self.height,
            tunables::GPU_BUDGET_VIEWPORTS_MOBILE,
        );
        self.presentation.enforce_budget(budget, &mut adapter);
        Ok(())
    }

    fn resize(&mut self, width: u32, height: u32, content_scale: f32) {
        if width == 0 || height == 0 {
            return;
        }
        let changed =
            width != self.width || height != self.height || content_scale != self.content_scale;
        if !changed {
            return;
        }
        self.width = width;
        self.height = height;
        self.content_scale = content_scale;
        self.surface_config.width = width;
        self.surface_config.height = height;
        self.surface.configure(&self.device, &self.surface_config);
        self.rasterizer.resize(width, height, self.content_scale);
        self.compositor.set_content_scale(self.content_scale);
        self.presentation.invalidate();
    }
}

struct IosLayerPresentationAdapter<'a> {
    rasterizer: &'a mut VelloLayerRasterizer,
    compositor: &'a mut WgpuQuadCompositor,
    device: &'a wgpu::Device,
    surface: &'a wgpu::Surface<'static>,
    surface_config: &'a wgpu::SurfaceConfiguration,
    clear: [f32; 4],
}

impl LayerPresentationAdapter for IosLayerPresentationAdapter<'_> {
    type Error = String;

    fn rasterize(&mut self, job: &RasterJob<'_>) -> Result<u64, Self::Error> {
        match job.kind {
            RasterJobKind::Content => match job.bounds {
                Some(bounds) => self
                    .rasterizer
                    .rasterize_in_bounds(job.layer, job.scene, bounds, job.band)?,
                None => self.rasterizer.rasterize(job.layer, job.scene, job.band)?,
            },
            RasterJobKind::ScrollChrome => {
                match job.bounds {
                    Some(bounds) => self.rasterizer.update_scroll_chrome_in_bounds(
                        job.layer,
                        job.scene,
                        bounds,
                        job.repaint,
                    )?,
                    None => {
                        self.rasterizer
                            .update_scroll_chrome(job.layer, job.scene, job.repaint)?
                    }
                };
            }
        }
        Ok(self.rasterizer.cache_bytes(job.layer))
    }

    fn composite(&mut self, plan: &PlacementPlan) -> Result<(), Self::Error> {
        let surface_texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            wgpu::CurrentSurfaceTexture::Occluded => return Ok(()),
            other => return Err(format!("get_current_texture: {other:?}")),
        };
        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut quads = Vec::with_capacity(plan.planes.len());
        for plane in &plan.planes {
            let texture = match plane.kind {
                RasterJobKind::Content => self.rasterizer.texture(plane.layer),
                RasterJobKind::ScrollChrome => self.rasterizer.scroll_chrome_texture(plane.layer),
            };
            if let Some(texture) = texture {
                quads.push(CompositeQuad {
                    layer: plane.layer,
                    transform: plane.transform,
                    opacity: 1.0,
                    clip: plane.clip,
                    texture,
                });
            }
        }
        let mut target = CompositeTarget {
            view: surface_view,
            width: self.surface_config.width,
            height: self.surface_config.height,
            format: self.surface_config.format,
            clear: self.clear,
        };
        self.compositor.composite(&mut target, &quads)?;
        surface_texture.present();
        Ok(())
    }

    fn discard(&mut self, layers: &[ElementId]) {
        for &layer in layers {
            self.rasterizer.discard(layer);
        }
    }
}
