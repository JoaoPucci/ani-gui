//! Whether the breaker is open, as a question the gate answers
//! without changing state.

use super::*;

#[tokio::test(start_paused = true)]
async fn the_breaker_reports_open_after_the_threshold_and_closed_after_the_cooldown() {
    let gate = ScraperGate::new();
    assert!(!gate.is_open(), "a fresh gate is closed");
    for _ in 0..FAILURE_THRESHOLD {
        gate.record(ScrapeOutcome::Failure, Instant::now());
    }
    assert!(gate.is_open(), "the threshold opens it");
    tokio::time::advance(BREAKER_COOLDOWN + Duration::from_millis(1)).await;
    assert!(
        !gate.is_open(),
        "past the cooldown the breaker is no longer open — the next admit runs the trial"
    );
}

#[tokio::test(start_paused = true)]
async fn a_success_closes_the_breaker() {
    let gate = ScraperGate::new();
    for _ in 0..FAILURE_THRESHOLD {
        gate.record(ScrapeOutcome::Failure, Instant::now());
    }
    assert!(gate.is_open());
    tokio::time::advance(Duration::from_secs(1)).await;
    gate.record(ScrapeOutcome::Success, Instant::now());
    assert!(
        !gate.is_open(),
        "a success recorded after the opening closes it"
    );
}
