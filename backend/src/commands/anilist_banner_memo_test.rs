use super::*;

// `remember`, `remembered` and `attach` are exercised end to end in
// kitsu_anilist_banner_test.rs, through the trending and detail reads.
// These pin the one pure piece: the row's key.

proptest::proptest! {
    /// The key is the fixed prefix and the Kitsu id, nothing else, so a
    /// row is found by its id alone.
    #[test]
    fn the_key_is_the_prefix_and_the_id(id in "[0-9]{1,8}") {
        proptest::prop_assert_eq!(key(&id), format!("anilist:banner:v1:k{id}"));
    }

    /// Two ids never share a row.
    #[test]
    fn distinct_ids_have_distinct_keys(a in "[0-9]{1,8}", b in "[0-9]{1,8}") {
        proptest::prop_assume!(a != b);
        proptest::prop_assert_ne!(key(&a), key(&b));
    }
}
