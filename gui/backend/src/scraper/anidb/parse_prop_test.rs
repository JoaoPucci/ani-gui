//! Property coverage for the query encoder.
//!
//! Its own file rather than an append to `anidb_test.rs`, matching
//! the other property modules on this branch: appended blocks collide
//! with every later addition to the same file.

use crate::scraper::anidb::parse::encode_query;

proptest::proptest! {
    /// Form-decoding the encoded output recovers the input exactly,
    /// over arbitrary unicode. The naive space swap this function
    /// replaced broke on `;` and friends — a failure class a
    /// fixed-example table cannot sweep.
    #[test]
    fn form_decoding_recovers_the_original_query(q in ".*") {
        let encoded = encode_query(&q);
        let decoded: String = url::form_urlencoded::parse(format!("q={encoded}").as_bytes())
            .next()
            .map(|(_, v)| v.into_owned())
            .unwrap_or_default();
        proptest::prop_assert_eq!(decoded, q);
    }

    /// The output stays inside the form-urlencoding alphabet, so it
    /// splices into a query string without further quoting.
    #[test]
    fn encoded_output_is_query_safe(q in ".*") {
        let encoded = encode_query(&q);
        proptest::prop_assert!(encoded
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'%' | b'+' | b'-' | b'.' | b'_' | b'*')));
    }
}
