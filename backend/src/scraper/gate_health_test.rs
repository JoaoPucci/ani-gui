//! The gate notes every outcome it records through `record_from` for
//! the notification center's outage rule.

use super::*;

fn fail(gate: &ScraperGate, prio: ScrapePriority) {
    gate.record_from(prio, ScrapeOutcome::Failure, Instant::now());
}

#[test]
fn a_long_run_a_user_saw_reads_as_not_answering() {
    let gate = ScraperGate::new();
    fail(&gate, ScrapePriority::Background);
    fail(&gate, ScrapePriority::Background);
    fail(&gate, ScrapePriority::Interactive);
    assert_eq!(gate.health(), Health::Answering);
    fail(&gate, ScrapePriority::Background);
    assert!(matches!(
        gate.health(),
        Health::NotAnswering { failures: 4, .. }
    ));
    // The breaker saw the same outcomes.
    assert!(gate.is_open());
}

#[test]
fn background_failures_alone_read_as_answering() {
    let gate = ScraperGate::new();
    for _ in 0..10 {
        fail(&gate, ScrapePriority::Background);
    }
    assert_eq!(gate.health(), Health::Answering);
}

#[test]
fn a_success_after_an_outage_reads_as_answering_again() {
    let gate = ScraperGate::new();
    for _ in 0..4 {
        fail(&gate, ScrapePriority::Interactive);
    }
    gate.record_from(
        ScrapePriority::Interactive,
        ScrapeOutcome::Success,
        Instant::now(),
    );
    assert!(matches!(gate.health(), Health::AnsweringAgain { .. }));
    assert!(!gate.is_open());
}
