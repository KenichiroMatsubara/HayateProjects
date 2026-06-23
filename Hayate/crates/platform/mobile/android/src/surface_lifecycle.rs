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

/// content scale 1.0 で描画する現行 Android 経路の content scale。
///
/// DPI 対応を入れる際は、ここを実機の density から取得した値へ差し替え、`translate_touch`
/// が渡すタッチ座標も同じ scale で再スケールしてヒットテストと描画を揃える。
const ANDROID_CONTENT_SCALE: f32 = 1.0;

/// ネイティブウィンドウ寸法と content scale から論理ビューポート/バッファを導く。
///
/// 計算は Web 経路と共有する `ViewportMetrics::from_physical_size` に集約されている。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn surface_metrics(width: i32, height: i32) -> ViewportMetrics {
    ViewportMetrics::from_physical_size(width, height, ANDROID_CONTENT_SCALE)
}

/// wgpu サーフェス設定のため、ネイティブウィンドウ寸法を最低 1×1 にクランプする。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn window_dimensions(width: i32, height: i32) -> (u32, u32) {
    surface_metrics(width, height).buffer_size()
}

/// クランプ済みサーフェス寸法(物理 px)を `ElementTree` のビューポートへ写す。
///
/// content scale 1.0 で描画するため、レイアウト/ビューポート空間は物理サーフェス
/// ピクセルそのもの。これは `translate_touch` がポインタ API に渡す空間と同じで、
/// ヒットテストが画面描画と揃う。DPI 対応のコンテンツスケーリングを入れる際は、
/// この整合を保つためタッチ座標を同調して再スケールする必要がある。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn viewport_for_surface(width: u32, height: u32) -> (f32, f32) {
    surface_metrics(width as i32, height as i32).viewport_size()
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
        assert_eq!(viewport_for_surface(1080, 1920), (1080.0, 1920.0));
        assert_eq!(viewport_for_surface(1, 1), (1.0, 1.0));
    }
}
