use std::cell::RefCell;
use std::rc::Rc;

use hayate_performance_matrix::{with_fixed_refresh_rate, RefreshRateController, RefreshRunError};

struct FakeRefreshController {
    events: Rc<RefCell<Vec<String>>>,
}

impl RefreshRateController for FakeRefreshController {
    type Mode = String;
    type Error = &'static str;

    fn current_mode(&mut self) -> Result<Self::Mode, Self::Error> {
        self.events.borrow_mut().push("capture:auto".into());
        Ok("auto".into())
    }

    fn set_fixed_rate(&mut self, refresh_rate_hz: u32) -> Result<(), Self::Error> {
        self.events
            .borrow_mut()
            .push(format!("set:{refresh_rate_hz}"));
        Ok(())
    }

    fn restore_mode(&mut self, mode: &Self::Mode) -> Result<(), Self::Error> {
        self.events.borrow_mut().push(format!("restore:{mode}"));
        Ok(())
    }
}

#[test]
fn refresh_mode_is_restored_after_success_and_work_failure() {
    for should_fail in [false, true] {
        let events = Rc::new(RefCell::new(Vec::new()));
        let mut controller = FakeRefreshController {
            events: events.clone(),
        };
        let result = with_fixed_refresh_rate(&mut controller, 90, || {
            events.borrow_mut().push("work".into());
            if should_fail {
                Err("failed")
            } else {
                Ok(())
            }
        });

        if should_fail {
            assert!(matches!(result, Err(RefreshRunError::Work("failed"))));
        } else {
            assert_eq!(result, Ok(()));
        }
        assert_eq!(
            *events.borrow(),
            ["capture:auto", "set:90", "work", "restore:auto"]
        );
    }
}

#[test]
fn refresh_mode_is_restored_while_unwinding_an_interrupt_or_panic() {
    let events = Rc::new(RefCell::new(Vec::new()));
    let mut controller = FakeRefreshController {
        events: events.clone(),
    };

    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: Result<(), RefreshRunError<&'static str, &'static str>> =
            with_fixed_refresh_rate(&mut controller, 60, || panic!("interrupt"));
    }));

    assert!(panic.is_err());
    assert_eq!(*events.borrow(), ["capture:auto", "set:60", "restore:auto"]);
}
