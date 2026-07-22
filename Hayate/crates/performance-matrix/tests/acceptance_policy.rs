use std::collections::BTreeMap;

use hayate_performance_matrix::{
    evaluate, BuildSettings, ComparisonEvidence, CorrectnessEvidence, EvidenceSet, GateVerdict,
    HealthGrade, MatrixTarget, RunEvidence, ScalingClass, StructuralEvidence, Workload,
};

const MB: u64 = 1024 * 1024;

fn run(phase_ns: u64, cpu_bytes: u64, startup_ns: u64) -> RunEvidence {
    RunEvidence {
        frame_total_ns: vec![5_000_000; 120],
        reported_frames_over_two_intervals: None,
        phase_ns: BTreeMap::from([("renderer_present".into(), vec![phase_ns; 120])]),
        cpu_resident_bytes: cpu_bytes,
        gpu_resident_bytes: 40 * MB,
        startup_ns,
        idle_activity_count: 0,
        thermal_status: HealthGrade::Nominal,
        memory_pressure_status: HealthGrade::Nominal,
        scaling_class: ScalingClass::Linear,
        correctness: CorrectnessEvidence::passed(),
        raw_artifacts: Vec::new(),
    }
}

fn set(commit: &str, run: RunEvidence) -> EvidenceSet {
    EvidenceSet {
        target: MatrixTarget::AndroidDevice,
        device_id: "device-serial".into(),
        device_model: "Hayate Phone".into(),
        build_id: format!("benchmark-{commit}"),
        commit: commit.into(),
        build_settings: BuildSettings::profileable_release("assets-v1", "fonts-v1", "rgba8"),
        renderer_selection_reason: "vello:selected".into(),
        failure_category: None,
        refresh_rate_hz: 60,
        workload: Workload::Representative,
        battery_power_state: "unplugged:80%".into(),
        warm_condition: "steady_after_120_frames".into(),
        runs: vec![run; 5],
    }
}

fn comparison(baseline: RunEvidence, candidate: RunEvidence) -> ComparisonEvidence {
    ComparisonEvidence {
        baseline: set("baseline", baseline),
        candidate: set("candidate", candidate),
        structural: StructuralEvidence::Passed,
    }
}

#[test]
fn neutral_and_minor_deltas_pass_but_dual_threshold_regressions_fail() {
    let minor = evaluate(comparison(
        run(1_000_000, 80 * MB, 100_000_000),
        run(1_200_000, 88 * MB, 120_000_000),
    ))
    .expect("matched evidence");
    assert_eq!(minor.performance, GateVerdict::Pass);
    assert!(minor.findings.iter().any(|finding| finding.is_minor()));

    let material = evaluate(comparison(
        run(1_000_000, 80 * MB, 100_000_000),
        run(1_260_001, 90 * MB + 1, 126_000_001),
    ))
    .expect("matched evidence");
    assert_eq!(material.performance, GateVerdict::Fail);
    assert!(material
        .findings
        .iter()
        .any(|finding| finding.metric == "renderer_present_p95_ns" && finding.is_material()));
    assert!(material
        .findings
        .iter()
        .any(|finding| finding.metric == "cpu_resident_bytes" && finding.is_material()));
    assert!(material
        .findings
        .iter()
        .any(|finding| finding.metric == "startup_p95_ns" && finding.is_material()));
}

#[test]
fn correctness_long_frames_idle_health_and_scaling_are_blocking_rules() {
    let mut baseline = run(1_000_000, 80 * MB, 100_000_000);
    baseline.frame_total_ns[0] = 40_000_000;
    let mut candidate = baseline.clone();
    candidate.frame_total_ns[1] = 40_000_000;
    candidate.correctness.pixel_parity = false;
    candidate.idle_activity_count = 1;
    candidate.thermal_status = HealthGrade::Elevated;
    candidate.memory_pressure_status = HealthGrade::Elevated;
    candidate.scaling_class = ScalingClass::Superlinear;

    let report = evaluate(comparison(baseline, candidate)).expect("matched evidence");

    assert_eq!(report.performance, GateVerdict::Fail);
    for metric in [
        "correctness",
        "frames_over_two_intervals",
        "idle_activity_count",
        "thermal_status",
        "memory_pressure_status",
        "scaling_class",
    ] {
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.metric == metric && finding.is_material()),
            "missing material finding for {metric}"
        );
    }
}

#[test]
fn structural_acceptance_is_reported_separately_from_performance() {
    let mut evidence = comparison(
        run(1_000_000, 80 * MB, 100_000_000),
        run(1_000_000, 80 * MB, 100_000_000),
    );
    evidence.structural = StructuralEvidence::Failed {
        reason: "contract validation failed".into(),
    };

    let report = evaluate(evidence).expect("matched evidence");

    assert_eq!(report.performance, GateVerdict::Pass);
    assert_eq!(report.structural, GateVerdict::Fail);
    assert_eq!(report.overall, GateVerdict::Fail);
}

#[test]
fn mismatched_device_build_settings_or_run_count_is_rejected_as_invalid_evidence() {
    let mut evidence = comparison(
        run(1_000_000, 80 * MB, 100_000_000),
        run(1_000_000, 80 * MB, 100_000_000),
    );
    evidence.candidate.device_id = "another-device".into();
    evidence.candidate.runs.truncate(4);

    let errors = evaluate(evidence).expect_err("unmatched evidence must not be judged");

    assert!(errors.iter().any(|error| error.contains("same device")));
    assert!(errors.iter().any(|error| error.contains("at least 5 runs")));
}
