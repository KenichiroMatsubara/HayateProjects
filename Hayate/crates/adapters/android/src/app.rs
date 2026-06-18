//! Stage B render + touch loop (ADR-0087): lower an interactive `ElementTree`
//! (`scene_demo`) to a `SceneGraph` and present it each frame to the GPU
//! surface backed by the `ANativeWindow` that `android-activity` hands us.
//! `MotionEvent`s are translated into `hayate-core`'s coordinate-based pointer
//! API, so a tap flips the demo button's `:active` color on screen. IME /
//! AccessKit / clipboard (stage C) are not implemented yet.

use std::time::{Duration, Instant};

use android_activity::input::{InputEvent, MotionAction};
use android_activity::{AndroidApp, MainEvent, PollEvent};
use hayate_core::{ElementTree, SceneGraph};
use hayate_scene_renderer_vello::{
    create_blitter, create_target_view, VelloRenderTarget, VelloSceneRenderer,
};
use wgpu::util::TextureBlitter;

use hayate_core::ElementId;

use crate::ime_input::{apply_ime_action, translate_text_input, TextInputState, TextSpan};
use crate::scene_demo::build_demo_tree;
use crate::surface_lifecycle::{
    viewport_for_surface, window_dimensions, SurfaceLifecycleAction, SurfaceLifecycleState,
};
use crate::touch_input::{translate_touch, PointerInput, TouchAction};

/// RGBA clear color for the stage A smoke test.
pub const CLEAR_COLOR: [f32; 4] = crate::STAGE_A_CLEAR_COLOR;

struct GpuSurface {
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

#[no_mangle]
pub fn android_main(app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );

    let mut gpu: Option<GpuSurface> = None;
    let mut lifecycle = SurfaceLifecycleState::new();
    let mut tree = build_demo_tree();
    let start = Instant::now();
    // Last GameTextInput buffer we synced, and which element the soft keyboard is
    // currently shown for (stage C IME, ADR-0094).
    let mut ime_state = TextInputState::default();
    let mut keyboard_shown_for: Option<ElementId> = None;
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
        sync_ime(&app, &mut tree, &mut ime_state, &mut keyboard_shown_for);

        if let Some(surface) = gpu.as_mut() {
            // Drive layout + cursor blink off a monotonic clock, then present the
            // lowered scene (mirrors `hayate-adapter-web`'s `render`).
            let timestamp_ms = start.elapsed().as_secs_f64() * 1000.0;
            let scene = tree.render(timestamp_ms);
            if let Err(err) = surface.render_frame(scene) {
                log::error!("hayate-adapter-android: render failed: {err}");
            }
        }
    }
}

/// Sync GameTextInput into the focused TextInput (stage C IME, ADR-0094).
///
/// Shows/hides the soft keyboard as focus enters/leaves a *text input* and diffs
/// GameTextInput's absolute buffer into core edit calls. A tap focuses whatever
/// it hits (buttons, plain text, views), so the keyboard is gated on
/// [`ElementTree::focused_text_input`] — keying it on raw focus raised the soft
/// keyboard for every tap, not just editable fields (#392). The diff/apply logic
/// lives in the host-testable [`crate::ime_input`]; this wrapper is thin glue
/// over `android-activity`'s text-input API and is verified on-device (#195).
fn sync_ime(
    app: &AndroidApp,
    tree: &mut ElementTree,
    prev: &mut TextInputState,
    keyboard_shown_for: &mut Option<ElementId>,
) {
    let target = tree.focused_text_input();

    if *keyboard_shown_for != target {
        match target {
            Some(_) => app.show_soft_input(true),
            None => app.hide_soft_input(true),
        }
        *keyboard_shown_for = target;
        // A fresh focus starts from an empty baseline buffer.
        *prev = TextInputState::default();
    }

    let Some(target) = target else {
        return;
    };

    // GameTextInput reports the full buffer plus an optional composing span
    // (byte offsets into `text`); mirror it into the NDK-free type and diff.
    let next = app.text_input_state(|state| TextInputState {
        text: state.text.clone(),
        compose_region: state
            .compose_region
            .map(|span| TextSpan {
                start: span.start,
                end: span.end,
            }),
    });

    if next != *prev {
        for action in translate_text_input(prev, &next) {
            apply_ime_action(tree, target, &action);
        }
        *prev = next;
    }
}

/// Drain pending `MotionEvent`s and drive `tree`'s coordinate-based pointer API.
///
/// Single-pointer tap/drag only (ADR-0082 stage B); multi-touch gestures and
/// scroll inertia (ADR-0046) are out of scope. The per-event math lives in the
/// host-testable [`translate_touch`]; this wrapper is thin NDK glue.
fn process_touch_input(app: &AndroidApp, tree: &mut ElementTree) {
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
            android_activity::input::InputStatus::Unhandled
        });
        if !read {
            break;
        }
    }
}

/// Map an Android `MotionAction` to a single-pointer [`TouchAction`], or `None`
/// for actions outside the basic tap/drag set (hover, scroll, buttons, …).
fn motion_action_to_touch(action: MotionAction) -> Option<TouchAction> {
    match action {
        MotionAction::Down | MotionAction::PointerDown => Some(TouchAction::Down),
        MotionAction::Move => Some(TouchAction::Move),
        MotionAction::Up | MotionAction::PointerUp => Some(TouchAction::Up),
        MotionAction::Cancel => Some(TouchAction::Cancel),
        _ => None,
    }
}

async fn init_gpu_surface(window: &ndk::native_window::NativeWindow) -> Result<GpuSurface, String> {
    let (width, height) = window_dimensions(window.width(), window.height());

    let instance = wgpu::Instance::default();

    // SAFETY: `window` outlives the surface for the lifetime of this adapter
    // (recreated on `InitWindow`, dropped on `TerminateWindow`).
    let surface = unsafe {
        instance
            .create_surface_unsafe(
                wgpu::SurfaceTargetUnsafe::from_window(window)
                    .map_err(|e| format!("SurfaceTargetUnsafe::from_window: {e}"))?,
            )
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
    fn render_frame(&mut self, scene: &SceneGraph) -> Result<(), String> {
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

    fn resize(&mut self, width: u32, height: u32) {
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
