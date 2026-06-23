//! iOS leaf の surface glue。
//!
//! プラットフォーム非依存のサーフェスライフサイクル状態機械（四論理イベント → GPU
//! サーフェス操作）は `hayate_core::surface_lifecycle` が単一の正本として所有する（ADR-0117）。
//! 本モジュールはその型を re-export し、iOS 固有の glue — UIScene / UIApplication の
//! ライフサイクル（`app.rs`）→ 四論理イベントの写像と、`CAMetalLayer` のドローアブル寸法
//! → 論理ビューポート/バッファの導出 — だけを残す。グルーが落とす UIKit → 論理イベントの
//! 対応は次のとおり:
//!
//! | UIKit                                              | 論理イベント        |
//! |---------------------------------------------------|--------------------|
//! | `scene(_:willConnectTo:)` / 初回 sized CAMetalLayer | `InitWindow`       |
//! | `sceneWillResignActive` / `sceneDidEnterBackground` | `TerminateWindow`  |
//! | `layoutSubviews` / `viewWillTransition` で drawableSize 変化 | `WindowResized` |
//! | `sceneDidDisconnect` / 終了                          | `Destroy`          |
//!
//! 物理ドローアブル寸法から論理ビューポート/バッファを導く計算は、Web/Android 経路と
//! 共有する `hayate_core::ViewportMetrics` に委譲する。Android が content scale を 1.0 に
//! 固定するのに対し、iOS は Retina の実 scale（`UIScreen.scale` = 2.0 / 3.0）を渡すため、
//! iOS が初めて content scale > 1.0 の経路を実走させる（レンダラーは対応済み）。

#[cfg_attr(not(target_os = "ios"), allow(unused_imports))]
pub use hayate_core::{SurfaceLifecycleAction, SurfaceLifecycleEvent, SurfaceLifecycleState};

use hayate_core::ViewportMetrics;

/// content scale 未取得時のフォールバック（scale=1.0 = 非 Retina 相当）。実機では
/// グルーが `UIScreen.scale` / `UIView.contentScaleFactor` を渡すため通常使われない。
#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
pub const IOS_FALLBACK_CONTENT_SCALE: f32 = 1.0;

/// `CAMetalLayer` のドローアブル寸法（物理 px）と content scale から論理ビューポート/
/// バッファを導く。計算は Web/Android 経路と共有する
/// `ViewportMetrics::from_physical_size` に集約されている。
#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
pub fn surface_metrics(width: i32, height: i32, content_scale: f32) -> ViewportMetrics {
    ViewportMetrics::from_physical_size(width, height, content_scale)
}

/// wgpu サーフェス設定のため、ドローアブル寸法を最低 1×1 にクランプする。
#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
pub fn window_dimensions(width: i32, height: i32) -> (u32, u32) {
    surface_metrics(width, height, IOS_FALLBACK_CONTENT_SCALE).buffer_size()
}

/// クランプ済みドローアブル寸法（物理 px）と content scale を `ElementTree` のビューポート
/// （論理 points）へ写す。
///
/// レイアウト/ヒットテストは論理 points 空間で走る。`touch.location(in:view)` も points を
/// 返すため、`translate_touch` がポインタ API に渡す座標とこのビューポート空間が揃い、
/// ヒットテストと描画が一致する。GPU サーフェスのみが物理 px（`buffer_size`）で動く。
#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
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

    // Android と違い iOS は実 content scale を通す: 物理ドローアブル 1170×2532 を scale 3.0
    // で描くと、論理ビューポートは 390×844（iPhone の point 寸法）になる。
    #[test]
    fn viewport_divides_drawable_pixels_by_retina_scale() {
        assert_eq!(viewport_for_surface(1170, 2532, 3.0), (390.0, 844.0));
        assert_eq!(viewport_for_surface(750, 1334, 2.0), (375.0, 667.0));
    }

    // scale 1.0（非 Retina / フォールバック）では論理＝物理で Android と同じ挙動。
    #[test]
    fn viewport_equals_drawable_pixels_at_unit_scale() {
        assert_eq!(viewport_for_surface(800, 600, 1.0), (800.0, 600.0));
    }
}
