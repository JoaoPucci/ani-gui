//! Property coverage for the rate limit a caller gets for a request the
//! gate kept back, beside `anilist_gate_answer.rs`.

use std::time::Duration;

use super::super::pace::Class;
use super::not_sent;
use crate::error::AniError;

proptest::proptest! {
    /// Whatever the wait and whoever asked, a request the gate kept back
    /// is AniList refusing it, for the wait rounded up to whole seconds:
    /// the notice of a write to another tracker names AniList as the
    /// one that is busy.
    #[test]
    fn a_request_kept_back_is_refused_by_anilist_for_its_wait_rounded_up(
        millis in 0u64..10_000_000,
        background in proptest::bool::ANY,
    ) {
        let class = if background { Class::Background } else { Class::Interactive };
        let err = not_sent(Duration::from_millis(millis), class);
        let AniError::RateLimited { retry_after_secs, refused_by } = err else {
            panic!("not a rate limit: {err:?}");
        };
        proptest::prop_assert_eq!(retry_after_secs, Some(millis.div_ceil(1000)));
        proptest::prop_assert_eq!(refused_by, Some("anilist"));
    }
}
