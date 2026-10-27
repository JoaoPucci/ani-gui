//! The entry every availability lookup that reaches the provider is
//! asked through: the route's lookups and the warm's alike.

use std::sync::Arc;

use super::availability::{check_availability, AvailabilityArgs, AvailabilityResponse};
use crate::app::AppState;
use crate::error::Result;

/// The walks in flight, per row.
#[derive(Clone, Default)]
pub struct AvailabilityFlights;

/// Answer `args`.
///
/// # Errors
/// What [`check_availability`] returns.
pub async fn check_shared(
    state: &Arc<AppState>,
    args: AvailabilityArgs,
) -> Result<AvailabilityResponse> {
    state
        .availability_refreshes
        .flights
        .check(state, args)
        .await
}

impl AvailabilityFlights {
    /// Answer `args` for `state`.
    async fn check(
        &self,
        state: &Arc<AppState>,
        args: AvailabilityArgs,
    ) -> Result<AvailabilityResponse> {
        let _ = self;
        check_availability(state, &args).await
    }
}

/// How many background walks may run on with nobody waiting.
pub(crate) const UNWATCHED_MAX: usize = 3;

#[cfg(test)]
impl AvailabilityFlights {
    /// Walks in flight.
    pub(crate) fn live(&self) -> usize {
        0
    }

    /// Callers waiting on the walk out for `key`, if one is.
    pub(crate) fn waiting(&self, _key: &str) -> Option<usize> {
        None
    }

    /// Whether the walk out for `key` is interactive, if one is.
    pub(crate) fn interactive(&self, _key: &str) -> Option<bool> {
        None
    }
}

/// The row a lookup's walk can be shared under.
pub(crate) fn shared_row(_args: &AvailabilityArgs) -> Option<String> {
    None
}

/// Whether a caller joins the walk already out for its row.
pub(crate) fn joins(_walk_interactive: bool, _caller_interactive: bool) -> bool {
    false
}

/// Whether a walk its last caller left keeps walking.
pub(crate) fn outlives_its_callers(_interactive: bool, _unwatched: usize) -> bool {
    false
}

#[cfg(test)]
#[path = "availability_flight_prop_test.rs"]
mod prop_tests;
