//! The pacing module's tests, mounted by `#[path]` so the module's
//! own complexity stays what its production code is.

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

#[test]
fn a_downloads_tracks_are_fetched_one_at_a_time_while_playback_is_live() {
    assert_eq!(sidecar_concurrency(true), 1);
    assert_eq!(
        sidecar_concurrency(false),
        super::super::download::SIDECAR_FETCH_CONCURRENCY
    );
    assert_eq!(
        sidecar_phase_deadline(false),
        super::super::download::SIDECAR_PHASE_DEADLINE
    );
    assert!(
        sidecar_phase_deadline(true) > sidecar_phase_deadline(false),
        "tracks taking their tokens one at a time in turn with the player's need the time"
    );
}

proptest! {
    /// The sidecar selectors choose between their two values by
    /// liveness alone: live is the smaller concurrency and the longer
    /// deadline, and neither value is ever anything else.
    #[test]
    fn the_sidecar_choices_are_one_of_two_and_live_is_the_slower(live in proptest::bool::ANY) {
        let n = sidecar_concurrency(live);
        prop_assert!(n == 1 || n == super::super::download::SIDECAR_FETCH_CONCURRENCY);
        prop_assert_eq!(n == 1, live);
        let d = sidecar_phase_deadline(live);
        prop_assert!(
            d == super::super::download::SIDECAR_PHASE_DEADLINE || d == SIDECAR_PHASE_DEADLINE_LIVE
        );
        prop_assert_eq!(d == SIDECAR_PHASE_DEADLINE_LIVE, live);
        prop_assert!(SIDECAR_PHASE_DEADLINE_LIVE > super::super::download::SIDECAR_PHASE_DEADLINE);
    }
}

/// Playback starting under a free run is seen when the proxy notes the
/// player's first request, not at the next poll: a free run left going
/// for up to a poll fires another full burst beside the player's start.
#[tokio::test(start_paused = true)]
async fn a_note_from_the_proxy_ends_the_wait_at_once() {
    let live = std::sync::atomic::AtomicBool::new(false);
    let is_live = || live.load(std::sync::atomic::Ordering::Relaxed);
    let lane = Semaphore::new(1);
    let noted = tokio::sync::Notify::new();
    let pacing = Pacing::new(&is_live, Duration::from_secs(3600), &lane).woken_by(&noted);
    let waiting = pacing.until_live_changes(false);
    tokio::pin!(waiting);
    assert!(
        tokio::time::timeout(Duration::from_millis(10), waiting.as_mut())
            .await
            .is_err(),
        "nothing changed: the wait goes on"
    );
    live.store(true, std::sync::atomic::Ordering::Relaxed);
    noted.notify_waiters();
    assert!(
        tokio::time::timeout(Duration::from_millis(1), waiting.as_mut())
            .await
            .is_ok(),
        "the note ended the wait at once, not at the next poll"
    );
}

/// A note that changes nothing — playback was already live, or a
/// download's own session — does not end the wait.
#[tokio::test(start_paused = true)]
async fn a_note_that_changes_nothing_does_not_end_the_wait() {
    let is_live = || true;
    let lane = Semaphore::new(1);
    let noted = tokio::sync::Notify::new();
    let pacing = Pacing::new(&is_live, Duration::from_secs(3600), &lane).woken_by(&noted);
    let waiting = pacing.until_live_changes(true);
    tokio::pin!(waiting);
    let _ = tokio::time::timeout(Duration::from_millis(10), waiting.as_mut()).await;
    noted.notify_waiters();
    assert!(
        tokio::time::timeout(Duration::from_millis(10), waiting.as_mut())
            .await
            .is_err(),
        "playback is as it was: the wait goes on"
    );
}

/// Playback that started before the wait began — its note sent before
/// anything was listening, after the caller last looked — is seen at
/// once, not at the next poll.
#[tokio::test(start_paused = true)]
async fn a_change_before_the_wait_began_is_seen_at_once() {
    let live = std::sync::atomic::AtomicBool::new(false);
    let is_live = || live.load(std::sync::atomic::Ordering::Relaxed);
    let lane = Semaphore::new(1);
    let noted = tokio::sync::Notify::new();
    let pacing = Pacing::new(&is_live, Duration::from_secs(3600), &lane).woken_by(&noted);
    live.store(true, std::sync::atomic::Ordering::Relaxed);
    noted.notify_waiters();
    assert!(
        tokio::time::timeout(Duration::from_millis(1), pacing.until_live_changes(false))
            .await
            .is_ok(),
        "the change was seen at once"
    );
}

/// The relay holds a fragment's request until its turn at the host, and
/// while the player's need with a quarter to spare reaches the refill
/// that turn comes only when the player stops asking — at the end of a
/// fill that can last minutes while the need is under the refill, and
/// when playback stops once it is over it. A tool that gave up on the wait would fail
/// the run, and a failed relayed run is followed by a fallback that
/// reads the host directly: the request the relay exists to keep from
/// it. The tool waits as long as a paced run may last.
#[test]
fn a_relayed_run_waits_its_turn_as_long_as_a_paced_run_may_last() {
    assert!(Duration::from_secs(u64::from(RELAYED_SOCKET_TIMEOUT_S)) >= PACED_RUN_CEILING);
}
