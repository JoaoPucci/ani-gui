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

/// Past the cooldown the breaker is half-open: it refuses nobody, and
/// one trial is let through, but nothing has been seen answering.
/// Refusing and recovered are two different questions, and a verdict
/// that has to be stood behind asks the second: a provider whose
/// outage merely outlasted the cooldown has recovered nothing until a
/// success closes the breaker.
#[tokio::test(start_paused = true)]
async fn a_cooled_breaker_refuses_nobody_but_is_recovered_only_by_a_success() {
    let gate = ScraperGate::new();
    assert!(
        gate.is_recovered(),
        "a fresh gate has nothing to recover from"
    );
    for _ in 0..FAILURE_THRESHOLD {
        gate.record(ScrapeOutcome::Failure, Instant::now());
    }
    assert!(!gate.is_recovered(), "an open breaker is not recovered");
    tokio::time::advance(BREAKER_COOLDOWN + Duration::from_millis(1)).await;
    assert!(
        !gate.is_refusing(),
        "past the cooldown the breaker refuses nobody"
    );
    assert!(
        !gate.is_recovered(),
        "and is half-open: nothing has answered, so nothing has recovered"
    );
    gate.record(ScrapeOutcome::Success, Instant::now());
    assert!(
        gate.is_recovered(),
        "a success closes it, and that is recovery"
    );
}

/// An advertised pause is the upstream naming the moment to come
/// back, and admission clears it on the clock, so its window's end is
/// recovery — unlike the breaker's cooldown, which only opens a trial.
#[tokio::test(start_paused = true)]
async fn an_advertised_pause_is_recovered_at_its_windows_end() {
    let gate = ScraperGate::new();
    gate.record(
        ScrapeOutcome::RateLimited {
            retry_after: Some(Duration::from_secs(9)),
        },
        Instant::now(),
    );
    assert!(!gate.is_recovered(), "a running pause is not recovered");
    tokio::time::advance(Duration::from_secs(9) + Duration::from_millis(1)).await;
    assert!(
        gate.is_recovered(),
        "the window the upstream named has passed"
    );
}

mod recovery_props {
    use super::*;
    use proptest::prelude::*;

    /// Offsets in milliseconds around a base instant: a deadline in
    /// the past, at the instant, or in the future, or none at all.
    fn deadline() -> impl Strategy<Value = Option<i64>> {
        proptest::option::of(-120_000i64..120_000)
    }

    fn at(base: Instant, offset: Option<i64>) -> Option<Instant> {
        offset.map(|ms| {
            if ms < 0 {
                base - Duration::from_millis(ms.unsigned_abs())
            } else {
                base + Duration::from_millis(ms.unsigned_abs())
            }
        })
    }

    proptest! {
        /// Recovered implies not refusing; refusing implies not
        /// recovered; and a breaker whose cooldown has elapsed without
        /// a success is neither — the half-open state that tells the
        /// two questions apart.
        #[test]
        fn recovered_never_refuses_and_a_cooled_breaker_is_neither(
            open in deadline(),
            paused in deadline(),
        ) {
            let base = Instant::now() + Duration::from_secs(200);
            let open_until = at(base, open);
            let paused_until = at(base, paused);
            let recovered = recovered_at(open_until, paused_until, base);
            let refusing = refusing_at(open_until, paused_until, base);
            prop_assert!(!(recovered && refusing));
            if let Some(until) = open_until {
                if base >= until {
                    let pause_running = paused_until.is_some_and(|p| base < p);
                    prop_assert!(!recovered, "a cooled breaker has recovered nothing");
                    prop_assert_eq!(refusing, pause_running, "and refuses only through a pause");
                }
            }
            if open_until.is_none() {
                let pause_running = paused_until.is_some_and(|p| base < p);
                prop_assert_eq!(recovered, !pause_running);
                prop_assert_eq!(refusing, pause_running);
            }
        }
    }
}
