//! Property coverage for the `Retry-After` reading, beside the pure
//! functions in `rate_limit.rs`.

use super::parse_retry_after;

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
