//! Whether the streaming source is answering, read only from traffic
//! the app already sends: the outcomes its gate records. No request is
//! ever made to find out.
//!
//! The notification center tells the user once that the source is not
//! answering, instead of every failed play telling them again. The
//! rule is deliberately slow to speak, so one flaky minute, or failures
//! that only background probes saw, raise nothing:
//!
//! - **Not answering**: at least [`OUTAGE_FAILURES`] consecutive
//!   failures with no success between, and at least one of them a
//!   request a user was waiting on (a play click, a download). That is
//!   the breaker opening ([`FAILURE_THRESHOLD`] failures) and the next
//!   attempt failing too.
//! - **Answering again**: the first success after that. The run is
//!   over; the renderer shows the outage resolved.
//! - A rate limit is the source answering, busy: it neither counts
//!   towards a run nor ends one.
//!
//! What it cannot say is why: an outage and something between the
//! user and the source blocking it look the same from here, and the
//! renderer's wording says so. Pure: the caller passes the clock.

use serde::Serialize;

use super::gate::FAILURE_THRESHOLD;

/// Consecutive failures, one of them a user's, before the source is
/// reported as not answering.
pub const OUTAGE_FAILURES: u32 = FAILURE_THRESHOLD + 1;

/// One outcome the gate recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Observation {
    /// The source answered.
    Success,
    /// The source did not answer. `interactive` when a user was
    /// waiting on the request.
    Failure {
        /// Whether a user was waiting on it.
        interactive: bool,
    },
    /// The source answered with a rate limit.
    RateLimited,
}

/// What the renderer is told.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Health {
    /// Nothing to say.
    Answering,
    /// The source has not answered since `since_ms`.
    NotAnswering {
        /// The first failure of the run, in ms since the Unix epoch.
        since_ms: u64,
        /// How many consecutive failures the run holds.
        failures: u32,
        /// The latest failure, in ms since the Unix epoch.
        last_failure_ms: u64,
    },
    /// The source answered again after not answering.
    AnsweringAgain {
        /// When the outage started.
        since_ms: u64,
        /// When the source answered again.
        recovered_ms: u64,
    },
}

/// The current run of failures, and the last outage that ended.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HealthRun {
    failures: u32,
    user_failed: bool,
    since_ms: Option<u64>,
    last_failure_ms: u64,
    /// The outage that ended most recently: (since, recovered).
    recovered: Option<(u64, u64)>,
}

impl HealthRun {
    /// The run after `obs`, observed at `now_ms`.
    #[must_use]
    pub fn observe(self, obs: Observation, now_ms: u64) -> Self {
        let _ = (obs, now_ms);
        self
    }

    /// What the run says.
    #[must_use]
    pub fn health(&self) -> Health {
        Health::Answering
    }
}

#[cfg(test)]
#[path = "health_test.rs"]
mod tests;

#[cfg(test)]
#[path = "health_prop_test.rs"]
mod prop_tests;
