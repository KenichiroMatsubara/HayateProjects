use std::cell::RefCell;
use std::rc::Rc;

use hayate_performance_matrix::{
    MatrixCase, MatrixCaseExecutor, MatrixRunner, MatrixRunnerError, MatrixTarget,
    PerformanceMatrix, RefreshRateController, RunEvidence, RunnerSettings, DEFAULT_COOLDOWN_MILLIS,
    DEFAULT_THERMAL_GUARD_TIMEOUT_MILLIS, DEFAULT_WARMUP_FRAMES,
};

struct Refresh {
    events: Rc<RefCell<Vec<String>>>,
}

impl RefreshRateController for Refresh {
    type Mode = String;
    type Error = &'static str;

    fn current_mode(&mut self) -> Result<Self::Mode, Self::Error> {
        self.events.borrow_mut().push("refresh:capture".into());
        Ok("adaptive".into())
    }

    fn set_fixed_rate(&mut self, hz: u32) -> Result<(), Self::Error> {
        self.events.borrow_mut().push(format!("refresh:set:{hz}"));
        Ok(())
    }

    fn restore_mode(&mut self, mode: &Self::Mode) -> Result<(), Self::Error> {
        self.events
            .borrow_mut()
            .push(format!("refresh:restore:{mode}"));
        Ok(())
    }
}

struct Executor {
    events: Rc<RefCell<Vec<String>>>,
    fail_at_hz: Option<u32>,
}

impl MatrixCaseExecutor for Executor {
    type Error = &'static str;

    fn execute(
        &mut self,
        case: MatrixCase,
        settings: RunnerSettings,
    ) -> Result<RunEvidence, Self::Error> {
        self.events.borrow_mut().push(format!(
            "execute:{:?}:{}:{}:{}:{}",
            case.target,
            case.refresh_rate_hz,
            case.run_index,
            settings.warmup_frames,
            settings.cooldown_millis
        ));
        if self.fail_at_hz == Some(case.refresh_rate_hz) {
            return Err("case failed");
        }
        Ok(RunEvidence::synthetic_pass())
    }

    fn await_thermal_guard(
        &mut self,
        maximum: hayate_performance_matrix::HealthGrade,
        timeout_millis: u64,
    ) -> Result<(), Self::Error> {
        self.events
            .borrow_mut()
            .push(format!("thermal:{maximum:?}:{timeout_millis}"));
        Ok(())
    }

    fn cooldown(&mut self, millis: u64) -> Result<(), Self::Error> {
        self.events.borrow_mut().push(format!("cooldown:{millis}"));
        Ok(())
    }
}

#[test]
fn one_runner_executes_android_refresh_groups_and_host_120_hz_cases() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut refresh = Refresh {
        events: events.clone(),
    };
    let mut executor = Executor {
        events: events.clone(),
        fail_at_hz: None,
    };

    let output = MatrixRunner::new(PerformanceMatrix::standard(), RunnerSettings::default())
        .run(&mut refresh, &mut executor)
        .expect("matrix succeeds");

    assert_eq!(output.len(), PerformanceMatrix::standard().cases().len());
    assert!(output.iter().any(|output| {
        output.case.target == MatrixTarget::HostSynthetic && output.case.refresh_rate_hz == 120
    }));
    assert_eq!(
        events
            .borrow()
            .iter()
            .filter(|event| event.as_str() == "refresh:set:60")
            .count(),
        1
    );
    assert_eq!(
        events
            .borrow()
            .iter()
            .filter(|event| event.as_str() == "refresh:set:90")
            .count(),
        1
    );
    assert_eq!(
        events
            .borrow()
            .iter()
            .filter(|event| event.as_str() == "refresh:restore:adaptive")
            .count(),
        2
    );
    assert!(events.borrow().iter().any(|event| {
        event == &format!("thermal:Elevated:{}", DEFAULT_THERMAL_GUARD_TIMEOUT_MILLIS)
    }));
    assert!(events
        .borrow()
        .iter()
        .any(|event| event == &format!("cooldown:{DEFAULT_COOLDOWN_MILLIS}")));
    assert!(events.borrow().iter().any(|event| {
        event.ends_with(&format!(
            ":{}:{}",
            DEFAULT_WARMUP_FRAMES, DEFAULT_COOLDOWN_MILLIS
        ))
    }));
}

#[test]
fn android_case_failure_still_restores_refresh_and_stops_the_matrix() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut refresh = Refresh {
        events: events.clone(),
    };
    let mut executor = Executor {
        events: events.clone(),
        fail_at_hz: Some(60),
    };

    let result = MatrixRunner::new(PerformanceMatrix::standard(), RunnerSettings::default())
        .run(&mut refresh, &mut executor);

    assert!(matches!(result, Err(MatrixRunnerError::Android(_))));
    assert!(events
        .borrow()
        .iter()
        .any(|event| event == "refresh:restore:adaptive"));
    assert!(!events
        .borrow()
        .iter()
        .any(|event| event == "refresh:set:90"));
}
