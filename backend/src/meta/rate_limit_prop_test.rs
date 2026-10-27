//! Property coverage for the `Retry-After` reading, beside the pure
//! functions in `rate_limit.rs`.

use super::{parse_retry_after, rate_limited};
use crate::error::AniError;

proptest::proptest! {
    /// Every delay-seconds value a tracker can send reads back as
    /// itself, whatever whitespace the header carries around it.
    #[test]
    fn delay_seconds_read_back_as_themselves(
        secs in proptest::num::u64::ANY,
        before in "[ \t]{0,3}",
        after in "[ \t]{0,3}",
    ) {
        let header = format!("{before}{secs}{after}");
        proptest::prop_assert_eq!(parse_retry_after(Some(&header)), Some(secs));
    }

    /// Anything that is not a run of digits — the HTTP-date form, a
    /// negative, a fraction, a word — is no advertised wait. A wait
    /// invented from a header that did not say one would be honoured
    /// by the renderer as if the tracker had asked for it.
    #[test]
    fn anything_but_digits_is_no_wait(value in "[^0-9]+|-[0-9]+|[0-9]+\\.[0-9]+|[0-9]+ [a-zA-Z]+") {
        proptest::prop_assert_eq!(parse_retry_after(Some(&value)), None);
    }

    /// A header the tracker did not send is no wait.
    #[test]
    fn an_absent_header_is_no_wait(_any in proptest::num::u8::ANY) {
        proptest::prop_assert_eq!(parse_retry_after(None), None);
    }
}

/// A tracker's 429 as the transport sees it, `Retry-After` and all.
fn refusal(retry_after: Option<&str>) -> reqwest::Response {
    let mut builder = axum::http::Response::builder().status(429);
    if let Some(v) = retry_after {
        builder = builder.header("retry-after", v);
    }
    reqwest::Response::from(builder.body(Vec::<u8>::new()).expect("response"))
}

proptest::proptest! {
    /// A 429 is the tracker that answered it refusing, and says so: a
    /// write to one tracker refused by another names the one that
    /// refused, with the wait it advertised.
    #[test]
    fn a_429_names_the_tracker_that_answered_it(
        mal in proptest::bool::ANY,
        secs in proptest::option::of(0u64..100_000),
    ) {
        let provider = if mal { "mal" } else { "anilist" };
        let header = secs.map(|s| s.to_string());
        let err = rate_limited(provider, &refusal(header.as_deref()));
        let AniError::RateLimited { retry_after_secs, refused_by } = err else {
            panic!("not a rate limit: {err:?}");
        };
        proptest::prop_assert_eq!(retry_after_secs, secs);
        proptest::prop_assert_eq!(refused_by, Some(provider));
    }
}
