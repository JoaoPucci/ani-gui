//! One walk per availability row.
//!
//! A lookup that reaches the provider walks it — searches, alias
//! recovery, the episode listing — and against the scraper gate's
//! background pacing that walk can take most of a minute. Nothing
//! used to stop the same row being walked several times at once: every
//! Home mount warmed its rails afresh and re-asked its Continue
//! Watching rows, so a show still being walked from the last mount was
//! walked again beside it, each walk taking its share of the same
//! background admissions and slowing all of them toward the deadline.
//!
//! So a walk is a flight, kept per cache row — `(kitsu_id, mode)`, the
//! key [`cache_key`] names — and a lookup of a row whose walk is out
//! waits on that walk instead of starting another. The walk runs in a
//! task of its own, not inside any one caller, so who leaves when is a
//! question of bookkeeping rather than of whose future is dropped:
//!
//! - **Not shared.** A lookup with no Kitsu id has no row to share,
//!   and a cache-skipping refresh asks the provider itself: a walk
//!   already out may have set off before the question the refresh
//!   exists to answer was asked. Both walk inline, as before.
//! - **Joining.** A background caller joins whatever walk is out. An
//!   interactive caller joins an interactive walk only: a background
//!   walk waits its paced turn at the gate for every request, and the
//!   user waiting on the answer would wait with it.
//! - **Displacing.** An interactive caller finding a background walk
//!   out starts an interactive one in its place. The background walk
//!   ends where it is, and its callers move onto the interactive walk
//!   rather than losing their answer.
//! - **The first caller leaves while others wait.** The walk goes on;
//!   whoever is still waiting gets its verdict.
//! - **The last caller leaves an interactive walk.** It ends: nobody
//!   is waiting on it, and it is not paced.
//! - **The last caller leaves a background walk.** It walks on — a
//!   Continue Watching probe abandoned with its page is the same walk
//!   the next mount asks for, and its verdict is cached for whoever
//!   asks next — up to [`UNWATCHED_MAX`] such walks; past that, it
//!   ends. A caller arriving while it is out joins it.
//! - **The walk finishes.** It leaves the table before it publishes,
//!   so a caller arriving afterwards starts a walk of its own — which
//!   finds the verdict cached, or walks again where there was none.
//! - **The walk panics.** Its entry is removed as the task unwinds,
//!   so nothing is left behind, and its callers fail the way a lookup
//!   whose walk panicked always did.
//!
//! The walk is the first caller's: its arguments, and the AniList
//! class it was asked in. Every caller of a row asks about the same
//! show — the row is keyed on it — and the verdict is written to the
//! one row whoever asked.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use tokio::sync::watch;
use tokio::task::AbortHandle;

use super::availability::{cache_key, check_availability, AvailabilityArgs, AvailabilityResponse};
use crate::app::AppState;
use crate::error::Result;

/// How many background walks may run on with nobody waiting. Three is
/// the renderer's share of background probes at once, so a page left
/// with all of them out keeps all of them.
pub(crate) const UNWATCHED_MAX: usize = 3;

/// What a walk's callers hear.
#[derive(Clone)]
enum Slot {
    Walking,
    Answered(Result<AvailabilityResponse>),
    /// An interactive walk took this one's place: ask again.
    Superseded,
}

struct Flight {
    id: u64,
    interactive: bool,
    slot: watch::Sender<Slot>,
    walk: AbortHandle,
    waiters: usize,
}

#[derive(Default)]
struct Table {
    flights: HashMap<String, Flight>,
    next_id: u64,
    /// Live background walks nobody is waiting on.
    unwatched: usize,
}

impl Table {
    /// Take the flight out of the table, if `id` is still the one out
    /// for `key`.
    fn take(&mut self, key: &str, id: u64) -> Option<Flight> {
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

/// The walks in flight, per row. One per app state; a clone shares
/// the table.
#[derive(Clone, Default)]
pub struct AvailabilityFlights {
    table: Arc<Mutex<Table>>,
}

/// Answer `args`, sharing the walk of its row with every other lookup
/// of that row out at the same time (see the module docs).
///
/// # Errors
/// What [`check_availability`] returns.
///
/// # Panics
/// When the walk this lookup waited on panicked.
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
    fn table(&self) -> std::sync::MutexGuard<'_, Table> {
        self.table.lock().unwrap_or_else(PoisonError::into_inner)
    }

    async fn check(
        &self,
        state: &Arc<AppState>,
        args: AvailabilityArgs,
    ) -> Result<AvailabilityResponse> {
        let Some(key) = shared_row(&args) else {
            return check_availability(state, &args).await;
        };
        loop {
            let (mut slot, _waiting) = self.join(state, &key, &args);
            let heard = slot
                .wait_for(|s| !matches!(s, Slot::Walking))
                .await
                .map(|s| s.clone());
            match heard {
                Ok(Slot::Answered(got)) => return got,
                Ok(Slot::Superseded) => {}
                Ok(Slot::Walking) | Err(_) => {
                    panic!("the availability walk for {key} ended without an answer")
                }
            }
        }
    }

    /// Wait on the walk out for `key`, or start one.
    fn join(
        &self,
        state: &Arc<AppState>,
        key: &str,
        args: &AvailabilityArgs,
    ) -> (watch::Receiver<Slot>, Waiting) {
        let interactive = !args.background;
        let mut table = self.table();
        let mut displaced = None;
        if let Some(flight) = table.flights.get_mut(key) {
            if joins(flight.interactive, interactive) {
                let was_unwatched = flight.waiters == 0;
                flight.waiters += 1;
                let joined = (flight.slot.subscribe(), self.waiter(key, flight.id));
                if was_unwatched {
                    table.unwatched -= 1;
                }
                return joined;
            }
            let id = flight.id;
            displaced = table.take(key, id);
        }
        table.next_id += 1;
        let id = table.next_id;
        let (slot, heard) = watch::channel(Slot::Walking);
        let walk = tokio::spawn(crate::meta::anilist_gate::in_callers_class(walk(
            Arc::clone(state),
            args.clone(),
            self.clone(),
            key.to_owned(),
            id,
        )))
        .abort_handle();
        table.flights.insert(
            key.to_owned(),
            Flight {
                id,
                interactive,
                slot,
                walk,
                waiters: 1,
            },
        );
        drop(table);
        if let Some(old) = displaced {
            old.walk.abort();
            old.slot.send_replace(Slot::Superseded);
        }
        (heard, self.waiter(key, id))
    }

    fn waiter(&self, key: &str, id: u64) -> Waiting {
        Waiting {
            flights: self.clone(),
            key: key.to_owned(),
            id,
        }
    }

    /// A caller of `key`'s walk `id` stopped waiting.
    fn leave(&self, key: &str, id: u64) {
        let mut table = self.table();
        let unwatched = table.unwatched;
        let Some(flight) = table.flights.get_mut(key).filter(|f| f.id == id) else {
            return;
        };
        flight.waiters -= 1;
        if flight.waiters > 0 {
            return;
        }
        // Unwatched from here, whether it walks on or `take` ends it
        // and uncounts it again.
        table.unwatched += 1;
        if outlives_its_callers(flight_is_interactive(&table, key), unwatched) {
            return;
        }
        let ended = table.take(key, id);
        drop(table);
        if let Some(flight) = ended {
            flight.walk.abort();
        }
    }
}

fn flight_is_interactive(table: &Table, key: &str) -> bool {
    table.flights.get(key).is_some_and(|f| f.interactive)
}

/// The walk itself, run in a task of its own.
async fn walk(
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

/// One caller waiting on a walk; dropping it — the caller answered,
/// or gone — is that caller leaving.
struct Waiting {
    flights: AvailabilityFlights,
    key: String,
    id: u64,
}

impl Drop for Waiting {
    fn drop(&mut self) {
        self.flights.leave(&self.key, self.id);
    }
}

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
impl AvailabilityFlights {
    /// Walks in flight.
    pub(crate) fn live(&self) -> usize {
        self.table().flights.len()
    }

    /// Callers waiting on the walk out for `key`, if one is.
    pub(crate) fn waiting(&self, key: &str) -> Option<usize> {
        self.table().flights.get(key).map(|f| f.waiters)
    }

    /// Every live walk nobody waits on is counted, and nothing else.
    pub(crate) fn assert_consistent(&self) {
        let table = self.table();
        let unwatched = table.flights.values().filter(|f| f.waiters == 0).count();
        assert_eq!(table.unwatched, unwatched, "the unwatched count");
    }

    /// Whether the walk out for `key` is interactive, if one is.
    pub(crate) fn interactive(&self, key: &str) -> Option<bool> {
        self.table().flights.get(key).map(|f| f.interactive)
    }
}

#[cfg(test)]
#[path = "availability_flight_prop_test.rs"]
mod prop_tests;
