//! Property coverage for when a teardown is past its ceiling.

use super::ceiling_passed;
use proptest::prelude::*;
use std::time::Duration;

fn duration() -> impl Strategy<Value = Duration> {
    (0u64..10_000_000).prop_map(Duration::from_micros)
}

proptest! {
    /// A teardown is past its ceiling exactly when the clock it is
    /// measured on has run the ceiling out, whatever the other clock
    /// reads.
    #[test]
    fn the_ceiling_is_read_on_its_own_clock(
        wall in duration(),
        tokio in duration(),
        other in duration(),
        ceiling in duration(),
        on_tokio in proptest::bool::ANY,
    ) {
        let chosen = if on_tokio { tokio } else { wall };
        prop_assert_eq!(ceiling_passed((wall, tokio), on_tokio, ceiling), chosen >= ceiling);
        let other_moved = if on_tokio { (other, tokio) } else { (wall, other) };
        prop_assert_eq!(
            ceiling_passed(other_moved, on_tokio, ceiling),
            ceiling_passed((wall, tokio), on_tokio, ceiling)
        );
    }

    /// Once past its ceiling, a teardown stays past it however much
    /// longer it waits: the wait that gave up is never taken back.
    #[test]
    fn past_the_ceiling_stays_past_it(
        waited in duration(),
        more in duration(),
        ceiling in duration(),
        on_tokio in proptest::bool::ANY,
    ) {
        if ceiling_passed((waited, waited), on_tokio, ceiling) {
            prop_assert!(ceiling_passed((waited + more, waited + more), on_tokio, ceiling));
        }
        prop_assert!(ceiling_passed((ceiling, ceiling), on_tokio, ceiling), "at the ceiling itself");
    }
}
