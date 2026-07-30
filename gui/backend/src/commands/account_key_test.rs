//! Property coverage for the mapping cache key.
//!
//! Its own file rather than an append to `account_test.rs`: that file
//! grows by appending, so anything landing at its end collides with
//! every later addition.

use super::mal_map_key;

proptest::proptest! {
    /// The key a resolved mapping is stored under. A collision would
    /// hand one title another title's answer — silently, and for
    /// thirty days if the borrowed answer was positive. Fixed ids in
    /// an integration case cannot establish that; distinctness is a
    /// property of the key.
    #[test]
    fn keys_are_distinct_and_carry_their_id(a: u32, b: u32) {
        let ka = mal_map_key(a);
        proptest::prop_assert!(ka.starts_with("kitsu:mal-map:v1:"));
        proptest::prop_assert_eq!(
            ka.trim_start_matches("kitsu:mal-map:v1:").parse::<u32>().ok(),
            Some(a),
        );
        proptest::prop_assert_eq!(ka == mal_map_key(b), a == b);
    }
}
