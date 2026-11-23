//! When a lane waits for the gate's turn, and when it hands its batch
//! over.

use std::time::Duration;

use proptest::prelude::*;

use super::{batch_deadline, next_look, waits_for_turn, CLOSED_POLL};
use crate::meta::anilist_gate::Class;

fn class() -> impl Strategy<Value = Class> {
    prop_oneof![
        Just(Class::Interactive),
        Just(Class::Background),
        Just(Class::Retry)
    ]
}

fn turn() -> impl Strategy<Value = Option<Duration>> {
    proptest::option::of((0u64..5_000).prop_map(Duration::from_millis))
}

proptest! {
    #[test]
    fn only_a_background_lookup_held_for_a_turn_waits_for_it(class in class(), turn in turn()) {
        let waits = waits_for_turn(class, turn);
        if class != Class::Background || turn.is_none_or(|t| t.is_zero()) {
            prop_assert!(!waits, "{class:?} {turn:?}");
        }
        if class == Class::Background && turn.is_some_and(|t| !t.is_zero()) {
            prop_assert!(waits, "{turn:?}");
        }
    }

    #[test]
    fn a_held_batch_waits_as_long_as_its_latest_lookup_would_have(
        began in 0u64..10_000,
        joined in 0u64..10_000,
        budget in 0u64..5_000,
    ) {
        let base = tokio::time::Instant::now();
        let started = base + Duration::from_millis(began);
        let last_join = base + Duration::from_millis(joined);
        let budget = Duration::from_millis(budget);
        let deadline = batch_deadline(started, last_join, budget);
        prop_assert!(deadline >= started + budget);
        prop_assert!(deadline >= last_join + budget);
        prop_assert!(deadline == started + budget || deadline == last_join + budget);
    }

    #[test]
    fn a_lane_hands_over_by_the_turn_and_never_past_its_wait(
        class in class(),
        turn in turn(),
        left in (0u64..5_000).prop_map(Duration::from_millis),
    ) {
        match next_look(class, turn, left) {
            Some(nap) => {
                let turn = turn.expect("a turn to wait for");
                prop_assert!(waits_for_turn(class, Some(turn)));
                // Inside the wait, and woken before the turn's last stretch.
                prop_assert!(turn <= left);
                prop_assert!(turn > CLOSED_POLL);
                prop_assert!(nap > Duration::ZERO && nap <= CLOSED_POLL);
                prop_assert!(nap + CLOSED_POLL <= turn);
            }
            None => prop_assert!(
                !waits_for_turn(class, turn)
                    || turn.is_some_and(|t| t <= CLOSED_POLL || t > left)
            ),
        }
    }
}
