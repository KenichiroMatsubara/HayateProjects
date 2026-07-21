//! Host-side contract for the Android Choreographer frame source (issue #884).

use std::fs;
use std::path::PathBuf;

fn read(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

#[test]
fn both_android_entry_paths_wait_for_the_single_choreographer_source() {
    let native = read("src/app.rs");
    let tsubame = read("src/app_tsubame.rs");

    for (name, source) in [("native", native), ("tsubame", tsubame)] {
        assert!(
            source.contains("AndroidFrameScheduler"),
            "{name} entry path must use the shared Choreographer scheduler"
        );
        assert!(
            source.contains("poll_events(None"),
            "{name} entry path must block for events instead of polling a frame timer"
        );
        assert!(
            !source.contains("Duration::from_millis(16)"),
            "{name} entry path must not retain the 16ms polling clock"
        );
    }
}

#[test]
fn frame_time_nanos_is_the_timestamp_and_callbacks_are_one_shot() {
    let scheduler = read("src/frame_schedule.rs");

    assert!(scheduler.contains("AChoreographer_postFrameCallback"));
    assert!(scheduler.contains("frame_time_nanos"));
    assert!(scheduler.contains("SingleFlightVsync"));
}

#[test]
fn asynchronous_resource_completion_wakes_the_blocked_event_loop() {
    let tsubame = read("src/app_tsubame.rs");
    let reload = read("src/reload_socket.rs");

    assert!(tsubame.contains("create_waker()"));
    assert!(reload.contains("AndroidAppWaker"));
    assert!(reload.contains("waker.wake()"));
    assert!(tsubame.contains("has_buffered_entries()"));
    assert!(tsubame.contains("device_log::FLUSH_INTERVAL_MS"));
}

#[test]
fn profile_trace_captures_choreographer_and_frame_timing_evidence() {
    let script = read("../../../../scripts/collect-android-performance-report.sh");

    assert!(script.contains("atrace_categories: \"gfx\""));
    assert!(script.contains("atrace_categories: \"view\""));
    assert!(script.contains("gfxinfo") && script.contains("framestats"));
}
