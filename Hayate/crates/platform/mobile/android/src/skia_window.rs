//! skia raster の `ANativeWindow` 提示面（issue #802・ADR-0146 §3）。
//!
//! wgpu 非依存の CPU present 経路——skia の CPU raster surface へ 1 フレーム焼き
//! （[`crate::skia_present::raster_frame_rgbx`]、ホストテストと同一経路）、`ANativeWindow_lock`
//! / `ANativeWindow_unlockAndPost`（`ndk::native_window::NativeWindow::lock`）で直接 present
//! する。vello の `GpuSurface`（`app.rs`）と並立する Renderer Selection Policy の一方向
//! fallback 先——GPU adapter が一切無い/初期化に失敗する端末でも Android が描画を出せる。
//!
//! present 形式は RGBX_8888（4byte/px、アルファ無視。`skia_present::copy_rgba_to_rgbx` 参照）。
//! `set_buffers_geometry` で surface 作成時・resize 時にだけ形式/寸法を通知し、毎フレームは
//! `lock().lines()` で行ごとに書く（stride を気にしない）。

use std::collections::HashSet;
use std::mem::MaybeUninit;

use hayate_core::{ElementId, SceneGraph};
use hayate_layer_compositor::PresentPlanner;
use ndk::hardware_buffer_format::HardwareBufferFormat;
use ndk::native_window::NativeWindow;

use crate::skia_present::raster_frame_rgbx;

/// skia raster の一方向 fallback 先が使う CPU present 面。`GpuSurface`（`app.rs`）と対の型で、
/// 同じ `RasterCommand` チャネル越しに専用 Raster スレッド上で駆動される（ADR-0128）。
pub(crate) struct SkiaGpuSurface {
    /// `ANativeWindow_acquire` 済みの独立参照（`NativeWindow::clone`）。Raster スレッドへ move
    /// して所有させる（vello の wgpu surface と同じ move-after-creation パターン）。
    window: NativeWindow,
    width: u32,
    height: u32,
    content_scale: f32,
    /// present 側 raster gating（vello の `GpuSurface::planner` と同型、#632/#687）。
    planner: PresentPlanner,
}

/// `window` を skia raster 用の CPU present 面として立てる（RGBX_8888、フルウィンドウ）。
/// GPU adapter を一切要求しないため常に同期・ほぼ非失敗——`set_buffers_geometry` の失敗だけ
/// `Err` として返す。
pub(crate) fn init_skia_surface(
    window: &NativeWindow,
    content_scale: f32,
) -> Result<SkiaGpuSurface, String> {
    let (width, height) =
        crate::surface_lifecycle::window_dimensions(window.width(), window.height());
    // 独立参照を保持する（ANativeWindow_acquire）。UI スレッドの `window` はイベントハンドラの
    // スコープで drop されうるが、OS 側の ANativeWindow 自体は生存しているため参照カウントで
    // 安全に生き続ける（`app.rs::init_gpu_surface` の SAFETY コメントと同じ前提）。
    let window = window.clone();
    window
        .set_buffers_geometry(width as i32, height as i32, Some(HardwareBufferFormat::R8G8B8X8_UNORM))
        .map_err(|e| format!("ANativeWindow_setBuffersGeometry: {e}"))?;

    Ok(SkiaGpuSurface {
        window,
        width,
        height,
        content_scale,
        planner: PresentPlanner::new(),
    })
}

impl SkiaGpuSurface {
    /// 1 フレームの提示（vello 側 `GpuSurface::render_frame` と同じ raster gating・safe-area
    /// offset の扱い）。`plan().needs_raster` なら skia で全面 raster してから present する。
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

        // b2（edge-to-edge, issue #794・ADR-0144）: vello 経路（`GpuSurface::render_frame`）と
        // 同じ安全領域平行移動を skia 側でも適用する。
        let (origin_x, origin_y) = crate::safe_area::pushed_insets()
            .map(|insets| insets.scene_origin(self.content_scale))
            .unwrap_or((0.0, 0.0));

        let rgbx = raster_frame_rgbx(
            scene,
            self.width,
            self.height,
            self.content_scale,
            crate::app::CLEAR_COLOR,
            origin_x,
            origin_y,
        );
        present_rgbx(&self.window, self.width, &rgbx)?;
        self.planner.note_full_raster(layers);
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
        if let Err(e) = self.window.set_buffers_geometry(
            width as i32,
            height as i32,
            Some(HardwareBufferFormat::R8G8B8X8_UNORM),
        ) {
            log::warn!("hayate-adapter-android: skia set_buffers_geometry (resize) failed: {e}");
        }
        self.planner.invalidate();
    }
}

/// tightly-packed RGBX8888 の `rgbx`（`width * height * 4` バイト）を `window` の次回描画
/// バッファへ、stride を気にせず行単位で書いて present する（`ANativeWindow_lock` /
/// `ANativeWindow_unlockAndPost`、`NativeWindowBufferLockGuard::lines()` 越し）。
fn present_rgbx(window: &NativeWindow, width: u32, rgbx: &[u8]) -> Result<(), String> {
    let mut guard = window.lock(None).map_err(|e| format!("ANativeWindow_lock: {e}"))?;
    let Some(lines) = guard.lines() else {
        return Err("ANativeWindow buffer format has no known bytes_per_pixel".to_string());
    };
    let row_bytes = width as usize * 4;
    for (dst_row, src_row) in lines.zip(rgbx.chunks_exact(row_bytes)) {
        for (d, s) in dst_row.iter_mut().zip(src_row.iter()) {
            *d = MaybeUninit::new(*s);
        }
    }
    // guard drop（ここでスコープ末尾）で ANativeWindow_unlockAndPost。
    Ok(())
}
