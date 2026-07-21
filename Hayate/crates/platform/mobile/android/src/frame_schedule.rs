//! Android Choreographer latest-wins single-flight scheduling（ADR-0154）。
//!
//! Wake sources only arm one callback; only that callback may commit, using its `frameTimeNanos`.
//! Continuation policy remains in App Host's `FrameContinuation`, so an idle host owns no callback
//! and produces no timer, commit, render, or present work.

/// Choreographer の one-shot callback を最大 1 件に保つ single-flight state。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SingleFlightVsync {
    callback_armed: bool,
}

impl SingleFlightVsync {
    pub fn new() -> Self {
        Self::default()
    }

    /// wake を次の vsync へ集約する。`true` のときだけ Platform Front は callback を post する。
    pub fn request_frame(&mut self) -> bool {
        if self.callback_armed {
            return false;
        }
        self.callback_armed = true;
        true
    }

    /// armed callback を一度だけ消費し、Choreographer の時刻を milliseconds に正規化する。
    pub fn on_vsync(&mut self, frame_time_nanos: i64) -> Option<f64> {
        if !self.callback_armed {
            return None;
        }
        self.callback_armed = false;
        Some(frame_time_nanos as f64 / 1_000_000.0)
    }
}

/// Android NDK Choreographer adapter. The callback owns one temporary `Arc`, so the callback data
/// stays valid even if shutdown races the final posted vsync (the NDK has no cancellation API).
#[cfg(target_os = "android")]
pub struct AndroidFrameScheduler {
    state: std::sync::Arc<std::sync::Mutex<AndroidFrameState>>,
}

#[cfg(target_os = "android")]
#[derive(Default)]
struct AndroidFrameState {
    schedule: SingleFlightVsync,
    ready_timestamp_ms: Option<f64>,
}

#[cfg(target_os = "android")]
impl AndroidFrameScheduler {
    pub fn new() -> Self {
        Self {
            state: std::sync::Arc::new(std::sync::Mutex::new(AndroidFrameState::default())),
        }
    }

    /// Request exactly one callback for the next display vsync. Repeated wakes before that vsync
    /// only update application state; they do not post additional callbacks.
    pub fn request_frame(&self) {
        let should_post = self.state.lock().unwrap().schedule.request_frame();
        if !should_post {
            return;
        }

        let callback_state = std::sync::Arc::into_raw(std::sync::Arc::clone(&self.state));
        unsafe {
            let choreographer = ndk_sys::AChoreographer_getInstance();
            assert!(
                !choreographer.is_null(),
                "AChoreographer_getInstance returned null on the Android main looper"
            );
            ndk_sys::AChoreographer_postFrameCallback(
                choreographer,
                Some(on_choreographer_frame),
                callback_state.cast_mut().cast(),
            );
        }
    }

    /// Consume the one timestamp delivered by the armed callback. `None` means this loop wake was
    /// an input/lifecycle/resource event rather than a display vsync and must not commit a frame.
    pub fn take_frame_timestamp_ms(&self) -> Option<f64> {
        self.state.lock().unwrap().ready_timestamp_ms.take()
    }
}

#[cfg(target_os = "android")]
unsafe extern "C" fn on_choreographer_frame(
    frame_time_nanos: std::os::raw::c_long,
    data: *mut std::ffi::c_void,
) {
    let state =
        unsafe { std::sync::Arc::from_raw(data.cast::<std::sync::Mutex<AndroidFrameState>>()) };
    let mut state = state.lock().unwrap();
    state.ready_timestamp_ms = state.schedule.on_vsync(frame_time_nanos as i64);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_wakes_share_one_choreographer_callback_and_one_timestamp() {
        let mut schedule = SingleFlightVsync::new();

        assert!(
            schedule.request_frame(),
            "first wake must post one callback"
        );
        assert!(
            !schedule.request_frame(),
            "a second wake before vsync must reuse the armed callback"
        );
        assert_eq!(schedule.on_vsync(12_345_678), Some(12.345_678));
        assert_eq!(
            schedule.on_vsync(99_000_000),
            None,
            "one callback may commit App Host at most once"
        );
    }
}
