//! The session table's record of playback: when the proxy last
//! served media for any session, and whether that makes playback
//! live now. Mounted by `#[path]` so the table's inline tests stay
//! under the per-file complexity bar.

use super::*;
use std::time::{Duration, Instant};

const WINDOW: Duration = Duration::from_secs(30);

#[test]
fn a_fresh_table_has_no_live_playback() {
    let table = SessionTable::new();
    assert!(!table.playback_live_at(Instant::now(), WINDOW));
}

#[test]
fn a_media_fetch_makes_playback_live_for_the_window_and_no_longer() {
    // The player fetches a segment every few seconds while it plays
    // and stops once its buffer is full or it is paused, so a window
    // of silence is the player standing still, and the download it
    // was holding back may run free again.
    let table = SessionTable::new();
    let t0 = Instant::now();
    table.note_media_fetch_at(t0);
    assert!(table.playback_live_at(t0, WINDOW));
    assert!(table.playback_live_at(t0 + Duration::from_secs(29), WINDOW));
    assert!(
        table.playback_live_at(t0 + WINDOW, WINDOW),
        "the window's edge is inside it"
    );
    assert!(!table.playback_live_at(t0 + WINDOW + Duration::from_millis(1), WINDOW));
}

#[test]
fn the_latest_fetch_is_the_one_that_counts() {
    let table = SessionTable::new();
    let t0 = Instant::now();
    table.note_media_fetch_at(t0);
    table.note_media_fetch_at(t0 + Duration::from_secs(40));
    assert!(table.playback_live_at(t0 + Duration::from_secs(60), WINDOW));
}

#[test]
fn clones_share_the_record() {
    // The app state and the proxy state each hold a clone of one
    // table; the proxy notes the fetch, the download command asks.
    let proxy_side = SessionTable::new();
    let app_side = proxy_side.clone();
    let t0 = Instant::now();
    proxy_side.note_media_fetch_at(t0);
    assert!(app_side.playback_live_at(t0 + Duration::from_secs(1), WINDOW));
}

mod props {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// Playback is live exactly when a media fetch happened and
        /// no more than the window ago; nothing fetched is never live,
        /// whatever the window.
        #[test]
        fn live_exactly_within_the_window_of_the_last_fetch(
            fetched in prop::option::of(0u64..120_000),
            elapsed in 0u64..240_000,
            window_ms in 0u64..120_000,
        ) {
            let base = Instant::now();
            let last = fetched.map(|ms| base + Duration::from_millis(ms));
            let now = base + Duration::from_millis(fetched.unwrap_or(0) + elapsed);
            let window = Duration::from_millis(window_ms);
            let expected = last.is_some() && elapsed <= window_ms;
            prop_assert_eq!(live_at(last, now, window), expected);
        }
    }
}
