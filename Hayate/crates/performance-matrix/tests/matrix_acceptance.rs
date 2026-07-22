use hayate_performance_matrix::{
    evaluate_matrix, BuildSettings, CollectionEnvironment, GateVerdict, MatrixAcceptanceEvidence,
    MatrixTarget, PerformanceMatrix, StructuralEvidence,
};

fn bundle(commit: &str) -> hayate_performance_matrix::MatrixEvidenceBundle {
    let outputs = PerformanceMatrix::standard()
        .cases()
        .iter()
        .copied()
        .map(|case| hayate_performance_matrix::CaseOutput {
            case,
            evidence: hayate_performance_matrix::RunEvidence::synthetic_pass(),
        })
        .collect();
    hayate_performance_matrix::MatrixEvidenceBundle::from_outputs(
        CollectionEnvironment {
            device_id: "device".into(),
            device_model: "model".into(),
            build_id: format!("build-{commit}"),
            commit: commit.into(),
            build_settings: BuildSettings::profileable_release("assets", "fonts", "surface"),
            renderer_selection_reason: "vello:selected".into(),
            failure_category: None,
            battery_power_state: "unplugged".into(),
            warm_condition: "warm".into(),
        },
        outputs,
    )
}

#[test]
fn complete_standard_matrix_is_matched_and_judged_as_one_workstream() {
    let report = evaluate_matrix(MatrixAcceptanceEvidence {
        baseline: bundle("base"),
        candidate: bundle("candidate"),
        structural: StructuralEvidence::Passed,
    })
    .expect("complete matched matrix");

    assert_eq!(report.performance, GateVerdict::Pass);
    assert_eq!(report.structural, GateVerdict::Pass);
    assert_eq!(report.overall, GateVerdict::Pass);
    assert!(report.cases.iter().any(|case| {
        case.evidence.candidate.target == MatrixTarget::HostSynthetic
            && case.evidence.candidate.refresh_rate_hz == 120
    }));
    let json = report
        .to_json_pretty()
        .expect("machine-readable matrix report");
    assert!(json.contains("\"refresh_rate_hz\": 90"));
    let html = report.to_html().expect("reviewer matrix report");
    assert!(html.contains("60Hz") && html.contains("90Hz") && html.contains("120Hz"));
}

#[test]
fn missing_standard_case_is_invalid_instead_of_silently_passing() {
    let baseline = bundle("base");
    let mut candidate = bundle("candidate");
    candidate.sets.pop();

    let errors = evaluate_matrix(MatrixAcceptanceEvidence {
        baseline,
        candidate,
        structural: StructuralEvidence::Passed,
    })
    .expect_err("missing case invalidates the matrix");

    assert!(errors
        .iter()
        .any(|error| error.contains("missing matrix case")));
}
