use hayate_performance_matrix::{
    MatrixTarget, PerformanceMatrix, Workload, ANDROID_REFRESH_RATES_HZ, HOST_STRESS_HZ,
    MIN_RUNS_PER_DEVICE_CASE,
};

#[test]
fn standard_matrix_runs_representative_and_parameterized_stress_at_60_and_90_hz() {
    let matrix = PerformanceMatrix::standard();

    for refresh_rate_hz in ANDROID_REFRESH_RATES_HZ {
        for workload in [
            Workload::Representative,
            Workload::ResourcePressure {
                resource_count: 4_096,
            },
        ] {
            let cases = matrix
                .cases()
                .iter()
                .filter(|case| {
                    case.target == MatrixTarget::AndroidDevice
                        && case.refresh_rate_hz == refresh_rate_hz
                        && case.workload == workload
                })
                .collect::<Vec<_>>();
            assert_eq!(cases.len(), MIN_RUNS_PER_DEVICE_CASE);
            assert_eq!(
                cases.iter().map(|case| case.run_index).collect::<Vec<_>>(),
                vec![1, 2, 3, 4, 5]
            );
        }
    }
}

#[test]
fn standard_matrix_keeps_the_host_120_hz_stress_gate_in_the_same_runner() {
    let matrix = PerformanceMatrix::standard();

    assert!(matrix.cases().iter().any(|case| {
        case.target == MatrixTarget::HostSynthetic
            && case.refresh_rate_hz == HOST_STRESS_HZ
            && matches!(case.workload, Workload::ResourcePressure { .. })
    }));
}
