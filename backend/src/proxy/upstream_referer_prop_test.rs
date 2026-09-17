//! Property coverage for the header a session's stored referer becomes.

use super::referer_header;
use proptest::prelude::*;

proptest! {
    /// A referer a header value can carry reaches the wire byte for
    /// byte, and the empty string — every other reader's spelling of
    /// "this stream's CDN asks for none" — is no header at all.
    #[test]
    fn a_referer_a_header_can_carry_arrives_verbatim(referer in "[\\x21-\\x7e]{1,80}") {
        prop_assert_eq!(
            referer_header(&referer).map(|v| v.as_bytes().to_vec()),
            Some(referer.clone().into_bytes())
        );
        prop_assert!(referer_header("").is_none());
    }

    /// A control character cannot appear in a header value. Whatever
    /// surrounds it, the answer is no header rather than some
    /// substitute origin: naming a site the app never fetched from is
    /// worse than naming none.
    #[test]
    fn a_referer_no_header_can_carry_becomes_no_header(
        head in "[\\x21-\\x7e]{0,20}",
        control in proptest::sample::select(vec!['\n', '\r', '\u{0}', '\u{7}', '\u{7f}']),
        tail in "[\\x21-\\x7e]{0,20}",
    ) {
        let referer = format!("{head}{control}{tail}");
        prop_assert!(referer_header(&referer).is_none());
    }
}
