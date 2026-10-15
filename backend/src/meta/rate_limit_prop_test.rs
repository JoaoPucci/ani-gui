//! Property coverage for the `Retry-After` reading and the bounded
//! retry's wait, beside the pure functions in `rate_limit.rs`.

use std::time::Duration;

use super::{parse_retry_after, retry_wait_within};

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

    /// The bounded retry never sleeps past its cap: an advertised wait
    /// within it is honoured exactly, one past it is not retried, and
    /// no advertised wait sleeps the default.
    #[test]
    fn a_retry_sleeps_within_its_cap_or_not_at_all(
        advertised in proptest::option::of(0u64..600),
        cap_secs in 0u64..120,
        default_secs in 0u64..120,
    ) {
        let cap = Duration::from_secs(cap_secs);
        let default = Duration::from_secs(default_secs);
        let wait = retry_wait_within(advertised, cap, default);
        match advertised {
            None => proptest::prop_assert_eq!(wait, Some(default)),
            Some(secs) if secs <= cap_secs => {
                proptest::prop_assert_eq!(wait, Some(Duration::from_secs(secs)));
            }
            Some(_) => proptest::prop_assert_eq!(wait, None),
        }
        if let Some(w) = wait {
            if advertised.is_some() {
                proptest::prop_assert!(w <= cap, "an advertised wait is never stretched or cut: {w:?} > {cap:?}");
            }
        }
    }
}
