//! Android leaf の surface glue。
//!
//! プラットフォーム非依存のサーフェスライフサイクル状態機械（四論理イベント → GPU
//! サーフェス操作）は `hayate_core::surface_lifecycle` が単一の正本として所有する（ADR-0117）。
//! 本モジュールはその型を re-export し、Android 固有の glue — `android-activity` の
//! `MainEvent`（`app.rs`）→ 四論理イベントの写像と、ネイティブウィンドウ寸法 → 論理
//! ビューポート/バッファの導出 — だけを残す。
//!
//! 物理サーフェス寸法から論理ビューポート/バッファを導く計算は、Web 経路と共有する
//! `hayate_core::ViewportMetrics` に委譲する（content scale を 1.0 で渡すのが Android の差）。

#[cfg_attr(not(target_os = "android"), allow(unused_imports))]
pub use hayate_core::{SurfaceLifecycleAction, SurfaceLifecycleEvent, SurfaceLifecycleState};

use hayate_core::ViewportMetrics;

/// Android の baseline density（160dpi = 等倍。Web の CSS px と同じ意味論）。
#[cfg(target_os = "android")]
const BASELINE_DENSITY_DPI: f32 = 160.0;

/// 実機の density から content scale（DPI 倍率）を導く。取得できなければ等倍（1.0）。
///
/// レイアウト/ヒットテストは論理px（Web の CSS px 相当）で動くため、実密度を反映しないと
/// 高密度端末で物理px＝論理pxとして扱われ、レイアウトが本来よりずっと広いビューポートだと
/// 誤認して（例: 3x密度の1080px物理幅を1080論理pxとして扱う）、デスクトップ向けの
/// styleVariants（`maxWidth` 判定）が誤って選ばれる。
#[cfg(target_os = "android")]
pub fn content_scale(app: &android_activity::AndroidApp) -> f32 {
    app.config()
        .density()
        .map(|dpi| (dpi as f32 / BASELINE_DENSITY_DPI).max(1.0))
        .unwrap_or(1.0)
}

/// ネイティブウィンドウ寸法と content scale から論理ビューポート/バッファを導く。
///
/// 計算は Web 経路と共有する `ViewportMetrics::from_physical_size` に集約されている。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn surface_metrics(width: i32, height: i32, content_scale: f32) -> ViewportMetrics {
    ViewportMetrics::from_physical_size(width, height, content_scale)
}

/// wgpu サーフェス設定のため、ネイティブウィンドウ寸法を最低 1×1 にクランプする。バッファは
/// 常に物理 px そのものなので content scale に依存しない。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn window_dimensions(width: i32, height: i32) -> (u32, u32) {
    surface_metrics(width, height, 1.0).buffer_size()
}

/// クランプ済みサーフェス寸法(物理 px)と content scale から `ElementTree` の論理ビューポートを導く。
///
/// レイアウト/ヒットテストは論理px空間（`viewport_width = buffer_width / content_scale`）で動く。
/// 描画は Vello（`hayate-scene-renderer-vello`）側で同じ content_scale を掛けて物理バッファへ
/// 引き伸ばす（`VelloLayerRasterizer`）。タッチ座標も `process_touch_input` が同じ scale で
/// 論理px化してから `translate_touch` に渡し、ヒットテストと描画を揃える。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn viewport_for_surface(width: u32, height: u32, content_scale: f32) -> (f32, f32) {
    surface_metrics(width as i32, height as i32, content_scale).viewport_size()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_dimensions_clamp_to_at_least_one_pixel() {
        assert_eq!(window_dimensions(0, -3), (1, 1));
        assert_eq!(window_dimensions(640, 480), (640, 480));
    }

    #[test]
    fn viewport_tracks_surface_pixels_at_unit_scale() {
        assert_eq!(viewport_for_surface(1080, 1920, 1.0), (1080.0, 1920.0));
        assert_eq!(viewport_for_surface(1, 1, 1.0), (1.0, 1.0));
    }

    #[test]
    fn viewport_divides_by_content_scale_for_high_density_screens() {
        // 3x密度（480dpi 相当）の 1080px 幅は論理 360px。styleVariants の maxWidth 判定が
        // デスクトップ幅と誤認しないよう、物理pxをそのまま論理pxに使ってはいけない。
        assert_eq!(viewport_for_surface(1080, 2400, 3.0), (360.0, 800.0));
    }
}
