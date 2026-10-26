//! Property coverage for the listing row's cache key: one MAL id, one
//! row, under the row's own versioned prefix.
//!
//! Every strategy function returns a `BoxedStrategy`. Composed without
//! boxing, the nested combinator types grow into symbol names tens of
//! thousands of characters long, and the Windows test build's debug info
//! hits the linker's PDB limit (LNK4319). Box each new strategy too.

use super::*;
use proptest::prelude::*;

const PREFIX: &str = "anilist:media-ids-recording-mal:v1:";

/// Two MAL ids, distinct: anywhere in the range, neighbours, and one
/// the other's digits with more after them, where a key that dropped
/// or padded digits would collide.
fn distinct_ids() -> BoxedStrategy<(u32, u32)> {
    prop_oneof![
        (any::<u32>(), any::<u32>()),
        any::<u32>().prop_map(|a| (a, a.wrapping_add(1))),
        (0u32..=429_496_728, 0u32..=9).prop_map(|(a, d)| (a, a * 10 + d)),
    ]
    .prop_filter("the ids differ", |(a, b)| a != b)
    .boxed()
}

proptest! {
    /// Distinct MAL ids keep distinct rows, each under the stable
    /// prefix with the id after it in plain decimal.
    #[test]
    fn distinct_mal_ids_keep_distinct_rows_under_the_prefix((a, b) in distinct_ids()) {
        let (ka, kb) = (recording_key(a), recording_key(b));
        prop_assert_ne!(&ka, &kb);
        for (id, key) in [(a, &ka), (b, &kb)] {
            let decimal = id.to_string();
            prop_assert_eq!(key.strip_prefix(PREFIX), Some(decimal.as_str()));
        }
    }
}
