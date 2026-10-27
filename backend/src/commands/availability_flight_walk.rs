//! A flight's walk, run in a task of its own, and the bookkeeping
//! that takes it out of the table however it ends.

use std::sync::Arc;

use super::{AvailabilityFlights, Flight, Slot, Table};
use crate::app::AppState;
use crate::commands::availability::{check_availability, AvailabilityArgs};

impl Table {
    /// Take the flight out of the table, if `id` is still the one out
    /// for `key`.
    pub(super) fn take(&mut self, key: &str, id: u64) -> Option<Flight> {
        if self.flights.get(key)?.id != id {
            return None;
        }
        let flight = self.flights.remove(key)?;
        if flight.waiters == 0 {
            self.unwatched -= 1;
        }
        Some(flight)
    }
}

/// The walk itself, run in a task of its own.
pub(super) async fn walk(
    state: Arc<AppState>,
    args: AvailabilityArgs,
    flights: AvailabilityFlights,
    key: String,
    id: u64,
) {
    let mut out = Out {
        flights,
        key,
        id,
        done: false,
    };
    let got = check_availability(&state, &args).await;
    out.done = true;
    let finished = out.flights.table().take(&out.key, id);
    if let Some(flight) = finished {
        flight.slot.send_replace(Slot::Answered(got));
    }
}

/// Takes a walk that did not finish — aborted, or unwinding from a
/// panic — out of the table. An aborted walk is out already; a
/// panicking one would otherwise stay there for good.
struct Out {
    flights: AvailabilityFlights,
    key: String,
    id: u64,
    done: bool,
}

impl Drop for Out {
    fn drop(&mut self) {
        if !self.done {
            drop(self.flights.table().take(&self.key, self.id));
        }
    }
}
