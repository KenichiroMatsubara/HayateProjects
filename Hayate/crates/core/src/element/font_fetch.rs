//! On-demand font-fetch bookkeeping (issue #343).
//!
//! `.notdef` detection (ADR-0042) emits a `FetchFont { family }` event; the
//! platform adapter fetches the face and calls `register_font` on success or
//! reports a failure on error. This tracker decides, per family, whether a
//! `FetchFont` should be emitted right now, so that:
//!
//! - duplicate events are suppressed while a fetch is in flight,
//! - a *failed* fetch does not latch the family forever — it becomes eligible
//!   to be requested again on a later frame (the bug this fixes), and
//! - a family that keeps failing is given up on after a finite budget, so logs
//!   and re-requests never run away.

use std::collections::{HashMap, HashSet};

/// Maximum number of fetch attempts for one family before core gives up. The
/// adapter spaces the attempts out (backoff); core caps their count so a
/// permanently-unreachable family cannot be re-requested forever (issue #343).
pub(crate) const MAX_FETCH_ATTEMPTS: u32 = 3;

/// What core decided after a reported fetch failure.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum FailureOutcome {
    /// Budget remains — the family will be requested again on a later frame.
    WillRetry,
    /// Budget exhausted — the family is given up on and will not be re-requested.
    GaveUp,
}

/// Per-family fetch state for on-demand font loading.
#[derive(Default)]
pub(crate) struct FontFetchTracker {
    /// Families requested via `FetchFont` and awaiting a result. Suppresses
    /// duplicate events across frames while a fetch is outstanding.
    in_flight: HashSet<String>,
    /// Failed-attempt count per family, kept until the family loads or is given
    /// up on.
    attempts: HashMap<String, u32>,
    /// Families that exhausted the retry budget. Never requested again; logged
    /// at most once by the caller.
    exhausted: HashSet<String>,
}

impl FontFetchTracker {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Whether a `FetchFont` should be emitted for `family` now: only when it is
    /// neither already in flight nor given up on.
    pub(crate) fn should_request(&self, family: &str) -> bool {
        !self.in_flight.contains(family) && !self.exhausted.contains(family)
    }

    /// Record that a `FetchFont` was just emitted for `family`.
    pub(crate) fn mark_requested(&mut self, family: &str) {
        self.in_flight.insert(family.to_string());
    }

    /// Record that `family` loaded successfully. Clears all state so a future
    /// `.notdef` for the same family can request it afresh.
    pub(crate) fn mark_loaded(&mut self, family: &str) {
        self.in_flight.remove(family);
        self.attempts.remove(family);
        self.exhausted.remove(family);
    }

    /// Record that a fetch for `family` failed. Returns whether the family will
    /// be retried (`WillRetry`) or has been given up on (`GaveUp`).
    pub(crate) fn mark_failed(&mut self, family: &str) -> FailureOutcome {
        self.in_flight.remove(family);
        let count = self.attempts.entry(family.to_string()).or_insert(0);
        *count += 1;
        if *count >= MAX_FETCH_ATTEMPTS {
            self.exhausted.insert(family.to_string());
            self.attempts.remove(family);
            FailureOutcome::GaveUp
        } else {
            FailureOutcome::WillRetry
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_until_in_flight_then_clears_on_load() {
        let mut t = FontFetchTracker::new();
        assert!(t.should_request("Noto Sans JP"));
        t.mark_requested("Noto Sans JP");
        assert!(!t.should_request("Noto Sans JP"), "in-flight suppresses re-request");
        t.mark_loaded("Noto Sans JP");
        assert!(t.should_request("Noto Sans JP"), "a loaded family can be requested afresh");
    }

    #[test]
    fn failure_reopens_until_budget_is_exhausted() {
        let mut t = FontFetchTracker::new();
        for _ in 0..MAX_FETCH_ATTEMPTS - 1 {
            t.mark_requested("Noto Sans JP");
            assert_eq!(t.mark_failed("Noto Sans JP"), FailureOutcome::WillRetry);
            assert!(t.should_request("Noto Sans JP"), "a retryable failure reopens the request");
        }
        t.mark_requested("Noto Sans JP");
        assert_eq!(t.mark_failed("Noto Sans JP"), FailureOutcome::GaveUp);
        assert!(!t.should_request("Noto Sans JP"), "an exhausted family is never requested again");
    }
}
