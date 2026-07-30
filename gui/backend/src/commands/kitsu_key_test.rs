//! Property coverage for the anime detail-cache key.
//!
//! Its own file rather than an append to `kitsu.rs`'s inline test
//! module, for the same reason the mapping key's property lives apart:
//! appended blocks collide with every later addition.

use super::anime_detail_key;

proptest::proptest! {
    /// The key every detail row is written and read under. Two ids
    /// sharing one would serve a card under another title's name; an
    /// id dropped from the key would collapse the whole cache onto a
    /// single row. Both are properties of the key rather than of any
    /// particular id.
    #[test]
    fn keys_are_distinct_and_prefixed(a: String, b: String) {
        let ka = anime_detail_key(&a);
        proptest::prop_assert!(ka.starts_with("kitsu:v3:anime:"));
        proptest::prop_assert_eq!(ka.trim_start_matches("kitsu:v3:anime:"), a.as_str());
        proptest::prop_assert_eq!(ka == anime_detail_key(&b), a == b);
    }
}
