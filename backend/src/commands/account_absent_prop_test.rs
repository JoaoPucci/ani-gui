//! Which refusals a write confirms with a read: exactly the 4xx
//! statuses, of which absence never gets this far.

use super::refused_as_bad_request;
use crate::error::AniError;

proptest::proptest! {
    #[test]
    fn a_bad_request_is_exactly_a_4xx_status(status in proptest::num::u16::ANY) {
        proptest::prop_assert_eq!(
            refused_as_bad_request(&AniError::Upstream { status }),
            (400..500).contains(&status)
        );
    }
}

#[test]
fn a_refusal_of_this_client_is_not_a_bad_request() {
    for err in [
        AniError::InvalidToken,
        AniError::Network,
        AniError::Timeout,
        AniError::RateLimited {
            retry_after_secs: None,
            refused_by: None,
        },
    ] {
        assert!(!refused_as_bad_request(&err), "{err:?}");
    }
}
