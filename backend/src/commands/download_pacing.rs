//! How a download yields to live playback. yt-dlp fetches sixteen
//! fragments at once, and against the host the player is streaming
//! from that burst spends the address's request budget at the host,
//! which refuses the player's next segment within seconds. While the
//! proxy has served media recently — playback is live — a download
//! runs one fragment at a time at a limited byte rate instead. yt-dlp cannot change its
//! concurrency mid-run, so when playback starts or stops under a
//! running download the supervisor takes the tool down and starts it
//! again on the same output, which yt-dlp resumes from the fragments
//! it already has ([`super::download::spawn_download_tool_paced`]).
//!
//! The allowance is one fragment beside the player for the whole app,
//! not one per download: paced runs take [`PACED_LANE`] in turn, so
//! two episodes downloading during playback put one yt-dlp against
//! the host at a time, and the other waits for it or for playback to
//! stop, whichever comes first. The ffmpeg fallback is one connection
//! whose rate is set at its start — the stream's own while playback is
//! live, full speed otherwise, kept to its end either way — and that
//! cannot be resumed, so it holds the lane from its start regardless
//! of playback.

use std::time::Duration;

use tokio::sync::{Semaphore, SemaphorePermit};

/// Fragments in flight when nothing is playing: v5's `-N 16`.
pub(crate) const FAST_FRAGMENTS: u32 = 16;

/// Fragments in flight while playback is live: one, the floor a
/// download can pace to and still move. How much of the address's
/// request budget that one fragment spends is set by the byte-rate
/// limit beside it.
pub(crate) const PACED_FRAGMENTS: u32 = 1;

/// The sidecar phase's deadline while playback is live: the tracks
/// take their tokens one at a time behind the player's, sharing one
/// lane with every other download's, and a listing at the track cap
/// needs the time.
pub(crate) const SIDECAR_PHASE_DEADLINE_LIVE: Duration = Duration::from_secs(4 * 60);

/// How many subtitle tracks a download fetches at once beside its
/// transfer, chosen once as the phase starts: one while playback is
/// live, so the phase has at most one fetch in flight;
/// [`SIDECAR_FETCH_CONCURRENCY`](super::download::SIDECAR_FETCH_CONCURRENCY)
/// otherwise. Across downloads, [`SIDECAR_LANE`] bounds how many are
/// in flight while playback is live.
#[must_use]
pub(crate) fn sidecar_concurrency(playback_live: bool) -> usize {
    if playback_live {
        1
    } else {
        super::download::SIDECAR_FETCH_CONCURRENCY
    }
}

/// The sidecar phase's deadline, chosen once as the phase starts:
/// [`SIDECAR_PHASE_DEADLINE`](super::download::SIDECAR_PHASE_DEADLINE)
/// for a host that stalls, [`SIDECAR_PHASE_DEADLINE_LIVE`] while
/// playback is live.
#[must_use]
pub(crate) fn sidecar_phase_deadline(playback_live: bool) -> Duration {
    if playback_live {
        SIDECAR_PHASE_DEADLINE_LIVE
    } else {
        super::download::SIDECAR_PHASE_DEADLINE
    }
}

/// Fragments a relayed run keeps in flight. The proxy admits each as
/// background traffic, so how many are in flight does not decide how
/// many reach the host; a few keep one ready whenever the player
/// leaves a token.
pub(crate) const RELAYED_FRAGMENTS: u32 = 4;

/// How long a relayed run's tool waits for a request's first byte, in
/// seconds. The proxy holds a background request until the player
/// leaves a token, which while the player fills its buffer can be a
/// minute or more; the tool's default of twenty seconds would give up
/// on requests that are only waiting their turn.
pub(crate) const RELAYED_SOCKET_TIMEOUT_S: u32 = 300;

/// The byte rate a paced run is held to, in yt-dlp's `--limit-rate`
/// spelling. The host counts requests per address, and one fragment
/// at a time against small segments is still several requests a
/// second; at this rate a megabyte segment takes about two seconds,
/// which keeps the download's share of the address's budget small
/// beside the player's own fetches.
pub(crate) const PACED_RATE_LIMIT: &str = "512K";

/// A paced run's own ceiling: a day. The transfer's ceiling guards a
/// hung tool, and a paced run is slow by design and ends the moment
/// playback stops — so it runs under this one instead, and the time it
/// took is added to the transfer's ceiling for what follows. An ffmpeg
/// fallback started while playback is live runs under it too: it
/// reads at the stream's rate to its end, as long as the stream plays.
pub(crate) const PACED_RUN_CEILING: Duration = Duration::from_secs(24 * 60 * 60);

/// How long after the last media fetch playback counts as live: a
/// playing player fetches a segment every few seconds, and a paused
/// one with a full buffer fetches nothing, so a window of silence is
/// the player standing still.
pub(crate) const PLAYBACK_LIVE_WINDOW: Duration = Duration::from_secs(30);

/// How often a running transfer asks whether playback changed.
pub(crate) const PACING_POLL: Duration = Duration::from_secs(2);

/// The one paced lane of the app: while playback is live, yt-dlp runs
/// for one download at a time. Free runs — nothing playing — do not
/// take it. One per process because the host sees one address.
pub(crate) static PACED_LANE: Semaphore = Semaphore::const_new(1);

/// The lane a transfer nothing paces holds: every permit at once, so
/// the tests that drive the unpaced entry point never queue behind one
/// another or behind the app's own lane.
#[cfg(test)]
static NEVER_LANE: Semaphore = Semaphore::const_new(Semaphore::MAX_PERMITS);

/// The sidecar fetches' lane while playback is live: one in flight at
/// a time across every download, whatever concurrency each
/// download's phase chose, so the idle tokens the player leaves go to
/// one track at a time. None of them waits in the host's line ahead
/// of the player — they are background traffic at the host's budget
/// ([`crate::proxy::host_budget::HostBudget::admit_background`]),
/// including a fetch that asked before playback went live and is past
/// the gate already.
pub(crate) static SIDECAR_LANE: Semaphore = Semaphore::const_new(1);

/// What a sidecar fetch asks before it goes to the host: whether
/// playback is live now, and if it is, a turn on the lane every
/// download's sidecar fetches share. Asked per fetch, so a phase that
/// began before playback did yields from its next fetch on.
pub(crate) struct SidecarGate<'a> {
    is_live: &'a (dyn Fn() -> bool + Sync),
    lane: &'a Semaphore,
}

impl<'a> SidecarGate<'a> {
    /// A gate over a view of playback and a lane of the caller's.
    /// Only the transfer's cases build one, and they run on Unix alone.
    #[cfg(all(test, unix))]
    pub(crate) fn new(is_live: &'a (dyn Fn() -> bool + Sync), lane: &'a Semaphore) -> Self {
        Self { is_live, lane }
    }

    /// A gate nothing closes: playback is never live.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn never() -> SidecarGate<'static> {
        SidecarGate {
            is_live: &|| false,
            lane: &NEVER_LANE,
        }
    }

    /// A turn on the shared lane while playback is live, held for the
    /// fetch; nothing otherwise.
    pub(crate) async fn turn(&self) -> Option<SemaphorePermit<'a>> {
        if (self.is_live)() {
            // The lane is a static nobody closes.
            self.lane.acquire().await.ok()
        } else {
            None
        }
    }
}

/// The fragment concurrency for the current state of playback.
#[must_use]
pub(crate) fn fragment_concurrency(playback_live: bool) -> u32 {
    if playback_live {
        PACED_FRAGMENTS
    } else {
        FAST_FRAGMENTS
    }
}

/// The byte-rate limit for the current state of playback: held while
/// playback is live, none otherwise.
#[must_use]
pub(crate) fn rate_limit(playback_live: bool) -> Option<&'static str> {
    playback_live.then_some(PACED_RATE_LIMIT)
}

/// The transfer's view of playback: a question it can ask at any
/// moment, and how often it asks while a tool runs. Production asks
/// the session table the proxy shares with the download command; the
/// tests hand it a flag.
pub(crate) struct Pacing<'a> {
    is_live: &'a (dyn Fn() -> bool + Sync),
    poll: Duration,
    lane: &'a Semaphore,
    relay: Option<&'a (dyn Fn() -> Option<String> + Sync)>,
}

impl<'a> Pacing<'a> {
    pub(crate) fn new(
        is_live: &'a (dyn Fn() -> bool + Sync),
        poll: Duration,
        lane: &'a Semaphore,
    ) -> Self {
        Self {
            is_live,
            poll,
            lane,
            relay: None,
        }
    }

    /// The same pacing with a relay: where the transfer fetches from
    /// while playback is live — the app's proxy, which charges the
    /// download's requests to the host's budget behind the player's —
    /// in place of a byte-rate cap.
    #[must_use]
    pub(crate) fn with_relay(self, relay: &'a (dyn Fn() -> Option<String> + Sync)) -> Self {
        Self {
            relay: Some(relay),
            ..self
        }
    }

    /// The relay's URL for this transfer, when it has one.
    pub(crate) fn relay_url(&self) -> Option<String> {
        self.relay.and_then(|relay| relay())
    }

    /// The gate this transfer's sidecar fetches ask: the same view of
    /// playback, and the lane every download's sidecar fetches share.
    pub(crate) fn sidecar_gate(&self) -> SidecarGate<'a> {
        SidecarGate {
            is_live: self.is_live,
            lane: &SIDECAR_LANE,
        }
    }

    /// A transfer nothing paces: playback is never live, and the lane
    /// it holds for a fallback has a permit for everyone. The seam the
    /// unpaced entry point and its tests run on.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn never() -> Pacing<'static> {
        Pacing {
            is_live: &|| false,
            poll: Duration::from_secs(3600),
            lane: &NEVER_LANE,
            relay: None,
        }
    }

    /// The lane, whether or not anything plays: for a run that is one
    /// connection, keeps the rate it started with and cannot be
    /// resumed, so that if playback starts under it, it is already
    /// the one connection the allowance grants.
    pub(crate) async fn lane(&self) -> SemaphorePermit<'a> {
        self.lane
            .acquire()
            .await
            .expect("the paced lane is never closed")
    }

    /// A turn on the paced lane for one run of the tool. `Some` holds
    /// the lane until dropped; `None` means playback stopped while
    /// the lane was busy, and the run is free instead. Re-asks after
    /// acquiring, since the lane may have opened because the holder's
    /// own playback check went idle.
    pub(crate) async fn paced_turn(&self) -> Option<SemaphorePermit<'a>> {
        tokio::select! {
            permit = self.lane.acquire() => {
                let permit = permit.expect("the paced lane is never closed");
                self.is_live().then_some(permit)
            }
            () = self.until_live_changes(true) => None,
        }
    }

    #[must_use]
    pub(crate) fn is_live(&self) -> bool {
        (self.is_live)()
    }

    /// Resolves once playback is no longer in the state `current`
    /// describes — the moment a running tool should be respawned at
    /// the other concurrency. Never resolves while nothing changes.
    pub(crate) async fn until_live_changes(&self, current: bool) {
        loop {
            tokio::time::sleep(self.poll).await;
            if self.is_live() != current {
                return;
            }
        }
    }
}

#[cfg(test)]
#[path = "download_pacing_test.rs"]
mod tests;
