//! How a request takes a token from a host's budget: the player's
//! line, served in the order it arrived, and background traffic's
//! lane, which takes its turns beside a waiting player and leaves the
//! player a reserve.

use tokio::time::Instant;

use super::{player_turns, take_leaving, HostBudget, Turn};

/// Clears the waiting background request's turn when it is served or
/// given up, so a request dropped while it waits leaves no turn behind
/// it for the player to yield to.
struct BackgroundWaiting<'a> {
    budget: &'a HostBudget,
    host: &'a str,
}

impl Drop for BackgroundWaiting<'_> {
    fn drop(&mut self) {
        self.budget
            .with_state(self.host, |state| state.turn = Turn::default());
    }
}

impl HostBudget {
    /// A token for `host`, waiting for one while the burst is spent: the
    /// player's requests, through the proxy and a cached row's liveness
    /// check before it starts. Waiters are served in the order they arrived: each takes its
    /// place in the host's line and keeps it until it has its token,
    /// so a request arriving as a token matures does not take it from
    /// one that has waited for it, and a few requests arriving at once
    /// cannot keep taking the tokens ahead of the one that has waited
    /// longest. While background traffic waits, the tokens the player
    /// had to wait for count toward its turns ([`player_turns`]); once
    /// it has taken them the next token is the background request's,
    /// and while that one is owed the line yields it.
    pub(crate) async fn admit(&self, host: &str) {
        let line = self.line(host);
        let _place = line.lock().await;
        let mut waited = false;
        loop {
            let wait = self.with_state(host, |state| {
                if state.turn.background_owed {
                    return Some(self.refill);
                }
                let wait = take_leaving(
                    &mut state.bucket,
                    Instant::now(),
                    self.burst,
                    self.refill,
                    0,
                );
                if wait.is_none() && waited && state.turn.background_waiting {
                    state.turn.player_taken += 1;
                    let demand = state.demand.per_second(Instant::now());
                    state.turn.background_owed = player_turns(demand, self.refill)
                        .is_some_and(|turns| state.turn.player_taken >= turns);
                }
                wait
            });
            match wait {
                None => return,
                Some(wait) => {
                    tracing::debug!(
                        host,
                        wait_ms = wait.as_millis(),
                        "budget: pacing a fetch to the host's budget",
                    );
                    waited = true;
                    tokio::time::sleep(wait).await;
                }
            }
        }
    }

    /// A token for `host` for background traffic — subtitle tracks,
    /// the player's and a download's, and a download's fetches through
    /// the proxy. Background requests take their turn one at a time,
    /// in the order they arrived, however many are in flight. With no
    /// one in the host's line, the one whose turn it is takes a token
    /// only while more than the budget's reserve
    /// ([`BACKGROUND_RESERVE`] in the app's budget) is on hand, so the
    /// player's next requests find the reserve there; while a download
    /// tool runs free, at most one less than what its hold leaves is
    /// kept ([`Self::reserve_under_hold`]). With the player
    /// waiting in the line it yields until the player has taken its
    /// turns ([`player_turns`]) and then takes the next: while both
    /// wait, the player gets what it needs with room and the
    /// background request the rest of the refill. While the player's
    /// need with its room is the whole refill or more there is no rest,
    /// and the background request waits until the player stops asking.
    /// A cached row's probe waiting at the host ([`Self::admit_probe`])
    /// is served first: background traffic waits while one does.
    pub(crate) async fn admit_background(&self, host: &str) {
        let lane = self.lane(host);
        let _turn = lane.lock().await;
        let line = self.line(host);
        let _waiting = BackgroundWaiting { budget: self, host };
        self.with_state(host, |state| state.turn.background_waiting = true);
        loop {
            if self.probes_waiting(host) {
                tokio::time::sleep(self.refill).await;
                continue;
            }
            let player_waiting = line.try_lock().is_err();
            let wait = self.with_state(host, |state| {
                let reserve = match (player_waiting, state.turn.background_owed) {
                    (_, true) => 0,
                    (false, false) => self.reserve,
                    (true, false) => return Some(self.refill),
                };
                let reserve = self.reserve_under_hold(reserve);
                take_leaving(
                    &mut state.bucket,
                    Instant::now(),
                    self.burst,
                    self.refill,
                    reserve,
                )
            });
            // Looks again within a refill whatever the wait: a turn
            // the player owes it, or a player joining the line, changes
            // what it may take.
            match wait {
                None => return,
                Some(wait) => tokio::time::sleep(wait.min(self.refill)).await,
            }
        }
    }
}
