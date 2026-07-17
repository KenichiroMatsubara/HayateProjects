//! プラットフォーム非依存のビューポート/バッファ寸法導出。
//!
//! Web（CSS px + `devicePixelRatio`）と Android（物理サーフェス px + content scale）の
//! どちらも、最終的には次の三つを同じ規約で得たい:
//!
//! - **論理ビューポート**（レイアウト/CSS px・f32）— `ElementTree::set_viewport` 入力
//! - **バッキングストア**（物理 px・u32）— `Scene Renderer` のサーフェス設定入力
//! - **content scale**（dpr・f32）— 論理→物理の変換係数
//!
//! 従来は Web の `CanvasResizeMetrics` と Android の `window_dimensions` /
//! `viewport_for_surface` に同じ計算が二重実装されていた。本モジュールが正本を持ち、
//! Platform Adapter は raw な width/height の取得だけを担う（ADR-0080）。

/// 論理ビューポート（CSS px・f32）、バッキングストア（物理 px・u32）、content scale（dpr）。
///
/// `viewport_*` は CSS レイアウト座標、`buffer_*` は物理ピクセル、`content_scale` は
/// その比（常に `>= 1.0`）。Web と Android の Platform Adapter が共有する。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewportMetrics {
    pub viewport_width: f32,
    pub viewport_height: f32,
    pub buffer_width: u32,
    pub buffer_height: u32,
    pub content_scale: f32,
}

impl ViewportMetrics {
    /// 論理（CSS）寸法と content scale から導出する（Web 経路）。
    ///
    /// `viewport_*` は CSS px をそのまま（負値は 0 にクランプ）保持し、`buffer_*` は
    /// `論理 × scale` を四捨五入して最小 1px にクランプした物理 px。`scale` は最低 1.0。
    pub fn from_logical_size(logical_width: f32, logical_height: f32, content_scale: f64) -> Self {
        let viewport_width = logical_width.max(0.0);
        let viewport_height = logical_height.max(0.0);
        let scale = content_scale.max(1.0);
        let buffer_width = (f64::from(viewport_width) * scale).round().max(1.0) as u32;
        let buffer_height = (f64::from(viewport_height) * scale).round().max(1.0) as u32;
        Self {
            viewport_width,
            viewport_height,
            buffer_width,
            buffer_height,
            content_scale: scale as f32,
        }
    }

    /// 物理サーフェス寸法と content scale から導出する（Android 経路）。
    ///
    /// `buffer_*` はネイティブウィンドウ px を最小 1px にクランプした物理 px、`viewport_*`
    /// は `物理 / scale` の論理 px。`scale` は最低 1.0。content scale 1.0 では論理＝物理で、
    /// content scale 1.0 で描画する現行 Android 挙動と一致する。DPI 対応を入れる際は、
    /// ヒットテストと描画を揃えるためタッチ座標も同じ `scale` で再スケールすること。
    pub fn from_physical_size(
        physical_width: i32,
        physical_height: i32,
        content_scale: f32,
    ) -> Self {
        let scale = content_scale.max(1.0);
        let buffer_width = physical_width.max(1) as u32;
        let buffer_height = physical_height.max(1) as u32;
        Self {
            viewport_width: buffer_width as f32 / scale,
            viewport_height: buffer_height as f32 / scale,
            buffer_width,
            buffer_height,
            content_scale: scale,
        }
    }

    /// `ElementTree::set_viewport` へ渡す論理ビューポート寸法。
    pub fn viewport_size(&self) -> (f32, f32) {
        (self.viewport_width, self.viewport_height)
    }

    /// `Scene Renderer` のサーフェス設定へ渡すバッキングストア寸法。
    pub fn buffer_size(&self) -> (u32, u32) {
        (self.buffer_width, self.buffer_height)
    }
}

/// 論理ビューポートが意味のある量だけ変化したか（サブピクセルの揺れは無視する）。
///
/// `ResizeObserver` の連続報告や端数の異なる物理→論理換算で起きる微小なドリフトで
/// レイアウトを作り直さないためのしきい値判定。
pub fn viewport_size_changed(previous: (f32, f32), next: (f32, f32)) -> bool {
    const EPSILON: f32 = 0.5;
    (previous.0 - next.0).abs() > EPSILON || (previous.1 - next.1).abs() > EPSILON
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_logical_size_uses_css_viewport_and_dpr_scaled_buffer() {
        let metrics = ViewportMetrics::from_logical_size(400.0, 300.0, 2.0);
        assert_eq!(
            metrics,
            ViewportMetrics {
                viewport_width: 400.0,
                viewport_height: 300.0,
                buffer_width: 800,
                buffer_height: 600,
                content_scale: 2.0,
            }
        );
    }

    #[test]
    fn from_logical_size_with_unit_dpr_matches_css_size() {
        let metrics = ViewportMetrics::from_logical_size(640.0, 480.0, 1.0);
        assert_eq!(metrics.buffer_size(), (640, 480));
        assert_eq!(metrics.viewport_size(), (640.0, 480.0));
        assert_eq!(metrics.content_scale, 1.0);
    }

    #[test]
    fn from_logical_size_clamps_negative_to_zero_viewport_and_unit_buffer() {
        let metrics = ViewportMetrics::from_logical_size(-10.0, -5.0, 2.0);
        assert_eq!(metrics.viewport_size(), (0.0, 0.0));
        assert_eq!(metrics.buffer_size(), (1, 1));
    }

    #[test]
    fn from_logical_size_clamps_sub_unit_scale_to_one() {
        // dpr < 1 は物理ピクセルを論理より小さくしてしまうため 1.0 にクランプ。
        let metrics = ViewportMetrics::from_logical_size(200.0, 100.0, 0.5);
        assert_eq!(metrics.content_scale, 1.0);
        assert_eq!(metrics.buffer_size(), (200, 100));
    }

    #[test]
    fn from_physical_size_at_unit_scale_keeps_logical_equal_to_physical() {
        let metrics = ViewportMetrics::from_physical_size(1080, 1920, 1.0);
        assert_eq!(metrics.buffer_size(), (1080, 1920));
        assert_eq!(metrics.viewport_size(), (1080.0, 1920.0));
        assert_eq!(metrics.content_scale, 1.0);
    }

    #[test]
    fn from_physical_size_clamps_to_at_least_one_pixel() {
        let metrics = ViewportMetrics::from_physical_size(0, -3, 1.0);
        assert_eq!(metrics.buffer_size(), (1, 1));
        assert_eq!(metrics.viewport_size(), (1.0, 1.0));
    }

    #[test]
    fn from_physical_size_derives_logical_viewport_from_scale() {
        // 物理 800×600 を dpr 2.0 で描く → 論理ビューポートは 400×300。
        let metrics = ViewportMetrics::from_physical_size(800, 600, 2.0);
        assert_eq!(metrics.buffer_size(), (800, 600));
        assert_eq!(metrics.viewport_size(), (400.0, 300.0));
        assert_eq!(metrics.content_scale, 2.0);
    }

    #[test]
    fn web_and_android_agree_on_buffer_for_the_same_physical_surface() {
        // 同じ物理 800×600 サーフェス / dpr 2.0 を、Web は論理 400×300 から、
        // Android は物理 800×600 から導いても、両者の buffer と content scale は一致する。
        let web = ViewportMetrics::from_logical_size(400.0, 300.0, 2.0);
        let android = ViewportMetrics::from_physical_size(800, 600, 2.0);
        assert_eq!(web.buffer_size(), android.buffer_size());
        assert_eq!(web.viewport_size(), android.viewport_size());
        assert_eq!(web.content_scale, android.content_scale);
    }

    #[test]
    fn viewport_size_changed_ignores_sub_pixel_drift() {
        assert!(!viewport_size_changed((800.0, 600.0), (800.2, 600.3)));
    }

    #[test]
    fn viewport_size_changed_detects_css_pixel_changes() {
        assert!(viewport_size_changed((800.0, 600.0), (801.0, 600.0)));
        assert!(viewport_size_changed((800.0, 600.0), (800.0, 601.0)));
    }
}
