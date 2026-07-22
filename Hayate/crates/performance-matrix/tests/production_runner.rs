use std::path::PathBuf;
use std::process::Command;

use hayate_performance_matrix::RunEvidence;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn production_entrypoint_has_interrupt_safe_refresh_and_raw_android_capture() {
    let root = workspace_root();
    let main = std::fs::read_to_string(root.join("crates/performance-matrix/src/main.rs"))
        .expect("runner source");
    let collector =
        std::fs::read_to_string(root.join("scripts/collect-performance-matrix-case.py"))
            .expect("collector source");

    for required in [
        "min_refresh_rate",
        "peak_refresh_rate",
        "user_refresh_rate",
        "SIGINT",
        "SIGTERM",
        "restore_mode",
    ] {
        assert!(main.contains(required), "runner keeps {required}");
    }
    for required in [
        "hayate_performance_workload",
        "perfetto",
        "gfxinfo",
        "meminfo",
        "thermalservice",
        "getprop",
        "manual_blockers",
    ] {
        assert!(collector.contains(required), "collector keeps {required}");
    }
}

#[test]
fn production_collector_executes_the_host_120_hz_fixture() {
    let script = workspace_root().join("scripts/collect-performance-matrix-case.py");
    let case = r#"{"target":"host_synthetic","refresh_rate_hz":120,"run_index":1,"workload":{"kind":"wake_pressure","wakes_per_second":240}}"#;
    let settings = r#"{"warmup_frames":1,"cooldown_millis":0,"thermal_guard_maximum":"elevated","thermal_guard_timeout_millis":1,"perfetto_duration_millis":1}"#;

    let output = Command::new(script)
        .arg(case)
        .arg(settings)
        .output()
        .expect("run host collector");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let evidence: RunEvidence =
        serde_json::from_slice(&output.stdout).expect("typed host evidence");
    assert_eq!(evidence.frame_total_ns.len(), 240);
    assert!(evidence.phase_ns.contains_key("host_synthetic"));
}
