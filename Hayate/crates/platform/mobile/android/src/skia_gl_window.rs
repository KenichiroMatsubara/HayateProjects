//! skia GL（Ganesh/EGL）の `ANativeWindow` 提示面（issue #803・ADR-0146 §3）。
//!
//! Android HWUI / Chrome が長年叩いたドライバ成熟経路（GL）で skia を GPU 駆動する——
//! skia エスカレーション（ADR-0146/0147・#796）の完成形。Skia Vulkan は採らない（容疑者が
//! Adreno の Vulkan ドライバのため）。EGL コンテキスト・EGLSurface（ANativeWindow 結線）の
//! 管理は本モジュール＝Android Platform Adapter に閉じ、core に第二の GPU 抽象を持ち込まない
//! （REND-07）。#795 の wgpu GL スイッチ（`render_config::WgpuBackend::Gl`）が確立した
//! 「GL 初期化失敗でも boot を落とさない」姿勢を引き継ぎ、失敗は `Err` で返して呼び元
//! （`app.rs::init_and_spawn_raster`）が skia raster へ一方向 fallback する。
//!
//! painter は不変（ADR-0146 §3 の surface 非依存設計）: `SceneGraph`→Canvas の変換層
//! （`hayate-scene-renderer-skia::painter`）は CPU/GL で共有する。dirty layer は同じ
//! `SkiaLayerPresenter` で cache 更新し、dirty layer と最終 quad 合成先の両方を同じ
//! `DirectContext` 上の Ganesh surface にする。
//!
//! スレッド規約（ADR-0128 / `RasterThread`）: EGL 初期化・観測ログ（EGL vendor / GL renderer）
//! は UI スレッドで行い（fallback 判定を spawn 前に済ませるため）、直後に unbind して
//! `SkiaGlSurface` を Raster スレッドへ move する。GL コンテキストと Ganesh
//! `DirectContext` は初回フレームで Raster スレッドに bind / 生成され、以後そのスレッドに
//! 束縛される（EGL コンテキストは「どこかのスレッドで current でない限り」スレッド間 move 可）。

use std::collections::HashSet;
use std::ffi::{c_char, c_void, CStr, CString};
use std::ptr;

use hayate_core::{ElementId, SceneGraph, ScrollCompositorInput};
use hayate_layer_compositor::{scroll_layer_geometry_from_inputs, tunables, GpuBudget};
use hayate_scene_renderer_skia::{SkiaLayerPresenter, SkiaLayerSurfaceFactory};
use ndk::native_window::NativeWindow;
use skia_safe::gpu;

// ── 最小 EGL / GLES FFI ─────────────────────────────────────────────────────────────────
//
// khronos-egl 等のバインディング crate は足さない——必要なのは window surface 1 枚分の
// ライフサイクルだけで、libEGL / libGLESv2 は Android の system library（skia-bindings の
// `gl` feature も同じ 2 つをリンクする）。定数は EGL 1.5 / GLES 2.0 の標準値。

type EGLDisplay = *mut c_void;
type EGLConfig = *mut c_void;
type EGLContext = *mut c_void;
type EGLSurface = *mut c_void;
type EGLBoolean = u32;
type EGLint = i32;

const EGL_NO_CONTEXT: EGLContext = ptr::null_mut();
const EGL_NO_SURFACE: EGLSurface = ptr::null_mut();
const EGL_FALSE: EGLBoolean = 0;

const EGL_ALPHA_SIZE: EGLint = 0x3021;
const EGL_BLUE_SIZE: EGLint = 0x3022;
const EGL_GREEN_SIZE: EGLint = 0x3023;
const EGL_RED_SIZE: EGLint = 0x3024;
const EGL_STENCIL_SIZE: EGLint = 0x3026;
const EGL_SURFACE_TYPE: EGLint = 0x3033;
const EGL_NONE: EGLint = 0x3038;
const EGL_RENDERABLE_TYPE: EGLint = 0x3040;
const EGL_WINDOW_BIT: EGLint = 0x0004;
const EGL_OPENGL_ES2_BIT: EGLint = 0x0004;
const EGL_CONTEXT_CLIENT_VERSION: EGLint = 0x3098;
const EGL_VENDOR: EGLint = 0x3053;
const EGL_VERSION: EGLint = 0x3054;
const EGL_HEIGHT: EGLint = 0x3056;
const EGL_WIDTH: EGLint = 0x3057;

const GL_VENDOR: u32 = 0x1F00;
const GL_RENDERER: u32 = 0x1F01;
const GL_VERSION: u32 = 0x1F02;
/// Ganesh backend render target の色フォーマット（`FramebufferInfo::format`）。EGL config で
/// RGBA8888 を要求しているので GL_RGBA8。
const GL_RGBA8: u32 = 0x8058;

#[link(name = "EGL")]
extern "C" {
    fn eglGetDisplay(display_id: *mut c_void) -> EGLDisplay;
    fn eglInitialize(dpy: EGLDisplay, major: *mut EGLint, minor: *mut EGLint) -> EGLBoolean;
    fn eglChooseConfig(
        dpy: EGLDisplay,
        attrib_list: *const EGLint,
        configs: *mut EGLConfig,
        config_size: EGLint,
        num_config: *mut EGLint,
    ) -> EGLBoolean;
    fn eglGetConfigAttrib(
        dpy: EGLDisplay,
        config: EGLConfig,
        attribute: EGLint,
        value: *mut EGLint,
    ) -> EGLBoolean;
    fn eglCreateContext(
        dpy: EGLDisplay,
        config: EGLConfig,
        share_context: EGLContext,
        attrib_list: *const EGLint,
    ) -> EGLContext;
    fn eglCreateWindowSurface(
        dpy: EGLDisplay,
        config: EGLConfig,
        win: *mut c_void,
        attrib_list: *const EGLint,
    ) -> EGLSurface;
    fn eglMakeCurrent(
        dpy: EGLDisplay,
        draw: EGLSurface,
        read: EGLSurface,
        ctx: EGLContext,
    ) -> EGLBoolean;
    fn eglSwapBuffers(dpy: EGLDisplay, surface: EGLSurface) -> EGLBoolean;
    fn eglQuerySurface(
        dpy: EGLDisplay,
        surface: EGLSurface,
        attribute: EGLint,
        value: *mut EGLint,
    ) -> EGLBoolean;
    fn eglDestroySurface(dpy: EGLDisplay, surface: EGLSurface) -> EGLBoolean;
    fn eglDestroyContext(dpy: EGLDisplay, ctx: EGLContext) -> EGLBoolean;
    fn eglQueryString(dpy: EGLDisplay, name: EGLint) -> *const c_char;
    fn eglGetProcAddress(procname: *const c_char) -> *mut c_void;
    fn eglGetError() -> EGLint;
}

#[link(name = "GLESv2")]
extern "C" {
    fn glGetString(name: u32) -> *const u8;
}

fn egl_err(what: &str) -> String {
    // SAFETY: eglGetError は引数なしのステータス取得で常に呼べる。
    format!("{what} (eglGetError=0x{:x})", unsafe { eglGetError() })
}

/// nul 終端 C 文字列（EGL/GL のクエリ結果）を表示用 String へ。null は "?"。
fn c_str_or_unknown(ptr: *const c_char) -> String {
    if ptr.is_null() {
        return "?".to_string();
    }
    // SAFETY: EGL/GL のクエリ文字列は静的な nul 終端文字列。
    unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned()
}

// ── EGL ハンドル束（display / context / ANativeWindow 結線 surface） ─────────────────────

/// 初期化成功後の EGL ハンドル束。所有権は `SkiaGlSurface` ごと Raster スレッドへ move する。
struct EglHandles {
    display: EGLDisplay,
    context: EGLContext,
    surface: EGLSurface,
    /// 選ばれた EGL config の実ステンシルビット数（Ganesh backend render target に渡す）。
    stencil_bits: EGLint,
}

// SAFETY: 生の EGL ハンドル。EGL の規約上、コンテキストは「どのスレッドでも current でない」
// 状態ならスレッド間を move できる。`init_skia_gl_surface` は UI スレッドで unbind してから
// 返し、以後 bind・描画・破棄はすべて単一の Raster スレッドで行う（ADR-0128 の
// move-after-creation パターン）。
unsafe impl Send for EglHandles {}

impl EglHandles {
    fn make_current(&self) -> Result<(), String> {
        // SAFETY: ハンドルは初期化成功済み。current 化はこのスレッドに文脈を束縛するだけ。
        if unsafe { eglMakeCurrent(self.display, self.surface, self.surface, self.context) }
            == EGL_FALSE
        {
            return Err(egl_err("eglMakeCurrent"));
        }
        Ok(())
    }

    fn unbind(&self) {
        // SAFETY: current 解除。失敗しても後続の bind で顕在化するのでログ不要。
        unsafe { eglMakeCurrent(self.display, EGL_NO_SURFACE, EGL_NO_SURFACE, EGL_NO_CONTEXT) };
    }

    fn swap_buffers(&self) -> Result<(), String> {
        // SAFETY: current なコンテキストの window surface に対する標準 present。
        if unsafe { eglSwapBuffers(self.display, self.surface) } == EGL_FALSE {
            return Err(egl_err("eglSwapBuffers"));
        }
        Ok(())
    }

    /// EGLSurface の現在の物理pxサイズ。ANativeWindow の resize に追随した実サイズを毎フレーム
    /// これで取り、Ganesh の backend render target 寸法に使う。
    fn surface_size(&self) -> Result<(i32, i32), String> {
        let (mut w, mut h): (EGLint, EGLint) = (0, 0);
        // SAFETY: 初期化済み surface への属性クエリ。
        let ok = unsafe {
            eglQuerySurface(self.display, self.surface, EGL_WIDTH, &mut w) != EGL_FALSE
                && eglQuerySurface(self.display, self.surface, EGL_HEIGHT, &mut h) != EGL_FALSE
        };
        if !ok {
            return Err(egl_err("eglQuerySurface"));
        }
        Ok((w, h))
    }
}

impl Drop for EglHandles {
    fn drop(&mut self) {
        // Raster スレッド上（sink クロージャ drop）で走る。current 解除してから破棄する。
        self.unbind();
        // SAFETY: 自分が生成したハンドルの破棄。display の eglTerminate は呼ばない——
        // EGLDisplay はプロセス共有で、wgpu GL 経路（#795）等の他の消費者を巻き込むため。
        unsafe {
            eglDestroySurface(self.display, self.surface);
            eglDestroyContext(self.display, self.context);
        }
    }
}

// ── Ganesh（skia GL DirectContext）状態 ────────────────────────────────────────────────

/// Raster スレッド上で遅延生成される Ganesh 状態。
struct GaneshGl {
    context: gpu::DirectContext,
}

/// Dirty layer cache を final composite と同じ `DirectContext` 上へ確保する adapter。
struct GaneshLayerSurfaceFactory<'a> {
    context: &'a mut gpu::DirectContext,
}

impl SkiaLayerSurfaceFactory for GaneshLayerSurfaceFactory<'_> {
    fn create_layer_surface(
        &mut self,
        width: i32,
        height: i32,
    ) -> Result<skia_safe::Surface, String> {
        let info = skia_safe::ImageInfo::new(
            (width, height),
            skia_safe::ColorType::RGBA8888,
            skia_safe::AlphaType::Premul,
            None,
        );
        gpu::surfaces::render_target(
            self.context,
            gpu::Budgeted::Yes,
            &info,
            None,
            gpu::SurfaceOrigin::TopLeft,
            None,
            false,
            false,
        )
        .ok_or_else(|| format!("Ganesh layer render target {width}x{height} creation failed"))
    }
}

// SAFETY: `GaneshGl` の生成・使用・drop はすべて Raster スレッド上（`render_frame` /
// sink クロージャ drop）で起きる。この impl は「`SkiaGlSurface` がこのフィールドを `None` の
// まま Raster スレッドへ move する」spawn 境界を型的に満たすためだけに要る（skia-safe の
// RCHandle が保守的に !Send なため）。実行時にスレッドを跨いで共有・使用されることはない。
unsafe impl Send for GaneshGl {}

/// skia GL の一方向 fallback 元（既定側）が使う GPU present 面。`SkiaGpuSurface`
/// （`skia_window.rs`、CPU raster）と対の型で、同じ `RasterCommand` チャネル越しに専用
/// Raster スレッド上で駆動される（ADR-0128）。
pub(crate) struct SkiaGlSurface {
    /// GPU-backed layer cache を所有する。Rust は field 宣言順に drop するため、Ganesh context
    /// より前に置き、cache image を context より先に破棄する。
    presenter: SkiaLayerPresenter,
    /// Ganesh `DirectContext`。Raster スレッド上の初回フレームで生成する（EGL bind 後で
    /// ないと作れず、bind 先スレッドに束縛されるため）。
    ganesh: Option<GaneshGl>,
    /// Ganesh 生成に失敗したら以後のフレームを静かにスキップする（毎フレームのログ洪水を
    /// 防ぐ。EGL プローブは init 時に済んでいるため、ここに来る失敗は稀）。
    ganesh_failed: bool,
    egl: EglHandles,
    /// `ANativeWindow_acquire` 済みの独立参照。EGLSurface が結線している window を
    /// EGL ハンドルより長生きさせる（`skia_window.rs` と同じ前提）。
    _window: NativeWindow,
    width: u32,
    height: u32,
    content_scale: f32,
    /// この Raster スレッドで EGL コンテキストを bind 済みか（初回フレームで bind）。
    bound: bool,
}

/// `window` に EGL window surface + GL コンテキストを立て、EGL/GPU 情報を logcat に出す
/// （issue #803 の観測受け入れ条件）。UI スレッドで呼ばれ、成功時は unbind 済みの
/// `SkiaGlSurface` を返す（Raster スレッドが初回フレームで bind し直す）。失敗は `Err`——
/// 呼び元が skia raster へ一方向 fallback するので boot は落ちない。
pub(crate) fn init_skia_gl_surface(
    window: &NativeWindow,
    content_scale: f32,
) -> Result<SkiaGlSurface, String> {
    let (width, height) =
        crate::surface_lifecycle::window_dimensions(window.width(), window.height());
    let window = window.clone();

    // SAFETY 全般: EGL 1.4+ の標準初期化列。各ステップの失敗は Err で返し、生成済みハンドルは
    // その場で破棄する（半端な状態を持ち出さない）。
    unsafe {
        let display = eglGetDisplay(ptr::null_mut());
        if display.is_null() {
            return Err("eglGetDisplay returned EGL_NO_DISPLAY".to_string());
        }
        let (mut major, mut minor): (EGLint, EGLint) = (0, 0);
        if eglInitialize(display, &mut major, &mut minor) == EGL_FALSE {
            return Err(egl_err("eglInitialize"));
        }

        // RGBA8888・stencil 8（Ganesh はウィンドウ FBO にステンシルを要求しうる）・
        // window surface・GLES2 以上。MSAA はここでは要求しない（sample_count 0）。
        let config_attribs: [EGLint; 15] = [
            EGL_RENDERABLE_TYPE,
            EGL_OPENGL_ES2_BIT,
            EGL_SURFACE_TYPE,
            EGL_WINDOW_BIT,
            EGL_RED_SIZE,
            8,
            EGL_GREEN_SIZE,
            8,
            EGL_BLUE_SIZE,
            8,
            EGL_ALPHA_SIZE,
            8,
            EGL_STENCIL_SIZE,
            8,
            EGL_NONE,
        ];
        let mut config: EGLConfig = ptr::null_mut();
        let mut num_config: EGLint = 0;
        if eglChooseConfig(
            display,
            config_attribs.as_ptr(),
            &mut config,
            1,
            &mut num_config,
        ) == EGL_FALSE
            || num_config < 1
        {
            return Err(egl_err(
                "eglChooseConfig (no RGBA8888+stencil8 window config)",
            ));
        }
        let mut stencil_bits: EGLint = 0;
        if eglGetConfigAttrib(display, config, EGL_STENCIL_SIZE, &mut stencil_bits) == EGL_FALSE {
            stencil_bits = 8; // 要求どおりのはず。クエリ失敗時は要求値で続行。
        }

        // GLES3 を優先し、無ければ GLES2（Ganesh はどちらでも動く）。
        let mut context = EGL_NO_CONTEXT;
        for client_version in [3, 2] {
            let ctx_attribs = [EGL_CONTEXT_CLIENT_VERSION, client_version, EGL_NONE];
            context = eglCreateContext(display, config, EGL_NO_CONTEXT, ctx_attribs.as_ptr());
            if context != EGL_NO_CONTEXT {
                break;
            }
        }
        if context == EGL_NO_CONTEXT {
            return Err(egl_err("eglCreateContext (GLES3/GLES2)"));
        }

        let surface =
            eglCreateWindowSurface(display, config, window.ptr().as_ptr().cast(), ptr::null());
        if surface == EGL_NO_SURFACE {
            eglDestroyContext(display, context);
            return Err(egl_err("eglCreateWindowSurface"));
        }

        let egl = EglHandles {
            display,
            context,
            surface,
            stencil_bits,
        };

        // 観測（issue #803 受け入れ条件）: EGL/GPU 情報を logcat へ。GL 文字列のクエリには
        // current なコンテキストが要るため、ここで一時 bind して読み、unbind して返す
        // （Raster スレッドが初回フレームで bind し直す）。
        if let Err(err) = egl.make_current() {
            return Err(format!("initial eglMakeCurrent failed: {err}"));
            // egl は Drop で破棄される。
        }
        let egl_vendor = c_str_or_unknown(eglQueryString(display, EGL_VENDOR));
        let egl_version = c_str_or_unknown(eglQueryString(display, EGL_VERSION));
        let gl_vendor = c_str_or_unknown(glGetString(GL_VENDOR).cast());
        let gl_renderer = c_str_or_unknown(glGetString(GL_RENDERER).cast());
        let gl_version = c_str_or_unknown(glGetString(GL_VERSION).cast());
        log::info!(
            "hayate-adapter-android: skia GL surface — EGL vendor={egl_vendor} \
             EGL version={egl_version} ({major}.{minor}) GL vendor={gl_vendor} \
             GL renderer={gl_renderer} GL version={gl_version} stencil={stencil_bits}"
        );
        egl.unbind();

        Ok(SkiaGlSurface {
            presenter: SkiaLayerPresenter::new(width, height, content_scale),
            ganesh: None,
            ganesh_failed: false,
            egl,
            _window: window,
            width,
            height,
            content_scale,
            bound: false,
        })
    }
}

impl SkiaGlSurface {
    /// 1 フレームの提示。raster gating・safe-area offset は vello / skia raster の
    /// `render_frame` と同じ扱いで、違いは「Canvas の出自が Ganesh の FBO0 wrap で、
    /// present が `eglSwapBuffers`」なことだけ（ADR-0146 §3）。
    pub(crate) fn render_frame(
        &mut self,
        scene: &SceneGraph,
        layers: &[ElementId],
        layer_dirty: &HashSet<ElementId>,
        _transform_dirty: &HashSet<ElementId>,
        chrome_dirty: &HashSet<ElementId>,
        scroll_inputs: &[ScrollCompositorInput],
    ) -> Result<(), String> {
        let mut present_dirty = layer_dirty.clone();
        present_dirty.extend(chrome_dirty.iter().copied());
        let scroll_geometry = scroll_layer_geometry_from_inputs(scroll_inputs);

        if self.ganesh_failed {
            // Ganesh 生成に一度失敗している（初回フレームでログ済み）。ログ洪水を避けて
            // 静かにスキップする——EGL プローブは init 時に成功しているため、ここは稀。
            return Ok(());
        }
        if !self.bound {
            self.egl.make_current()?;
            self.bound = true;
        }
        if self.ganesh.is_none() {
            match make_ganesh_context() {
                Ok(context) => self.ganesh = Some(GaneshGl { context }),
                Err(err) => {
                    self.ganesh_failed = true;
                    return Err(format!(
                        "skia GL DirectContext init failed (subsequent frames will be skipped): {err}"
                    ));
                }
            }
        }
        let ganesh = self
            .ganesh
            .as_mut()
            .expect("ganesh context was just created");

        // ANativeWindow の実サイズに追随する（resize は EGLSurface が追いかける）。
        let (surface_w, surface_h) = self.egl.surface_size()?;
        self.presenter
            .resize(surface_w as u32, surface_h as u32, self.content_scale);
        let target = gpu::backend_render_targets::make_gl(
            (surface_w, surface_h),
            None,
            self.egl.stencil_bits as usize,
            gpu::gl::FramebufferInfo {
                fboid: 0, // EGL window surface の既定 framebuffer
                format: GL_RGBA8,
                protected: gpu::Protected::No,
            },
        );
        let surface = gpu::surfaces::wrap_backend_render_target(
            &mut ganesh.context,
            &target,
            gpu::SurfaceOrigin::BottomLeft,
            skia_safe::ColorType::RGBA8888,
            None,
            None,
        )
        .ok_or("gpu::surfaces::wrap_backend_render_target returned None")?;

        // b2（edge-to-edge, issue #794・ADR-0144）: vello / skia raster と同じ安全領域平行移動。
        let (origin_x, origin_y) = crate::safe_area::pushed_insets()
            .map(|insets| insets.scene_origin(self.content_scale))
            .unwrap_or((0.0, 0.0));

        let surface = {
            let mut layer_surfaces = GaneshLayerSurfaceFactory {
                context: &mut ganesh.context,
            };
            self.presenter.present_with_layer_surface_factory(
                scene,
                layers,
                &present_dirty,
                &scroll_geometry,
                crate::app::CLEAR_COLOR,
                (origin_x, origin_y),
                GpuBudget::from_viewports(
                    surface_w as u32,
                    surface_h as u32,
                    tunables::GPU_BUDGET_VIEWPORTS_MOBILE,
                ),
                &mut layer_surfaces,
                surface,
            )?
        };
        ganesh.context.flush_and_submit();
        drop(surface);

        self.egl.swap_buffers()?;
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
        // EGL window surface は ANativeWindow の resize に自動追随する（毎フレーム
        // `surface_size()` で実サイズを取る）ので、ここではレイヤキャッシュを resize する。
        self.presenter.resize(width, height, content_scale);
    }
}

/// GL interface を組んで Ganesh `DirectContext` を作る（current な GL コンテキスト前提）。
/// `new_native()`（skia 内蔵の EGL native interface）を優先し、無ければ `eglGetProcAddress`
/// でシンボルを引く assembled interface に落ちる。
fn make_ganesh_context() -> Result<gpu::DirectContext, String> {
    let interface = gpu::gl::Interface::new_native()
        .or_else(|| {
            gpu::gl::Interface::new_load_with(|name| {
                let Ok(name) = CString::new(name) else {
                    return ptr::null();
                };
                // SAFETY: current なコンテキストの下での標準シンボル解決。
                unsafe { eglGetProcAddress(name.as_ptr()) as *const c_void }
            })
        })
        .ok_or("GrGLInterface creation failed (native and eglGetProcAddress)")?;
    gpu::direct_contexts::make_gl(interface, None)
        .ok_or_else(|| "GrDirectContext::MakeGL returned null".to_string())
}
