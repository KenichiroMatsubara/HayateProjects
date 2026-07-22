use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{CaseOutput, MatrixTarget, PerformanceMatrix, Workload, MIN_RUNS_PER_DEVICE_CASE};

pub const MATERIAL_PERCENT_INCREASE: u64 = 10;
pub const MATERIAL_PHASE_INCREASE_NS: u64 = 250_000;
pub const MATERIAL_MEMORY_INCREASE_BYTES: u64 = 8 * 1024 * 1024;
pub const MATERIAL_STARTUP_INCREASE_NS: u64 = 25_000_000;
pub const LONG_FRAME_INTERVALS: u64 = 2;
const PERCENT_DENOMINATOR: u128 = 100;
const PERCENT_X100_DENOMINATOR: u128 = 10_000;
const P95_NUMERATOR: usize = 95;
const P95_DENOMINATOR: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildSettings {
    pub optimization: String,
    pub profileable: bool,
    pub debug_assertions: bool,
    pub scene_validation: bool,
    pub assets: String,
    pub fonts: String,
    pub surface: String,
}

impl BuildSettings {
    pub fn profileable_release(assets: &str, fonts: &str, surface: &str) -> Self {
        Self {
            optimization: "release".into(),
            profileable: true,
            debug_assertions: false,
            scene_validation: false,
            assets: assets.into(),
            fonts: fonts.into(),
            surface: surface.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthGrade {
    Nominal,
    Elevated,
    Severe,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScalingClass {
    Constant,
    Logarithmic,
    Linear,
    Superlinear,
    Quadratic,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorrectnessEvidence {
    pub correctness: bool,
    pub pixel_parity: bool,
    pub input_result: bool,
    /// Explicitly unresolved checks are blockers rather than silently assumed passes.
    pub manual_blockers: Vec<String>,
}

impl CorrectnessEvidence {
    pub const fn passed() -> Self {
        Self {
            correctness: true,
            pixel_parity: true,
            input_result: true,
            manual_blockers: Vec::new(),
        }
    }

    fn all_passed(&self) -> bool {
        self.correctness
            && self.pixel_parity
            && self.input_result
            && self.manual_blockers.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunEvidence {
    /// Raw per-frame totals. The report retains them; acceptance derives counts and percentiles.
    pub frame_total_ns: Vec<u64>,
    /// Device summary count when raw frame totals live in the linked Perfetto/gfx artifacts.
    /// Host fixtures may leave this unset and let the evaluator derive it from `frame_total_ns`.
    #[serde(default)]
    pub reported_frames_over_two_intervals: Option<u64>,
    /// Raw per-frame phase samples keyed by the stable observability phase name.
    pub phase_ns: BTreeMap<String, Vec<u64>>,
    pub cpu_resident_bytes: u64,
    pub gpu_resident_bytes: u64,
    pub startup_ns: u64,
    pub idle_activity_count: u64,
    pub thermal_status: HealthGrade,
    pub memory_pressure_status: HealthGrade,
    pub scaling_class: ScalingClass,
    pub correctness: CorrectnessEvidence,
    /// Paths or stable artifact identifiers for logcat, Perfetto, gfxinfo, and environment dumps.
    #[serde(default)]
    pub raw_artifacts: Vec<String>,
}

impl RunEvidence {
    /// Deterministic no-regression sample used by host fixtures and executor contract tests.
    pub fn synthetic_pass() -> Self {
        Self {
            frame_total_ns: vec![1_000_000],
            reported_frames_over_two_intervals: None,
            phase_ns: BTreeMap::new(),
            cpu_resident_bytes: 0,
            gpu_resident_bytes: 0,
            startup_ns: 0,
            idle_activity_count: 0,
            thermal_status: HealthGrade::Nominal,
            memory_pressure_status: HealthGrade::Nominal,
            scaling_class: ScalingClass::Constant,
            correctness: CorrectnessEvidence::passed(),
            raw_artifacts: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSet {
    pub target: MatrixTarget,
    pub device_id: String,
    pub device_model: String,
    pub build_id: String,
    pub commit: String,
    pub build_settings: BuildSettings,
    pub renderer_selection_reason: String,
    pub failure_category: Option<String>,
    pub refresh_rate_hz: u32,
    pub workload: Workload,
    pub battery_power_state: String,
    pub warm_condition: String,
    pub runs: Vec<RunEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectionEnvironment {
    pub device_id: String,
    pub device_model: String,
    pub build_id: String,
    pub commit: String,
    pub build_settings: BuildSettings,
    pub renderer_selection_reason: String,
    pub failure_category: Option<String>,
    pub battery_power_state: String,
    pub warm_condition: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatrixEvidenceBundle {
    pub sets: Vec<EvidenceSet>,
}

impl MatrixEvidenceBundle {
    pub fn from_outputs(environment: CollectionEnvironment, outputs: Vec<CaseOutput>) -> Self {
        let mut grouped =
            BTreeMap::<(MatrixTarget, u32, Workload), Vec<(usize, RunEvidence)>>::new();
        for output in outputs {
            grouped
                .entry((
                    output.case.target,
                    output.case.refresh_rate_hz,
                    output.case.workload,
                ))
                .or_default()
                .push((output.case.run_index, output.evidence));
        }
        let sets = grouped
            .into_iter()
            .map(|((target, refresh_rate_hz, workload), mut runs)| {
                runs.sort_by_key(|(run_index, _)| *run_index);
                EvidenceSet {
                    target,
                    device_id: environment.device_id.clone(),
                    device_model: environment.device_model.clone(),
                    build_id: environment.build_id.clone(),
                    commit: environment.commit.clone(),
                    build_settings: environment.build_settings.clone(),
                    renderer_selection_reason: environment.renderer_selection_reason.clone(),
                    failure_category: environment.failure_category.clone(),
                    refresh_rate_hz,
                    workload,
                    battery_power_state: environment.battery_power_state.clone(),
                    warm_condition: environment.warm_condition.clone(),
                    runs: runs.into_iter().map(|(_, run)| run).collect(),
                }
            })
            .collect();
        Self { sets }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum StructuralEvidence {
    Passed,
    Failed { reason: String },
    NotEvaluated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparisonEvidence {
    pub baseline: EvidenceSet,
    pub candidate: EvidenceSet,
    pub structural: StructuralEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateVerdict {
    Pass,
    Fail,
    NotEvaluated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingSeverity {
    Minor,
    Material,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub metric: String,
    pub severity: FindingSeverity,
    pub baseline: u64,
    pub candidate: u64,
    pub increase: u64,
    /// Hundredths of one percent; omitted when a zero baseline makes a ratio undefined.
    pub increase_percent_x100: Option<u64>,
    pub rule: String,
}

impl Finding {
    pub fn is_minor(&self) -> bool {
        self.severity == FindingSeverity::Minor
    }

    pub fn is_material(&self) -> bool {
        self.severity == FindingSeverity::Material
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptanceReport {
    /// Raw evidence and environment are embedded so the machine report is independently auditable.
    pub evidence: ComparisonEvidence,
    pub findings: Vec<Finding>,
    pub performance: GateVerdict,
    pub structural: GateVerdict,
    pub overall: GateVerdict,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatrixAcceptanceEvidence {
    pub baseline: MatrixEvidenceBundle,
    pub candidate: MatrixEvidenceBundle,
    pub structural: StructuralEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatrixAcceptanceReport {
    pub cases: Vec<AcceptanceReport>,
    pub performance: GateVerdict,
    pub structural: GateVerdict,
    pub overall: GateVerdict,
}

pub fn evaluate_matrix(
    evidence: MatrixAcceptanceEvidence,
) -> Result<MatrixAcceptanceReport, Vec<String>> {
    type Key = (MatrixTarget, u32, Workload);
    let baseline = evidence
        .baseline
        .sets
        .into_iter()
        .map(|set| ((set.target, set.refresh_rate_hz, set.workload), set))
        .collect::<BTreeMap<Key, _>>();
    let candidate = evidence
        .candidate
        .sets
        .into_iter()
        .map(|set| ((set.target, set.refresh_rate_hz, set.workload), set))
        .collect::<BTreeMap<Key, _>>();
    let expected = PerformanceMatrix::standard()
        .cases()
        .iter()
        .map(|case| (case.target, case.refresh_rate_hz, case.workload))
        .collect::<BTreeSet<Key>>();
    let mut errors = Vec::new();
    for key in &expected {
        if !baseline.contains_key(key) {
            errors.push(format!("baseline missing matrix case {key:?}"));
        }
        if !candidate.contains_key(key) {
            errors.push(format!("candidate missing matrix case {key:?}"));
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }

    let mut cases = Vec::with_capacity(expected.len());
    for key in expected {
        let comparison = ComparisonEvidence {
            baseline: baseline[&key].clone(),
            candidate: candidate[&key].clone(),
            structural: evidence.structural.clone(),
        };
        match evaluate(comparison) {
            Ok(report) => cases.push(report),
            Err(case_errors) => errors.extend(
                case_errors
                    .into_iter()
                    .map(|error| format!("{key:?}: {error}")),
            ),
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let performance = if cases
        .iter()
        .any(|report| report.performance == GateVerdict::Fail)
    {
        GateVerdict::Fail
    } else {
        GateVerdict::Pass
    };
    let structural = match evidence.structural {
        StructuralEvidence::Passed => GateVerdict::Pass,
        StructuralEvidence::Failed { .. } => GateVerdict::Fail,
        StructuralEvidence::NotEvaluated => GateVerdict::NotEvaluated,
    };
    let overall = match (performance, structural) {
        (GateVerdict::Fail, _) | (_, GateVerdict::Fail) => GateVerdict::Fail,
        (GateVerdict::Pass, GateVerdict::Pass) => GateVerdict::Pass,
        _ => GateVerdict::NotEvaluated,
    };
    Ok(MatrixAcceptanceReport {
        cases,
        performance,
        structural,
        overall,
    })
}

pub fn evaluate(evidence: ComparisonEvidence) -> Result<AcceptanceReport, Vec<String>> {
    let errors = validate(&evidence);
    if !errors.is_empty() {
        return Err(errors);
    }

    let mut findings = Vec::new();
    evaluate_correctness(&evidence.candidate, &mut findings);
    evaluate_long_frames(&evidence, &mut findings);
    evaluate_phases(&evidence, &mut findings);
    evaluate_metric(
        "cpu_resident_bytes",
        median(
            evidence
                .baseline
                .runs
                .iter()
                .map(|run| run.cpu_resident_bytes),
        ),
        median(
            evidence
                .candidate
                .runs
                .iter()
                .map(|run| run.cpu_resident_bytes),
        ),
        MATERIAL_MEMORY_INCREASE_BYTES,
        "steady memory >10% and >8MiB",
        &mut findings,
    );
    evaluate_metric(
        "gpu_resident_bytes",
        median(
            evidence
                .baseline
                .runs
                .iter()
                .map(|run| run.gpu_resident_bytes),
        ),
        median(
            evidence
                .candidate
                .runs
                .iter()
                .map(|run| run.gpu_resident_bytes),
        ),
        MATERIAL_MEMORY_INCREASE_BYTES,
        "steady memory >10% and >8MiB",
        &mut findings,
    );
    evaluate_metric(
        "startup_p95_ns",
        percentile95(evidence.baseline.runs.iter().map(|run| run.startup_ns)),
        percentile95(evidence.candidate.runs.iter().map(|run| run.startup_ns)),
        MATERIAL_STARTUP_INCREASE_NS,
        "startup p95 >10% and >25ms",
        &mut findings,
    );
    evaluate_ordered_regressions(&evidence, &mut findings);

    let performance = if findings.iter().any(Finding::is_material) {
        GateVerdict::Fail
    } else {
        GateVerdict::Pass
    };
    let structural = match evidence.structural {
        StructuralEvidence::Passed => GateVerdict::Pass,
        StructuralEvidence::Failed { .. } => GateVerdict::Fail,
        StructuralEvidence::NotEvaluated => GateVerdict::NotEvaluated,
    };
    let overall = match (performance, structural) {
        (GateVerdict::Fail, _) | (_, GateVerdict::Fail) => GateVerdict::Fail,
        (GateVerdict::Pass, GateVerdict::Pass) => GateVerdict::Pass,
        _ => GateVerdict::NotEvaluated,
    };
    Ok(AcceptanceReport {
        evidence,
        findings,
        performance,
        structural,
        overall,
    })
}

fn validate(evidence: &ComparisonEvidence) -> Vec<String> {
    let baseline = &evidence.baseline;
    let candidate = &evidence.candidate;
    let mut errors = Vec::new();
    if baseline.target != candidate.target {
        errors.push("baseline and candidate must use the same target".into());
    }
    if baseline.device_id != candidate.device_id || baseline.device_model != candidate.device_model
    {
        errors.push("baseline and candidate must use the same device".into());
    }
    if baseline.build_settings != candidate.build_settings {
        errors.push("baseline and candidate must use the same build settings".into());
    }
    if baseline.workload != candidate.workload {
        errors.push("baseline and candidate must use the same workload".into());
    }
    if baseline.refresh_rate_hz != candidate.refresh_rate_hz {
        errors.push("baseline and candidate must use the same refresh rate".into());
    }
    if baseline.refresh_rate_hz == 0 {
        errors.push("refresh rate must be non-zero".into());
    }
    let required_runs = if baseline.target == MatrixTarget::AndroidDevice {
        MIN_RUNS_PER_DEVICE_CASE
    } else {
        1
    };
    if baseline.runs.len() < required_runs || candidate.runs.len() < required_runs {
        errors.push(format!(
            "baseline and candidate require at least {required_runs} runs"
        ));
    }
    for (side, set) in [("baseline", baseline), ("candidate", candidate)] {
        if set.build_id.is_empty() || set.commit.is_empty() {
            errors.push(format!("{side} build id and commit are required"));
        }
        if set.renderer_selection_reason.is_empty() {
            errors.push(format!("{side} renderer selection reason is required"));
        }
        if set.runs.iter().any(|run| run.frame_total_ns.is_empty()) {
            errors.push(format!("{side} runs require raw frame samples"));
        }
    }
    errors
}

fn evaluate_correctness(candidate: &EvidenceSet, findings: &mut Vec<Finding>) {
    let failures = candidate
        .runs
        .iter()
        .filter(|run| !run.correctness.all_passed())
        .count() as u64;
    if failures > 0 {
        findings.push(categorical_finding(
            "correctness",
            0,
            failures,
            "correctness, pixel parity, and input results are blockers",
        ));
    }
}

fn evaluate_long_frames(evidence: &ComparisonEvidence, findings: &mut Vec<Finding>) {
    let interval_ns = 1_000_000_000_u64.div_ceil(u64::from(evidence.baseline.refresh_rate_hz));
    let threshold = interval_ns.saturating_mul(LONG_FRAME_INTERVALS);
    let baseline = median(evidence.baseline.runs.iter().map(|run| {
        run.reported_frames_over_two_intervals.unwrap_or_else(|| {
            run.frame_total_ns
                .iter()
                .filter(|&&sample| sample > threshold)
                .count() as u64
        })
    }));
    let candidate = median(evidence.candidate.runs.iter().map(|run| {
        run.reported_frames_over_two_intervals.unwrap_or_else(|| {
            run.frame_total_ns
                .iter()
                .filter(|&&sample| sample > threshold)
                .count() as u64
        })
    }));
    if candidate > baseline {
        findings.push(categorical_finding(
            "frames_over_two_intervals",
            baseline,
            candidate,
            "median count increased across matched runs",
        ));
    }
}

fn evaluate_phases(evidence: &ComparisonEvidence, findings: &mut Vec<Finding>) {
    let names = evidence
        .baseline
        .runs
        .iter()
        .chain(&evidence.candidate.runs)
        .flat_map(|run| run.phase_ns.keys().cloned())
        .collect::<BTreeSet<_>>();
    for name in names {
        let baseline = median(evidence.baseline.runs.iter().map(|run| {
            percentile95(
                run.phase_ns
                    .get(&name)
                    .into_iter()
                    .flat_map(|samples| samples.iter().copied()),
            )
        }));
        let candidate = median(evidence.candidate.runs.iter().map(|run| {
            percentile95(
                run.phase_ns
                    .get(&name)
                    .into_iter()
                    .flat_map(|samples| samples.iter().copied()),
            )
        }));
        evaluate_metric(
            &format!("{name}_p95_ns"),
            baseline,
            candidate,
            MATERIAL_PHASE_INCREASE_NS,
            "phase p95 >10% and >0.25ms",
            findings,
        );
    }
}

fn evaluate_ordered_regressions(evidence: &ComparisonEvidence, findings: &mut Vec<Finding>) {
    let baseline_idle = median(
        evidence
            .baseline
            .runs
            .iter()
            .map(|run| run.idle_activity_count),
    );
    let candidate_idle = median(
        evidence
            .candidate
            .runs
            .iter()
            .map(|run| run.idle_activity_count),
    );
    if candidate_idle > baseline_idle && candidate_idle > 0 {
        findings.push(categorical_finding(
            "idle_activity_count",
            baseline_idle,
            candidate_idle,
            "idle CPU/GPU/timer/vsync activity revived",
        ));
    }

    let baseline_thermal = worst(evidence.baseline.runs.iter().map(|run| run.thermal_status));
    let candidate_thermal = worst(evidence.candidate.runs.iter().map(|run| run.thermal_status));
    if candidate_thermal > baseline_thermal {
        findings.push(categorical_finding(
            "thermal_status",
            baseline_thermal as u64,
            candidate_thermal as u64,
            "thermal status worsened by at least one grade",
        ));
    }

    let baseline_memory = worst(
        evidence
            .baseline
            .runs
            .iter()
            .map(|run| run.memory_pressure_status),
    );
    let candidate_memory = worst(
        evidence
            .candidate
            .runs
            .iter()
            .map(|run| run.memory_pressure_status),
    );
    if candidate_memory > baseline_memory {
        findings.push(categorical_finding(
            "memory_pressure_status",
            baseline_memory as u64,
            candidate_memory as u64,
            "OS memory-pressure behavior worsened by at least one grade",
        ));
    }

    let baseline_scaling = worst(evidence.baseline.runs.iter().map(|run| run.scaling_class));
    let candidate_scaling = worst(evidence.candidate.runs.iter().map(|run| run.scaling_class));
    if candidate_scaling > baseline_scaling {
        findings.push(categorical_finding(
            "scaling_class",
            baseline_scaling as u64,
            candidate_scaling as u64,
            "stress scaling class worsened",
        ));
    }
}

fn evaluate_metric(
    metric: &str,
    baseline: u64,
    candidate: u64,
    absolute_threshold: u64,
    rule: &str,
    findings: &mut Vec<Finding>,
) {
    if candidate <= baseline {
        return;
    }
    let increase = candidate - baseline;
    let ratio_exceeded = baseline == 0
        || u128::from(increase) * PERCENT_DENOMINATOR
            > u128::from(baseline) * u128::from(MATERIAL_PERCENT_INCREASE);
    let severity = if ratio_exceeded && increase > absolute_threshold {
        FindingSeverity::Material
    } else {
        FindingSeverity::Minor
    };
    findings.push(Finding {
        metric: metric.into(),
        severity,
        baseline,
        candidate,
        increase,
        increase_percent_x100: (baseline != 0).then(|| {
            (u128::from(increase) * PERCENT_X100_DENOMINATOR / u128::from(baseline))
                .min(u128::from(u64::MAX)) as u64
        }),
        rule: rule.into(),
    });
}

fn categorical_finding(metric: &str, baseline: u64, candidate: u64, rule: &str) -> Finding {
    Finding {
        metric: metric.into(),
        severity: FindingSeverity::Material,
        baseline,
        candidate,
        increase: candidate.saturating_sub(baseline),
        increase_percent_x100: None,
        rule: rule.into(),
    }
}

fn median(values: impl IntoIterator<Item = u64>) -> u64 {
    let mut values = values.into_iter().collect::<Vec<_>>();
    values.sort_unstable();
    values.get(values.len() / 2).copied().unwrap_or(0)
}

fn percentile95(values: impl IntoIterator<Item = u64>) -> u64 {
    let mut values = values.into_iter().collect::<Vec<_>>();
    values.sort_unstable();
    if values.is_empty() {
        return 0;
    }
    let rank = (values.len() * P95_NUMERATOR)
        .div_ceil(P95_DENOMINATOR)
        .saturating_sub(1);
    values[rank]
}

fn worst<T>(values: impl IntoIterator<Item = T>) -> T
where
    T: Ord + Default,
{
    values.into_iter().max().unwrap_or_default()
}

impl Default for HealthGrade {
    fn default() -> Self {
        Self::Nominal
    }
}

impl Default for ScalingClass {
    fn default() -> Self {
        Self::Constant
    }
}
