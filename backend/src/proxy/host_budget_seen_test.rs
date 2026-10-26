//! What a test can see of who waits at a host's budget, so it can
//! start a second party only once the first is waiting there.
//!
//! Its own file, so that test-only readers add nothing to the budget's
//! own complexity.

use super::HostBudget;

impl HostBudget {
    /// Whether a background request waits at `host` for its token.
    pub(crate) fn background_waiting(&self, host: &str) -> bool {
        self.states
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(host)
            .is_some_and(|state| state.turn.background_waiting)
    }

    /// Whether a cached row's probe waits at `host`.
    pub(crate) fn probe_waiting(&self, host: &str) -> bool {
        self.probes_waiting(host)
    }
}
