//! The flight's decisions: which lookups share a walk, who joins
//! whose, and which walks outlive their callers.

use super::UNWATCHED_MAX;
use crate::commands::availability::{cache_key, AvailabilityArgs};

/// The row a lookup's walk can be shared under: its cache row, unless
/// it has none or asks the provider past the cache.
pub(crate) fn shared_row(args: &AvailabilityArgs) -> Option<String> {
    if args.bypass_cache {
        return None;
    }
    let id = args.kitsu_id.as_deref().filter(|id| !id.is_empty())?;
    Some(cache_key(id, &args.mode))
}

/// Whether a caller joins the walk already out for its row: a
/// background caller joins any walk, an interactive one only another
/// interactive walk.
pub(crate) fn joins(walk_interactive: bool, caller_interactive: bool) -> bool {
    walk_interactive || !caller_interactive
}

/// Whether a walk its last caller left keeps walking, given how many
/// walks already do.
pub(crate) fn outlives_its_callers(interactive: bool, unwatched: usize) -> bool {
    !interactive && unwatched < UNWATCHED_MAX
}

#[cfg(test)]
#[path = "availability_flight_prop_test.rs"]
mod prop_tests;
