//! Deterministic performance-matrix planning, evidence, and acceptance policy.

use serde::{Deserialize, Serialize};

mod acceptance;
pub use acceptance::{
    evaluate, evaluate_matrix, AcceptanceReport, BuildSettings, CollectionEnvironment,
    ComparisonEvidence, CorrectnessEvidence, EvidenceSet, Finding, FindingSeverity, GateVerdict,
    HealthGrade, MatrixAcceptanceEvidence, MatrixAcceptanceReport, MatrixEvidenceBundle,
    RunEvidence, ScalingClass, StructuralEvidence, LONG_FRAME_INTERVALS,
    MATERIAL_MEMORY_INCREASE_BYTES, MATERIAL_PERCENT_INCREASE, MATERIAL_PHASE_INCREASE_NS,
    MATERIAL_STARTUP_INCREASE_NS,
};
mod report;
mod runner;
pub use runner::{
    CaseOutput, MatrixCaseExecutor, MatrixRunner, MatrixRunnerError, RunnerSettings,
    DEFAULT_COOLDOWN_MILLIS, DEFAULT_PERFETTO_DURATION_MILLIS, DEFAULT_THERMAL_GUARD_MAXIMUM,
    DEFAULT_THERMAL_GUARD_TIMEOUT_MILLIS, DEFAULT_WARMUP_FRAMES,
};

pub const ANDROID_REFRESH_RATES_HZ: [u32; 2] = [60, 90];
pub const HOST_STRESS_HZ: u32 = 120;
pub const MIN_RUNS_PER_DEVICE_CASE: usize = 5;

pub const STRESS_LAYER_COUNT: u32 = 512;
pub const STRESS_DIRTY_PERCENT: u8 = 50;
pub const STRESS_SCENE_NODES: u32 = 16_384;
pub const STRESS_SCENE_DEPTH: u16 = 64;
pub const STRESS_SIBLINGS: u16 = 256;
pub const STRESS_Z_INDEX_CHANGE_PERCENT: u8 = 25;
pub const STRESS_RESOURCE_COUNT: u32 = 4_096;
pub const STRESS_WAKE_FREQUENCY_HZ: u32 = 240;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatrixTarget {
    AndroidDevice,
    HostSynthetic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Workload {
    Representative,
    LayerPressure {
        layer_count: u32,
        dirty_percent: u8,
    },
    SceneGraphPressure {
        nodes: u32,
        depth: u16,
        siblings: u16,
        z_index_change_percent: u8,
    },
    ResourcePressure {
        resource_count: u32,
    },
    WakePressure {
        wakes_per_second: u32,
    },
}

impl Workload {
    pub const fn named_stress() -> [Self; 4] {
        [
            Self::LayerPressure {
                layer_count: STRESS_LAYER_COUNT,
                dirty_percent: STRESS_DIRTY_PERCENT,
            },
            Self::SceneGraphPressure {
                nodes: STRESS_SCENE_NODES,
                depth: STRESS_SCENE_DEPTH,
                siblings: STRESS_SIBLINGS,
                z_index_change_percent: STRESS_Z_INDEX_CHANGE_PERCENT,
            },
            Self::ResourcePressure {
                resource_count: STRESS_RESOURCE_COUNT,
            },
            Self::WakePressure {
                wakes_per_second: STRESS_WAKE_FREQUENCY_HZ,
            },
        ]
    }

    pub const fn is_stress(self) -> bool {
        !matches!(self, Self::Representative)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatrixCase {
    pub target: MatrixTarget,
    pub refresh_rate_hz: u32,
    /// One-based for human reports and artifact directory names.
    pub run_index: usize,
    pub workload: Workload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PerformanceMatrix {
    cases: Vec<MatrixCase>,
}

impl PerformanceMatrix {
    pub fn standard() -> Self {
        let mut cases = Vec::new();
        let workloads = std::iter::once(Workload::Representative).chain(Workload::named_stress());
        for refresh_rate_hz in ANDROID_REFRESH_RATES_HZ {
            for workload in workloads.clone() {
                for run_index in 1..=MIN_RUNS_PER_DEVICE_CASE {
                    cases.push(MatrixCase {
                        target: MatrixTarget::AndroidDevice,
                        refresh_rate_hz,
                        run_index,
                        workload,
                    });
                }
            }
        }
        for workload in Workload::named_stress() {
            cases.push(MatrixCase {
                target: MatrixTarget::HostSynthetic,
                refresh_rate_hz: HOST_STRESS_HZ,
                run_index: 1,
                workload,
            });
        }
        Self { cases }
    }

    pub fn cases(&self) -> &[MatrixCase] {
        &self.cases
    }
}

/// Platform adapter for temporarily pinning display refresh. The captured mode remains opaque so
/// Android can preserve adaptive/min/peak settings exactly rather than reconstructing them.
pub trait RefreshRateController {
    type Mode;
    type Error;

    fn current_mode(&mut self) -> Result<Self::Mode, Self::Error>;
    fn set_fixed_rate(&mut self, refresh_rate_hz: u32) -> Result<(), Self::Error>;
    fn restore_mode(&mut self, mode: &Self::Mode) -> Result<(), Self::Error>;
}

#[derive(Debug, PartialEq, Eq)]
pub enum RefreshRunError<ControllerError, WorkError> {
    Capture(ControllerError),
    Apply(ControllerError),
    Work(WorkError),
    Restore(ControllerError),
    WorkAndRestore {
        work: WorkError,
        restore: ControllerError,
    },
}

/// Run one matrix segment at a fixed refresh rate and restore the exact previous mode on every
/// normal, error, and unwind exit. The production CLI turns SIGINT/TERM/HUP into a work error, so
/// interrupt restoration follows this same path.
pub fn with_fixed_refresh_rate<C, T, WorkError>(
    controller: &mut C,
    refresh_rate_hz: u32,
    work: impl FnOnce() -> Result<T, WorkError>,
) -> Result<T, RefreshRunError<C::Error, WorkError>>
where
    C: RefreshRateController,
{
    let original = controller
        .current_mode()
        .map_err(RefreshRunError::Capture)?;
    if let Err(error) = controller.set_fixed_rate(refresh_rate_hz) {
        let _ = controller.restore_mode(&original);
        return Err(RefreshRunError::Apply(error));
    }

    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(work));
    let restore = controller.restore_mode(&original);
    match outcome {
        Err(panic) => std::panic::resume_unwind(panic),
        Ok(Ok(value)) => match restore {
            Ok(()) => Ok(value),
            Err(error) => Err(RefreshRunError::Restore(error)),
        },
        Ok(Err(work)) => match restore {
            Ok(()) => Err(RefreshRunError::Work(work)),
            Err(restore) => Err(RefreshRunError::WorkAndRestore { work, restore }),
        },
    }
}
