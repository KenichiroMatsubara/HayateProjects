//! skia raster の `ANativeWindow` 提示面（issue #802・ADR-0146 §3）。
//!
//! wgpu 非依存の CPU present 経路——skia-safe のレイヤ cache/composite surface を
//! `ANativeWindow_lock` / `ANativeWindow_unlockAndPost`
//! （`ndk::native_window::NativeWindow::lock`）で直接 present する。vello の `GpuSurface`
//! （`app.rs`）と並立する Renderer Selection Policy の一方向 fallback 先——GPU adapter が
//! 一切無い/初期化に失敗する端末でも Android が描画を出せる。
//!
//! present 形式は RGBX_8888（4byte/px、アルファ無視。`skia_present::copy_rgba_to_rgbx` 参照）。
//! `set_buffers_geometry` で surface 作成時・resize 時にだけ形式/寸法を通知し、毎フレームは
//! `lock().lines()` で行ごとに書く（stride を気にしない）。

use std::mem::MaybeUninit;

use hayate_core::{LayerTopology, SceneSnapshot, ScrollCompositorInput};
use hayate_layer_compositor::{
    scroll_layer_geometry_from_inputs, DeviceMemoryClass, GpuBudget, RenderResourceBudgetPolicy,
    ResourceBudgetInputs,
};
use hayate_scene_renderer_skia::{new_raster_surface, read_rgba, SkiaLayerPresenter};
use ndk::hardware_buffer_format::HardwareBufferFormat;
use ndk::native_window::NativeWindow;

use crate::skia_present::copy_rgba_to_rgbx;

/// skia raster の一方向 fallback 先が使う CPU present 面。`GpuSurface`（`app.rs`）と対の型で、
/// 同じ `RasterCommand` チャネル越しに専用 Raster スレッド上で駆動される（ADR-0128）。
pub(crate) struct SkiaGpuSurface {
    /// `ANativeWindow_acquire` 済みの独立参照（`NativeWindow::clone`）。Raster スレッドへ move
    /// して所有させる（vello の wgpu surface と同じ move-after-creation パターン）。
    window: NativeWindow,
    width: u32,
    height: u32,
    content_scale: f32,
    presenter: SkiaLayerPresenter,
    resource_policy: RenderResourceBudgetPolicy,
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
        .set_buffers_geometry(
            width as i32,
            height as i32,
            Some(HardwareBufferFormat::R8G8B8X8_UNORM),
        )
        .map_err(|e| format!("ANativeWindow_setBuffersGeometry: {e}"))?;

    let resource_policy = RenderResourceBudgetPolicy::for_device(ResourceBudgetInputs::new(
        DeviceMemoryClass::Constrained,
        width,
        height,
    ));
    let mut presenter = SkiaLayerPresenter::new(width, height, content_scale);
    presenter.configure_resource_residency(resource_policy);

    Ok(SkiaGpuSurface {
        window,
        width,
        height,
        content_scale,
        presenter,
        resource_policy,
    })
}

impl SkiaGpuSurface {
    /// 1 フレームの提示。dirty layer だけを再 raster し、clean layer と scroll overscan 帯は
    /// skia-safe image cache から合成する。safe-area offset は vello 経路と同じ値を使う。
    pub(crate) fn render_frame(
        &mut self,
        scene: &SceneSnapshot,
        topology: &LayerTopology,
        scroll_inputs: &[ScrollCompositorInput],
    ) -> Result<(), String> {
        let scroll_geometry = scroll_layer_geometry_from_inputs(scroll_inputs);

        // b2（edge-to-edge, issue #794・ADR-0144）: vello 経路（`GpuSurface::render_frame`）と
        // 同じ安全領域平行移動を skia 側でも適用する。
        let (origin_x, origin_y) = crate::safe_area::pushed_insets()
            .map(|insets| insets.scene_origin(self.content_scale))
            .unwrap_or((0.0, 0.0));

        let target = new_raster_surface(self.width as i32, self.height as i32)
            .ok_or_else(|| format!("skia present surface {}x{}", self.width, self.height))?;
        let mut target = self.presenter.present(
            scene,
            topology,
            &scroll_geometry,
            crate::app::CLEAR_COLOR,
            (origin_x, origin_y),
            GpuBudget::from_bytes(self.resource_policy.cpu.max_bytes),
            target,
        )?;
        let rgba = read_rgba(&mut target);
        let mut rgbx = vec![0u8; rgba.len()];
        copy_rgba_to_rgbx(&rgba, &mut rgbx);
        present_rgbx(&self.window, self.width, &rgbx)?;
        Ok(())
    }

    pub(crate) fn resize(
        &mut self,
        width: u32,
        height: u32,
        content_scale: f32,
    ) -> Result<(), String> {
        let content_scale = content_scale.max(1.0);
        if width == 0
            || height == 0
            || (width == self.width && height == self.height && content_scale == self.content_scale)
        {
            return Ok(());
        }
        self.width = width;
        self.height = height;
        self.content_scale = content_scale;
        self.resource_policy = RenderResourceBudgetPolicy::for_device(ResourceBudgetInputs::new(
            DeviceMemoryClass::Constrained,
            width,
            height,
        ));
        self.presenter
            .configure_resource_residency(self.resource_policy);
        self.window
            .set_buffers_geometry(
                width as i32,
                height as i32,
                Some(HardwareBufferFormat::R8G8B8X8_UNORM),
            )
            .map_err(|error| format!("skia set_buffers_geometry (resize): {error}"))?;
        self.presenter.resize(width, height, content_scale);
        Ok(())
    }
}

/// tightly-packed RGBX8888 の `rgbx`（`width * height * 4` バイト）を `window` の次回描画
/// バッファへ、stride を気にせず行単位で書いて present する（`ANativeWindow_lock` /
/// `ANativeWindow_unlockAndPost`、`NativeWindowBufferLockGuard::lines()` 越し）。
fn present_rgbx(window: &NativeWindow, width: u32, rgbx: &[u8]) -> Result<(), String> {
    let mut guard = window
        .lock(None)
        .map_err(|e| format!("ANativeWindow_lock: {e}"))?;
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
