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

/// An advertised rate-limit window is the provider refusing as much
/// as an open breaker is — a walk that asks it now is told to come
/// back later — so the read that says whether the provider can be
/// relied on right now says so for both, and for neither once the
/// window has elapsed.
#[tokio::test(start_paused = true)]
async fn an_advertised_pause_is_refusing_while_the_breaker_stays_closed() {
    let gate = ScraperGate::new();
    assert!(!gate.is_refusing(), "a fresh gate refuses nothing");
    gate.record(
        ScrapeOutcome::RateLimited {
            retry_after: Some(Duration::from_secs(9)),
        },
        Instant::now(),
    );
    assert!(
        !gate.is_open(),
        "a rate limit opens a pause, not the breaker"
    );
    assert!(gate.is_refusing(), "and the pause is the provider refusing");
    tokio::time::advance(Duration::from_secs(9) + Duration::from_millis(1)).await;
    assert!(!gate.is_refusing(), "past the window the provider is back");
}

#[tokio::test(start_paused = true)]
async fn a_fresh_success_ends_the_pause_and_an_open_breaker_refuses_too() {
    let gate = ScraperGate::new();
    gate.record(
        ScrapeOutcome::RateLimited {
            retry_after: Some(Duration::from_secs(30)),
        },
        Instant::now(),
    );
    tokio::time::advance(Duration::from_secs(1)).await;
    gate.record(ScrapeOutcome::Success, Instant::now());
    assert!(
        !gate.is_refusing(),
        "a success that started after the pause opened clears it"
    );
    tokio::time::advance(Duration::from_secs(1)).await;
    for _ in 0..FAILURE_THRESHOLD {
        gate.record(ScrapeOutcome::Failure, Instant::now());
    }
    assert!(gate.is_open());
    assert!(
        gate.is_refusing(),
        "an open breaker is the provider refusing"
    );
}
