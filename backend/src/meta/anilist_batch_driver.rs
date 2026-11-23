//! The task gathering one lane's batches ([`super`]): each window, it
//! waits for the gate's turn when the gate would hold the batch back,
//! then hands the batch to a task of its own. Split from [`super`] so
//! each file stays inside the CRAP gate's per-file bar.

use std::marker::PhantomData;
use std::time::Duration;

use tokio::time::Instant;

use super::turn::{batch_deadline, next_look, Handoff};
use super::{deliver, Closing, Reply, WINDOW};
use crate::meta::anilist_batch::{Kind, Want};
use crate::meta::anilist_gate::{handing, in_class, place, turn_in, wait_for, Class};

/// The task gathering one lane's batches.
pub(super) struct Driver<K> {
    pub(super) lane: String,
    pub(super) client: reqwest::Client,
    pub(super) base: Option<String>,
    /// The endpoint, for its gate.
    pub(super) url: String,
    pub(super) class: Class,
    /// How long the first batch gathers before it waits for its turn.
    pub(super) first_wait: Duration,
    pub(super) kind: PhantomData<K>,
}

impl<K: Kind> Driver<K> {
    /// Each window, hand what the lane gathered to a task of its own and
    /// go on gathering: a lookup arriving while a batch is out goes in
    /// the next window's batch, not after the first one answers.
    pub(super) async fn run(self, mut guard: Closing<K>) {
        let mut wait = self.first_wait;
        loop {
            if !wait.is_zero() {
                tokio::time::sleep(wait).await;
            }
            wait = WINDOW;
            let Some(handoff) = self.until_turn().await else {
                guard.0 = None;
                return;
            };
            let Some(batch) = self.take() else {
                guard.0 = None;
                return;
            };
            let (wants, replies): (Vec<Want>, Vec<Reply<K::Out>>) = batch
                .into_iter()
                .filter(|(_, reply)| !reply.is_closed())
                .unzip();
            let client = self.client.clone();
            let base = self.base.clone();
            tokio::spawn(in_class(
                self.class,
                handing(
                    handoff.place,
                    handoff.deadline,
                    deliver::<K>(client, base, wants, replies),
                ),
            ));
        }
    }

    /// Wait, while what the lane gathered would be held back for a turn
    /// inside its wait, for that turn, holding a place in the gate's
    /// queue; callers that stop waiting meanwhile are let go. The place,
    /// if one was taken; `None`, and the lane closed, once nobody waits.
    ///
    /// The wait counts against the lookups' own: past what is left of it
    /// the batch is handed to the gate, with that deadline
    /// ([`Handoff::deadline`]), which turns it away as it would have
    /// turned away a lookup waiting there alone. The last
    /// [`CLOSED_POLL`] is waited in the gate's own queue, where the batch
    /// takes its turn with no step left before it.
    async fn until_turn(&self) -> Option<Handoff> {
        let started = Instant::now();
        let budget = wait_for(&self.url, self.class);
        let mut held = None;
        loop {
            if !self.still_wanted() {
                return None;
            }
            // Marked held in the step that checks it, so no lookup Kitsu
            // maps to AniList alone joins it from here. One gathered in
            // the window goes now, as it went before: held, its page
            // would wait for the MAL page of lookups that joined after it.
            let Some(last_join) = self.hold() else {
                return Some(Handoff {
                    place: None,
                    deadline: started + budget,
                });
            };
            let deadline = batch_deadline(started, last_join, budget);
            let left = deadline.saturating_duration_since(Instant::now());
            let Some(nap) = next_look(self.class, turn_in(&self.url, self.class), left) else {
                return Some(Handoff {
                    place: held,
                    deadline,
                });
            };
            if held.is_none() {
                held = Some(place(&self.url));
            }
            tokio::time::sleep(nap).await;
        }
    }

    /// Mark the lane held for the gate's turn, unless it gathered a
    /// lookup Kitsu maps to AniList alone; when its last lookup joined,
    /// if it is held.
    fn hold(&self) -> Option<Instant> {
        let mut lanes = K::lanes().open();
        let open = lanes.get_mut(&self.lane)?;
        if open.pending.iter().any(|(want, _)| want.mal.is_none()) {
            return None;
        }
        open.held = true;
        Some(open.last_join)
    }

    /// Drop the gathered lookups whose callers stopped waiting; whether
    /// any is left. A lane left with none is closed.
    fn still_wanted(&self) -> bool {
        let mut lanes = K::lanes().open();
        let Some(open) = lanes.get_mut(&self.lane) else {
            return false;
        };
        open.pending.retain(|(_, reply)| !reply.is_closed());
        if open.pending.is_empty() {
            lanes.remove(&self.lane);
            return false;
        }
        true
    }

    /// The lookups gathered since the last batch; `None`, and the lane
    /// closed, when there are none.
    fn take(&self) -> Option<Vec<(Want, Reply<K::Out>)>> {
        let mut lanes = K::lanes().open();
        let open = lanes.get_mut(&self.lane)?;
        if open.pending.is_empty() {
            lanes.remove(&self.lane);
            return None;
        }
        // The next batch gathers for its window first.
        open.held = false;
        Some(std::mem::take(&mut open.pending))
    }
}
