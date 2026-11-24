//! Which failures a write confirms with a read: exactly the 4xx
//! statuses that reject the request (of which absence never gets this
//! far) and an answer that is no entry. A 408, 425 or 429 says the
//! tracker did not take the write this time, nothing about the entry.

use super::needs_confirming;
use crate::error::AniError;

proptest::proptest! {
    #[test]
    fn a_status_is_confirmed_exactly_when_it_is_a_rejecting_4xx(
        status in proptest::prop_oneof![
            proptest::num::u16::ANY,
            proptest::sample::select(vec![408u16, 425, 429]),
        ],
    ) {
        proptest::prop_assert_eq!(
            needs_confirming(&AniError::Upstream { status }),
            (400..500).contains(&status) && !matches!(status, 408 | 425 | 429)
        );
        proptest::prop_assert_eq!(
            needs_confirming(&AniError::from_tracker_status(status)),
            (400..500).contains(&status) && !matches!(status, 408 | 425 | 429)
        );
    }
}

#[test]
fn an_answer_with_no_entry_is_confirmed() {
    assert!(needs_confirming(&AniError::ParseFailed {
        detail: String::new()
    }));
}

#[test]
fn a_refusal_of_this_client_is_not_confirmed() {
    for err in [
        AniError::InvalidToken,
        AniError::Network,
        AniError::Timeout,
        AniError::RateLimited {
            retry_after_secs: None,
            refused_by: None,
        },
    ] {
        assert!(!needs_confirming(&err), "{err:?}");
    }
}
