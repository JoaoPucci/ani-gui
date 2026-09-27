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
