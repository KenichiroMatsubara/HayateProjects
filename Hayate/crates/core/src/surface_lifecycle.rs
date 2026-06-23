//! プラットフォーム非依存のサーフェスライフサイクル状態機械（ADR-0117 フェーズ1）。
//!
//! GPU サーフェスのライフサイクルは、ネイティブのウィンドウ/シーンイベントを四つの論理
//! イベント（`InitWindow` / `TerminateWindow` / `WindowResized` / `Destroy`）へ畳んだうえで、
//! アダプタが取るべき GPU サーフェス操作（`SurfaceLifecycleAction`）へ写像する純粋な状態
//! 機械で表せる。芯はプラットフォーム非依存で、かつては `hayate-adapter-android` と
//! `hayate-adapter-ios` の双方に同型のまま複製されていた。本モジュールがその単一の正本を
//! 持ち、各 leaf には native（UIScene / android-activity）→ 四論理イベントへの glue だけを
//! 残す。
//!
//! 物理ドローアブル寸法から論理ビューポート/バッファを導く計算は状態機械の責務ではなく、
//! Web/Android/iOS 経路が共有する [`crate::ViewportMetrics`] に委譲する（content scale を
//! どの値で渡すか — Android は 1.0、iOS は Retina の実 scale — は leaf glue 側の差）。
//!
//! 実機 SDK や wgpu サーフェスを要さず全ターゲットでコンパイル/テストできる。

/// ネイティブのウィンドウ/シーンイベントを畳んだ四つの論理遷移。
///
/// leaf glue が広いネイティブイベント集合（android-activity の `MainEvent`、UIKit の
/// UIScene ライフサイクル等）をこの四イベントへ落とす。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceLifecycleEvent {
    InitWindow,
    TerminateWindow,
    WindowResized { width: u32, height: u32 },
    Destroy,
}

/// 論理イベントから導かれる、アダプタが取るべき GPU サーフェス操作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceLifecycleAction {
    CreateSurface,
    DestroySurface,
    ResizeSurface { width: u32, height: u32 },
    Quit,
    NoOp,
}

/// GPU サーフェスが現在ネイティブのドローアブル（android-activity のウィンドウ /
/// `CAMetalLayer`）に束縛されているかを追跡する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceLifecycleState {
    surface_active: bool,
}

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
                width: 1170,
                height: 2532,
            }),
            SurfaceLifecycleAction::ResizeSurface {
                width: 1170,
                height: 2532,
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

    // 背景化（TerminateWindow）→ 前景復帰（InitWindow）のサイクルでサーフェスが破棄され
    // 再生成される。iOS では Metal ドローアブルが背景で無効になるため、この再生成が必須。
    // Android も同じ状態機械でホームボタン → 復帰の再生成を表す。
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
}
