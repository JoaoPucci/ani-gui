//! Over any sequence of outcomes: the rule never speaks without a
//! user's failure in the run or before the run is long enough, a
//! success always ends an outage, and nothing panics.

use proptest::prelude::*;

use super::{Health, HealthRun, Observation, OUTAGE_FAILURES};

fn observation() -> impl Strategy<Value = Observation> {
    prop_oneof![
        Just(Observation::Success),
        Just(Observation::RateLimited),
        any::<bool>().prop_map(|interactive| Observation::Failure { interactive }),
    ]
}

proptest! {
    #[test]
    fn the_rule_speaks_only_for_a_long_run_a_user_saw(
        steps in prop::collection::vec((observation(), any::<u64>()), 0..64)
    ) {
        let mut r = HealthRun::default();
        // The run as the model counts it: failures since the last
        // success, and whether a user's was among them.
        let (mut run, mut user) = (0u32, false);
        for (obs, t) in steps {
            r = r.observe(obs, t);
            match obs {
                Observation::Success => { run = 0; user = false; }
                Observation::Failure { interactive } => { run += 1; user |= interactive; }
                Observation::RateLimited => {}
            }
            match r.health() {
                Health::NotAnswering { failures, since_ms, last_failure_ms } => {
                    prop_assert!(run >= OUTAGE_FAILURES && user);
                    prop_assert_eq!(failures, run);
                    let _ = (since_ms, last_failure_ms);
                }
                _ => prop_assert!(!(run >= OUTAGE_FAILURES && user)),
            }
            if obs == Observation::Success {
                let over = !matches!(r.health(), Health::NotAnswering { .. });
                prop_assert!(over, "a success left the outage open");
            }
            if let Health::AnsweringAgain { since_ms, recovered_ms } = r.health() {
                prop_assert!(recovered_ms >= since_ms);
            }
        }
    }
}
