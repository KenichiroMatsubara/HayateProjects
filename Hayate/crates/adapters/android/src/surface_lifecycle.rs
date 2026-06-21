//! プラットフォーム非依存のサーフェスライフサイクル状態機械。
//!
//! `android-activity` は `MainEvent` をバックグラウンドスレッドで配送する。本
//! モジュールはそれらのイベントをアダプタが取るべき GPU サーフェス操作へ写像する。
//! NDK なしで挙動を検証できるよう全ターゲットでコンパイルする。
//!
//! 物理サーフェス寸法から論理ビューポート/バッファを導く計算は、Web 経路と共有する
//! `hayate_core::ViewportMetrics` に委譲する（width/height 取得の抽象化）。

use hayate_core::ViewportMetrics;

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceLifecycleEvent {
    InitWindow,
    TerminateWindow,
    WindowResized { width: u32, height: u32 },
    Destroy,
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceLifecycleAction {
    CreateSurface,
    DestroySurface,
    ResizeSurface { width: u32, height: u32 },
    Quit,
    NoOp,
}

/// GPU サーフェスが現在ネイティブウィンドウに束縛されているかを追跡する。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceLifecycleState {
    surface_active: bool,
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
impl SurfaceLifecycleState {
    pub fn new() -> Self {
        Self {
            surface_active: false,
        }
    }

    pub fn surface_active(&self) -> bool {
        self.surface_active
    }

    pub fn handle(&mut self, event: SurfaceLifecycleEvent) -> SurfaceLifecycleAction {
        match event {
            SurfaceLifecycleEvent::InitWindow => {
                self.surface_active = true;
                SurfaceLifecycleAction::CreateSurface
            }
            SurfaceLifecycleEvent::TerminateWindow => {
                if self.surface_active {
                    self.surface_active = false;
                    SurfaceLifecycleAction::DestroySurface
                } else {
                    SurfaceLifecycleAction::NoOp
                }
            }
            SurfaceLifecycleEvent::WindowResized { width, height } => {
                if self.surface_active {
                    SurfaceLifecycleAction::ResizeSurface { width, height }
                } else {
                    SurfaceLifecycleAction::NoOp
                }
            }
            SurfaceLifecycleEvent::Destroy => {
                self.surface_active = false;
                SurfaceLifecycleAction::Quit
            }
        }
    }
}

impl Default for SurfaceLifecycleState {
    fn default() -> Self {
        Self::new()
    }
}

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
    fn init_window_requests_surface_creation() {
        let mut state = SurfaceLifecycleState::new();
        assert_eq!(
            state.handle(SurfaceLifecycleEvent::InitWindow),
            SurfaceLifecycleAction::CreateSurface
        );
        assert!(state.surface_active());
    }

    #[test]
    fn terminate_window_drops_active_surface() {
        let mut state = SurfaceLifecycleState::new();
        state.handle(SurfaceLifecycleEvent::InitWindow);
        assert_eq!(
            state.handle(SurfaceLifecycleEvent::TerminateWindow),
            SurfaceLifecycleAction::DestroySurface
        );
        assert!(!state.surface_active());
    }

    #[test]
    fn window_resized_updates_active_surface() {
        let mut state = SurfaceLifecycleState::new();
        state.handle(SurfaceLifecycleEvent::InitWindow);
        assert_eq!(
            state.handle(SurfaceLifecycleEvent::WindowResized {
                width: 1080,
                height: 1920,
            }),
            SurfaceLifecycleAction::ResizeSurface {
                width: 1080,
                height: 1920,
            }
        );
        assert!(state.surface_active());
    }

    #[test]
    fn window_resized_before_init_is_ignored() {
        let mut state = SurfaceLifecycleState::new();
        assert_eq!(
            state.handle(SurfaceLifecycleEvent::WindowResized {
                width: 800,
                height: 600,
            }),
            SurfaceLifecycleAction::NoOp
        );
        assert!(!state.surface_active());
    }

    #[test]
    fn destroy_quits_and_clears_surface_state() {
        let mut state = SurfaceLifecycleState::new();
        state.handle(SurfaceLifecycleEvent::InitWindow);
        assert_eq!(
            state.handle(SurfaceLifecycleEvent::Destroy),
            SurfaceLifecycleAction::Quit
        );
        assert!(!state.surface_active());
    }

    #[test]
    fn terminate_window_without_active_surface_is_noop() {
        let mut state = SurfaceLifecycleState::new();
        assert_eq!(
            state.handle(SurfaceLifecycleEvent::TerminateWindow),
            SurfaceLifecycleAction::NoOp
        );
        assert!(!state.surface_active());
    }

    #[test]
    fn background_foreground_cycle_recreates_surface() {
        let mut state = SurfaceLifecycleState::new();
        state.handle(SurfaceLifecycleEvent::InitWindow);
        state.handle(SurfaceLifecycleEvent::TerminateWindow);
        assert!(!state.surface_active());
        assert_eq!(
            state.handle(SurfaceLifecycleEvent::InitWindow),
            SurfaceLifecycleAction::CreateSurface
        );
        assert!(state.surface_active());
    }

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
