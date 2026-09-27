//! How a download yields to live playback. yt-dlp fetches sixteen
//! fragments at once, and against the host the player is streaming
//! from that burst starves the player's next segment within seconds.
//! While the proxy has served media recently — playback is live — a
//! download runs two fragments wide instead. yt-dlp cannot change its
//! concurrency mid-run, so when playback starts or stops under a
//! running download the supervisor takes the tool down and starts it
//! again on the same output, which yt-dlp resumes from the fragments
//! it already has ([`super::download::spawn_download_tool_paced`]).

use std::time::Duration;

/// Fragments in flight when nothing is playing: v5's `-N 16`.
pub(crate) const FAST_FRAGMENTS: u32 = 16;

/// Fragments in flight while playback is live. Two keeps the transfer
/// moving while leaving the player's single fetch most of the host.
pub(crate) const PACED_FRAGMENTS: u32 = 2;

/// How long after the last media fetch playback counts as live: a
/// playing player fetches a segment every few seconds, and a paused
/// one with a full buffer fetches nothing, so a window of silence is
/// the player standing still.
pub(crate) const PLAYBACK_LIVE_WINDOW: Duration = Duration::from_secs(30);

/// How often a running transfer asks whether playback changed.
pub(crate) const PACING_POLL: Duration = Duration::from_secs(2);

/// The fragment concurrency for the current state of playback.
#[must_use]
pub(crate) fn fragment_concurrency(playback_live: bool) -> u32 {
    if playback_live {
        PACED_FRAGMENTS
    } else {
        FAST_FRAGMENTS
    }
}

/// The transfer's view of playback: a question it can ask at any
/// moment, and how often it asks while a tool runs. Production asks
/// the session table the proxy shares with the download command; the
/// tests hand it a flag.
pub(crate) struct Pacing<'a> {
    is_live: &'a (dyn Fn() -> bool + Sync),
    poll: Duration,
}

impl<'a> Pacing<'a> {
    pub(crate) fn new(is_live: &'a (dyn Fn() -> bool + Sync), poll: Duration) -> Self {
        Self { is_live, poll }
    }

    /// A transfer nothing paces: playback is never live. The seam the
    /// unpaced entry point and its tests run on.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn never() -> Pacing<'static> {
        Pacing {
            is_live: &|| false,
            poll: Duration::from_secs(3600),
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

    #[test]
    fn the_concurrency_follows_playback() {
        assert_eq!(fragment_concurrency(true), PACED_FRAGMENTS);
        assert_eq!(fragment_concurrency(false), FAST_FRAGMENTS);
    }

    #[tokio::test(start_paused = true)]
    async fn the_change_is_seen_at_the_next_poll_and_not_before() {
        let live = std::sync::atomic::AtomicBool::new(false);
        let is_live = || live.load(std::sync::atomic::Ordering::Relaxed);
        let pacing = Pacing::new(&is_live, Duration::from_secs(2));
        let waited =
            tokio::time::timeout(Duration::from_secs(5), pacing.until_live_changes(false)).await;
        assert!(waited.is_err(), "nothing changed: the wait goes on");
        live.store(true, std::sync::atomic::Ordering::Relaxed);
        let waited =
            tokio::time::timeout(Duration::from_secs(3), pacing.until_live_changes(false)).await;
        assert!(waited.is_ok(), "the change is seen within a poll");
        assert!(!Pacing::never().is_live());
    }
}
