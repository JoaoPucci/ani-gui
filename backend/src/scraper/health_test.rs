//! The outage rule: slow to speak, quick to say it is over.

use super::{Health, HealthRun, Observation, OUTAGE_FAILURES};

const USER: Observation = Observation::Failure { interactive: true };
const PROBE: Observation = Observation::Failure { interactive: false };

fn run(steps: &[(Observation, u64)]) -> HealthRun {
    steps
        .iter()
        .fold(HealthRun::default(), |r, (o, t)| r.observe(*o, *t))
}

#[test]
fn the_breaker_opening_and_its_next_attempt_failing_is_four() {
    assert_eq!(OUTAGE_FAILURES, 4);
}

#[test]
fn a_fresh_run_is_answering() {
    assert_eq!(HealthRun::default().health(), Health::Answering);
}

#[test]
fn a_few_failures_say_nothing() {
    let r = run(&[(USER, 1), (USER, 2), (USER, 3)]);
    assert_eq!(r.health(), Health::Answering);
}

#[test]
fn enough_failures_with_a_users_among_them_say_the_source_is_not_answering() {
    let r = run(&[(PROBE, 10), (PROBE, 20), (USER, 30), (PROBE, 40)]);
    assert_eq!(
        r.health(),
        Health::NotAnswering {
            since_ms: 10,
            failures: 4,
            last_failure_ms: 40
        }
    );
    let r = r.observe(USER, 55);
    assert_eq!(
        r.health(),
        Health::NotAnswering {
            since_ms: 10,
            failures: 5,
            last_failure_ms: 55
        }
    );
}

#[test]
fn failures_only_background_probes_saw_say_nothing() {
    let steps: Vec<_> = (0..20).map(|t| (PROBE, t)).collect();
    assert_eq!(run(&steps).health(), Health::Answering);
}

#[test]
fn a_success_ends_the_outage_and_says_so() {
    let r = run(&[
        (USER, 10),
        (USER, 20),
        (USER, 30),
        (USER, 40),
        (Observation::Success, 90),
    ]);
    assert_eq!(
        r.health(),
        Health::AnsweringAgain {
            since_ms: 10,
            recovered_ms: 90
        }
    );
    // Further successes leave it answering again.
    let r = r.observe(Observation::Success, 100);
    assert_eq!(
        r.health(),
        Health::AnsweringAgain {
            since_ms: 10,
            recovered_ms: 90
        }
    );
}

#[test]
fn a_success_before_the_rule_spoke_resets_quietly() {
    let r = run(&[
        (USER, 1),
        (USER, 2),
        (Observation::Success, 3),
        (USER, 4),
        (USER, 5),
    ]);
    assert_eq!(r.health(), Health::Answering);
}

#[test]
fn a_new_failure_after_recovery_starts_a_new_run() {
    let r = run(&[
        (USER, 1),
        (USER, 2),
        (USER, 3),
        (USER, 4),
        (Observation::Success, 5),
    ]);
    let r = r.observe(PROBE, 6);
    assert_eq!(r.health(), Health::Answering);
    let r = r.observe(USER, 7).observe(PROBE, 8).observe(PROBE, 9);
    assert_eq!(
        r.health(),
        Health::NotAnswering {
            since_ms: 6,
            failures: 4,
            last_failure_ms: 9
        }
    );
}

#[test]
fn a_rate_limit_neither_counts_nor_ends_a_run() {
    let limited = Observation::RateLimited;
    let r = run(&[(USER, 1), (limited, 2), (USER, 3), (limited, 4), (USER, 5)]);
    assert_eq!(r.health(), Health::Answering);
    let r = r.observe(USER, 6).observe(limited, 7);
    assert!(matches!(
        r.health(),
        Health::NotAnswering { failures: 4, .. }
    ));
}

#[test]
fn a_clock_set_back_never_reports_recovery_before_the_outage() {
    let r = run(&[
        (USER, 100),
        (USER, 100),
        (USER, 100),
        (USER, 100),
        (Observation::Success, 5),
    ]);
    assert_eq!(
        r.health(),
        Health::AnsweringAgain {
            since_ms: 100,
            recovered_ms: 100
        }
    );
}

#[test]
fn the_wire_shape_is_what_the_renderer_reads() {
    let json = |h: Health| serde_json::to_value(h).expect("serialise");
    assert_eq!(
        json(Health::Answering),
        serde_json::json!({ "state": "answering" })
    );
    assert_eq!(
        json(Health::NotAnswering {
            since_ms: 1,
            failures: 4,
            last_failure_ms: 2
        }),
        serde_json::json!({
            "state": "not_answering",
            "since_ms": 1,
            "failures": 4,
            "last_failure_ms": 2
        })
    );
    assert_eq!(
        json(Health::AnsweringAgain {
            since_ms: 1,
            recovered_ms: 3
        }),
        serde_json::json!({ "state": "answering_again", "since_ms": 1, "recovered_ms": 3 })
    );
}

#[test]
fn a_gate_outcome_maps_to_its_observation() {
    use crate::scraper::outcome::ScrapeOutcome;
    assert_eq!(
        Observation::of(ScrapeOutcome::Success, true),
        Observation::Success
    );
    assert_eq!(Observation::of(ScrapeOutcome::Failure, true), USER);
    assert_eq!(Observation::of(ScrapeOutcome::Failure, false), PROBE);
    assert_eq!(
        Observation::of(ScrapeOutcome::RateLimited { retry_after: None }, true),
        Observation::RateLimited
    );
}
