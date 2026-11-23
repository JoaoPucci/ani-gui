//! When a lane waits for the gate's turn, and when it hands its batch
//! over ([`super`]). Split from [`super`] so each file stays inside the
//! CRAP gate's per-file bar.

use std::time::Duration;

use tokio::time::Instant;

use super::CLOSED_POLL;
use crate::meta::anilist_gate::{Class, Place};

/// Whether a lookup of `class` whose turn comes in `turn` waits for it in
/// its lane: a background lookup the gate would hold back, inside its
/// wait.
pub(crate) fn waits_for_turn(class: Class, turn: Option<Duration>) -> bool {
    class == Class::Background && turn.is_some_and(|wait| !wait.is_zero())
}

/// What a lane waiting for the gate's turn does next, `left` of its
/// lookups' wait: `Some(nap)` to look again after `nap`, `None` to hand
/// its batch to the gate now — once it would not wait for the turn, once
/// the turn is [`CLOSED_POLL`] away or nearer, and once the turn comes
/// past `left`, which the gate then turns away.
pub(crate) fn next_look(class: Class, turn: Option<Duration>, left: Duration) -> Option<Duration> {
    let turn = turn.filter(|_| waits_for_turn(class, turn))?;
    if turn <= CLOSED_POLL || turn > left {
        return None;
    }
    Some((turn - CLOSED_POLL).min(CLOSED_POLL))
}

/// When a held batch's wait for a turn runs out: `budget` after its wait
/// began, or after its last lookup joined, whichever is later — so no
/// lookup in it is turned away sooner than it would have been alone.
pub(crate) fn batch_deadline(started: Instant, last_join: Instant, budget: Duration) -> Instant {
    started.max(last_join) + budget
}

/// What a lane hands the gate with its batch: the place it held, and
/// when the lookups' wait for a turn runs out.
pub(super) struct Handoff {
    pub(super) place: Option<Place>,
    pub(super) deadline: Instant,
}
