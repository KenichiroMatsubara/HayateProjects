use std::collections::BTreeMap;

use hayate_performance_matrix::{
    evaluate, BuildSettings, ComparisonEvidence, CorrectnessEvidence, EvidenceSet, GateVerdict,
    HealthGrade, MatrixTarget, RunEvidence, ScalingClass, StructuralEvidence, Workload,
};

fn evidence(commit: &str) -> EvidenceSet {
    let run = RunEvidence {
        frame_total_ns: vec![1_000_000; 120],
        reported_frames_over_two_intervals: None,
        phase_ns: BTreeMap::from([("app_host".into(), vec![100_000; 120])]),
        cpu_resident_bytes: 1,
        gpu_resident_bytes: 2,
        startup_ns: 3,
        idle_activity_count: 0,
        thermal_status: HealthGrade::Nominal,
        memory_pressure_status: HealthGrade::Nominal,
        scaling_class: ScalingClass::Linear,
        correctness: CorrectnessEvidence::passed(),
        raw_artifacts: vec!["run-1/hayate.perfetto-trace".into()],
    };
    EvidenceSet {
        target: MatrixTarget::AndroidDevice,
        device_id: "serial<&>".into(),
        device_model: "Hayate Device".into(),
        build_id: format!("build-{commit}"),
        commit: commit.into(),
        build_settings: BuildSettings::profileable_release("assets", "fonts", "rgba8"),
        renderer_selection_reason: "vello:selected".into(),
        failure_category: Some("none".into()),
        refresh_rate_hz: 90,
        workload: Workload::Representative,
        battery_power_state: "unplugged".into(),
        warm_condition: "warm".into(),
        runs: vec![run; 5],
    }
}

#[test]
fn report_emits_machine_readable_raw_evidence_and_reviewer_html() {
    let report = evaluate(ComparisonEvidence {
        baseline: evidence("base"),
        candidate: evidence("candidate"),
        structural: StructuralEvidence::Passed,
    })
    .expect("valid evidence");

    let json = report.to_json_pretty().expect("json report");
    let decoded: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
    assert_eq!(decoded["performance"], "pass");
    assert_eq!(decoded["evidence"]["candidate"]["commit"], "candidate");
    assert_eq!(
        decoded["evidence"]["candidate"]["runs"]
            .as_array()
            .unwrap()
            .len(),
        5
    );

    let html = report.to_html().expect("HTML report");
    assert!(html.contains("Performance: pass"));
    assert!(html.contains("Structural: pass"));
    assert!(html.contains("Renderer selection reason"));
    assert!(html.contains("Raw evidence"));
    assert!(html.contains("serial&lt;&amp;&gt;"));
    assert!(!html.contains("serial<&>"));
    assert_eq!(report.overall, GateVerdict::Pass);
}
