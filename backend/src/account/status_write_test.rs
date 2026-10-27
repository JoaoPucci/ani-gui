//! What a write sends MAL for each status.

use super::ListStatus;
use proptest::prelude::*;

const ALL: [ListStatus; 6] = [
    ListStatus::Planning,
    ListStatus::Watching,
    ListStatus::Completed,
    ListStatus::Paused,
    ListStatus::Dropped,
    ListStatus::Rewatching,
];

proptest! {
    /// No write sends MAL a rewatch: every status but Rewatching sends
    /// the pair MAL reads back as that same status, and Rewatching sends
    /// nothing.
    #[test]
    fn a_write_never_sends_mal_a_rewatch(status in proptest::sample::select(ALL.to_vec())) {
        match status.to_mal_write() {
            None => prop_assert_eq!(status, ListStatus::Rewatching),
            Some((mal, rewatching)) => {
                prop_assert!(!rewatching, "{:?}", status);
                prop_assert_eq!(ListStatus::from_mal(mal, rewatching), Some(status));
            }
        }
    }
}
