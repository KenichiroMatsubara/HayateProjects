//! Platform-agnostic surface lifecycle state machine for the stage A smoke test.
//!
//! `android-activity` delivers `MainEvent`s on a background thread; this module
//! maps those events to the GPU surface actions the adapter must take. It is
//! compiled on all targets so behavior can be verified without the NDK.

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

/// Tracks whether a GPU surface is currently bound to the native window.
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

/// Clamp native window dimensions to at least 1×1 for wgpu surface configuration.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn window_dimensions(width: i32, height: i32) -> (u32, u32) {
    (width.max(1) as u32, height.max(1) as u32)
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
}
