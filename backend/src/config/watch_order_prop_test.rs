//! Properties of the watch-order reading.

use proptest::prelude::*;

use super::{normalize, LINKED, RELEASED};

/// A stored value: either order, or anything at all.
fn stored() -> BoxedStrategy<String> {
    prop_oneof![
        Just(RELEASED.to_string()),
        Just(LINKED.to_string()),
        ".{0,12}",
    ]
    .boxed()
}

proptest! {
    /// Every stored value reads as one of the two orders: "released"
    /// as itself, anything else as "linked".
    #[test]
    fn every_value_reads_as_one_of_the_two_orders(value in stored()) {
        let want = if value == RELEASED { RELEASED } else { LINKED };
        prop_assert_eq!(normalize(&value), want);
    }

    /// Reading is stable: a value read once reads the same again.
    #[test]
    fn reading_twice_is_reading_once(value in stored()) {
        prop_assert_eq!(normalize(normalize(&value)), normalize(&value));
    }
}
