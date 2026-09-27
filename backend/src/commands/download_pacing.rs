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
//! that can be neither paced down nor resumed, so it holds the lane
//! from its start regardless of playback.

use std::time::Duration;

use tokio::sync::{Semaphore, SemaphorePermit};

/// Fragments in flight when nothing is playing: v5's `-N 16`.
pub(crate) const FAST_FRAGMENTS: u32 = 16;

/// Fragments in flight while playback is live: one, the floor a
/// download can pace to and still move. The player holds up to three
/// connections of its own to the same host — a segment, a playlist,
/// a subtitle — and a live run with two download fragments beside
/// them still lost the player's segment request.
pub(crate) const PACED_FRAGMENTS: u32 = 1;

/// The byte rate a paced run is held to, in yt-dlp's `--limit-rate`
/// spelling. The host counts requests per address, and one fragment
/// at a time against small segments is still several requests a
/// second; at this rate a megabyte segment takes about two seconds,
/// which keeps the download's share of the address's budget small
/// beside the player's own fetches.
pub(crate) const PACED_RATE_LIMIT: &str = "512K";

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
        }
    }

    /// A transfer nothing paces: playback is never live. The seam the
    /// unpaced entry point and its tests run on.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn never() -> Pacing<'static> {
        Pacing {
            is_live: &|| false,
            poll: Duration::from_secs(3600),
            lane: &PACED_LANE,
        }
    }

    /// The lane, whether or not anything plays: for a run that is one
    /// connection and cannot be paced down or resumed, so that if
    /// playback starts under it, it is already the one connection the
    /// allowance grants.
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
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn the_concurrency_follows_playback() {
        assert_eq!(fragment_concurrency(true), PACED_FRAGMENTS);
        assert_eq!(fragment_concurrency(false), FAST_FRAGMENTS);
    }

    proptest::proptest! {
        #[test]
        fn the_concurrency_is_one_of_the_two_and_paced_is_the_smaller(live in proptest::bool::ANY) {
            let n = fragment_concurrency(live);
            prop_assert!(n >= 1, "a download always moves");
            prop_assert!(n <= FAST_FRAGMENTS, "nothing exceeds the free concurrency");
            prop_assert_eq!(n == PACED_FRAGMENTS, live);
            prop_assert_eq!(n == FAST_FRAGMENTS, !live);
            prop_assert_eq!(rate_limit(live).is_some(), live, "the byte rate is limited exactly when paced");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn the_change_is_seen_at_the_next_poll_and_not_before() {
        let live = std::sync::atomic::AtomicBool::new(false);
        let is_live = || live.load(std::sync::atomic::Ordering::Relaxed);
        let lane = Semaphore::new(1);
        let pacing = Pacing::new(&is_live, Duration::from_secs(2), &lane);
        let waited =
            tokio::time::timeout(Duration::from_secs(5), pacing.until_live_changes(false)).await;
        assert!(waited.is_err(), "nothing changed: the wait goes on");
        live.store(true, std::sync::atomic::Ordering::Relaxed);
        let waited =
            tokio::time::timeout(Duration::from_secs(3), pacing.until_live_changes(false)).await;
        assert!(waited.is_ok(), "the change is seen within a poll");
        assert!(!Pacing::never().is_live());
    }

    #[tokio::test]
    async fn a_turn_on_a_busy_lane_ends_when_playback_stops() {
        let live = std::sync::atomic::AtomicBool::new(true);
        let is_live = || live.load(std::sync::atomic::Ordering::Relaxed);
        let lane = Semaphore::new(1);
        let pacing = Pacing::new(&is_live, Duration::from_millis(10), &lane);
        let held = pacing.paced_turn().await.expect("an open lane is taken");
        let waiting = pacing.paced_turn();
        let waited = tokio::time::timeout(Duration::from_millis(100), waiting).await;
        assert!(waited.is_err(), "a held lane keeps the second waiting");
        live.store(false, std::sync::atomic::Ordering::Relaxed);
        let waited = tokio::time::timeout(Duration::from_millis(500), pacing.paced_turn()).await;
        assert!(
            matches!(waited, Ok(None)),
            "playback stopped: the wait ends without the lane"
        );
        drop(held);
        live.store(true, std::sync::atomic::Ordering::Relaxed);
        assert!(
            pacing.paced_turn().await.is_some(),
            "the lane is free again"
        );
    }
}
