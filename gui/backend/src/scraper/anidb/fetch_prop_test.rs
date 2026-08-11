//! Property coverage for the transport's pure helpers.
//!
//! Its own file rather than an append to `anidb_test.rs`, matching
//! the other property modules on this branch.

use super::candidate_names;

proptest::proptest! {
    /// Expansion is exactly the suffix table applied in order: one
    /// candidate per suffix, each the name with that suffix appended,
    /// none invented and none dropped — an empty table yields no
    /// candidates at all.
    #[test]
    fn expansion_is_the_suffix_table_applied_in_order(
        name in ".*",
        suffixes in proptest::collection::vec(".{0,6}", 0..6),
    ) {
        let refs: Vec<&str> = suffixes.iter().map(String::as_str).collect();
        let expanded = candidate_names(&name, &refs);
        proptest::prop_assert_eq!(expanded.len(), suffixes.len());
        for (got, suffix) in expanded.iter().zip(&suffixes) {
            proptest::prop_assert_eq!(got, &format!("{name}{suffix}"));
        }
    }
}
