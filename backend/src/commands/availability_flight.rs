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
