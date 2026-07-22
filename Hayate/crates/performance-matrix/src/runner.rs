use serde::{Deserialize, Serialize};

use crate::{
    with_fixed_refresh_rate, HealthGrade, MatrixCase, MatrixTarget, PerformanceMatrix,
    RefreshRateController, RefreshRunError, RunEvidence, ANDROID_REFRESH_RATES_HZ,
};

pub const DEFAULT_WARMUP_FRAMES: u32 = 120;
pub const DEFAULT_COOLDOWN_MILLIS: u64 = 5_000;
pub const DEFAULT_THERMAL_GUARD_MAXIMUM: HealthGrade = HealthGrade::Elevated;
pub const DEFAULT_THERMAL_GUARD_TIMEOUT_MILLIS: u64 = 120_000;
pub const DEFAULT_PERFETTO_DURATION_MILLIS: u64 = 20_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunnerSettings {
    pub warmup_frames: u32,
    pub cooldown_millis: u64,
    pub thermal_guard_maximum: HealthGrade,
    pub thermal_guard_timeout_millis: u64,
    pub perfetto_duration_millis: u64,
}

impl Default for RunnerSettings {
    fn default() -> Self {
        Self {
            warmup_frames: DEFAULT_WARMUP_FRAMES,
            cooldown_millis: DEFAULT_COOLDOWN_MILLIS,
            thermal_guard_maximum: DEFAULT_THERMAL_GUARD_MAXIMUM,
            thermal_guard_timeout_millis: DEFAULT_THERMAL_GUARD_TIMEOUT_MILLIS,
            perfetto_duration_millis: DEFAULT_PERFETTO_DURATION_MILLIS,
        }
    }
}

pub trait MatrixCaseExecutor {
    type Error;

    fn execute(
        &mut self,
        case: MatrixCase,
        settings: RunnerSettings,
    ) -> Result<RunEvidence, Self::Error>;

    fn await_thermal_guard(
        &mut self,
        maximum: HealthGrade,
        timeout_millis: u64,
    ) -> Result<(), Self::Error>;

    fn cooldown(&mut self, millis: u64) -> Result<(), Self::Error>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseOutput {
    pub case: MatrixCase,
    pub evidence: RunEvidence,
}

#[derive(Debug, PartialEq, Eq)]
pub enum MatrixRunnerError<ControllerError, ExecutorError> {
    Android(RefreshRunError<ControllerError, ExecutorError>),
    Host(ExecutorError),
}

/// Owns the complete device + host schedule. The executor knows how to drive a workload; matrix
/// ordering, refresh restoration, run counts and guard settings do not leak into that adapter.
pub struct MatrixRunner {
    matrix: PerformanceMatrix,
    settings: RunnerSettings,
}

impl MatrixRunner {
    pub fn new(matrix: PerformanceMatrix, settings: RunnerSettings) -> Self {
        Self { matrix, settings }
    }

    pub fn run<C, E>(
        &self,
        refresh: &mut C,
        executor: &mut E,
    ) -> Result<Vec<CaseOutput>, MatrixRunnerError<C::Error, E::Error>>
    where
        C: RefreshRateController,
        E: MatrixCaseExecutor,
    {
        let mut output = Vec::with_capacity(self.matrix.cases().len());
        for refresh_rate_hz in ANDROID_REFRESH_RATES_HZ {
            let cases = self
                .matrix
                .cases()
                .iter()
                .copied()
                .filter(|case| {
                    case.target == MatrixTarget::AndroidDevice
                        && case.refresh_rate_hz == refresh_rate_hz
                })
                .collect::<Vec<_>>();
            let mut group = with_fixed_refresh_rate(refresh, refresh_rate_hz, || {
                let mut group = Vec::with_capacity(cases.len());
                for case in cases {
                    executor.await_thermal_guard(
                        self.settings.thermal_guard_maximum,
                        self.settings.thermal_guard_timeout_millis,
                    )?;
                    let evidence = executor.execute(case, self.settings)?;
                    group.push(CaseOutput { case, evidence });
                    executor.cooldown(self.settings.cooldown_millis)?;
                }
                Ok(group)
            })
            .map_err(MatrixRunnerError::Android)?;
            output.append(&mut group);
        }

        for case in self
            .matrix
            .cases()
            .iter()
            .copied()
            .filter(|case| case.target == MatrixTarget::HostSynthetic)
        {
            let evidence = executor
                .execute(case, self.settings)
                .map_err(MatrixRunnerError::Host)?;
            output.push(CaseOutput { case, evidence });
        }
        Ok(output)
    }
}
