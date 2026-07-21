//! Tests for `crate::scraper::gate`. Extracted via `#[path]` per
//! `project_crap_inline_test_gotcha`. All tests run under tokio's
//! paused clock, so sleeps auto-advance and assertions on elapsed
//! time are deterministic.

use super::*;

#[tokio::test(start_paused = true)]
async fn first_background_admit_is_immediate() {
    let gate = ScraperGate::new();
    let t0 = Instant::now();
    gate.admit(ScrapePriority::Background).await.expect("admit");
    assert_eq!(Instant::now(), t0, "no wait for the first slot");
}

#[tokio::test(start_paused = true)]
async fn background_admits_are_spaced_by_the_interval() {
    let gate = ScraperGate::new();
    let t0 = Instant::now();
    gate.admit(ScrapePriority::Background).await.expect("first");
    gate.admit(ScrapePriority::Background)
        .await
        .expect("second");
    assert!(
        Instant::now() - t0 >= BACKGROUND_INTERVAL,
        "second background admit must wait out the interval"
    );
}

#[tokio::test(start_paused = true)]
async fn interactive_admits_never_wait() {
    let gate = ScraperGate::new();
    // Saturate the background schedule first.
    gate.admit(ScrapePriority::Background).await.expect("bg");
    let t0 = Instant::now();
    gate.admit(ScrapePriority::Interactive)
        .await
        .expect("interactive");
    assert_eq!(Instant::now(), t0, "interactive is unpaced");
}

#[tokio::test(start_paused = true)]
async fn fewer_than_threshold_failures_keep_the_gate_open_for_background() {
    let gate = ScraperGate::new();
    for _ in 0..FAILURE_THRESHOLD - 1 {
        gate.record_outcome(false);
    }
    assert!(gate.admit(ScrapePriority::Background).await.is_ok());
}

#[tokio::test(start_paused = true)]
async fn threshold_failures_open_the_breaker_for_background() {
    let gate = ScraperGate::new();
    for _ in 0..FAILURE_THRESHOLD {
        gate.record_outcome(false);
    }
    assert_eq!(
        gate.admit(ScrapePriority::Background).await,
        Err(GateClosed)
    );
}

#[tokio::test(start_paused = true)]
async fn interactive_admits_even_while_the_breaker_is_open() {
    let gate = ScraperGate::new();
    for _ in 0..FAILURE_THRESHOLD {
        gate.record_outcome(false);
    }
    assert!(gate.admit(ScrapePriority::Interactive).await.is_ok());
}

#[tokio::test(start_paused = true)]
async fn breaker_lets_a_probe_through_after_the_cooldown() {
    let gate = ScraperGate::new();
    for _ in 0..FAILURE_THRESHOLD {
        gate.record_outcome(false);
    }
    assert!(gate.admit(ScrapePriority::Background).await.is_err());
    tokio::time::advance(BREAKER_COOLDOWN).await;
    assert!(
        gate.admit(ScrapePriority::Background).await.is_ok(),
        "cooldown elapsed: half-open probe admitted"
    );
}

#[tokio::test(start_paused = true)]
async fn one_failure_after_the_cooldown_reopens_immediately() {
    let gate = ScraperGate::new();
    for _ in 0..FAILURE_THRESHOLD {
        gate.record_outcome(false);
    }
    tokio::time::advance(BREAKER_COOLDOWN).await;
    gate.admit(ScrapePriority::Background)
        .await
        .expect("half-open probe");
    // The probe failed too — the run of failures never broke, so the
    // breaker snaps shut without needing three more.
    gate.record_outcome(false);
    assert_eq!(
        gate.admit(ScrapePriority::Background).await,
        Err(GateClosed)
    );
}

#[tokio::test(start_paused = true)]
async fn queued_background_admit_rechecks_the_breaker_after_waiting() {
    // Cold-start burst: several background probes reserve future
    // slots before any of the first requests report failures. A
    // caller already sleeping toward its slot must re-check the
    // breaker when it wakes — otherwise the whole queued burst
    // proceeds against an open breaker and the gate never stops
    // within the failure threshold.
    let gate = std::sync::Arc::new(ScraperGate::new());
    gate.admit(ScrapePriority::Background)
        .await
        .expect("first slot");
    let queued = {
        let gate = gate.clone();
        tokio::spawn(async move { gate.admit(ScrapePriority::Background).await })
    };
    // Let the queued admit reserve its slot and enter its sleep.
    tokio::task::yield_now().await;
    for _ in 0..FAILURE_THRESHOLD {
        gate.record_outcome(false);
    }
    assert_eq!(
        queued.await.expect("join"),
        Err(GateClosed),
        "a queued admit must not proceed once the breaker opened during its wait"
    );
}

#[tokio::test(start_paused = true)]
async fn half_open_admits_exactly_one_probe_until_it_reports() {
    let gate = ScraperGate::new();
    for _ in 0..FAILURE_THRESHOLD {
        gate.record_outcome(false);
    }
    tokio::time::advance(BREAKER_COOLDOWN).await;
    gate.admit(ScrapePriority::Background)
        .await
        .expect("the single half-open trial");
    // A second background caller arriving before the trial reports
    // must be refused — with 500 ms slot spacing but ~1 s+ request
    // latency, letting it queue would put extra probes on a possibly
    // still-limited upstream during what the gate documents as one
    // half-open probe.
    assert_eq!(
        gate.admit(ScrapePriority::Background).await,
        Err(GateClosed)
    );
    // Trial succeeds → the gate opens for everyone again.
    gate.record_outcome(true);
    assert!(gate.admit(ScrapePriority::Background).await.is_ok());
}

#[tokio::test(start_paused = true)]
async fn failed_half_open_trial_reopens_for_the_full_cooldown() {
    let gate = ScraperGate::new();
    for _ in 0..FAILURE_THRESHOLD {
        gate.record_outcome(false);
    }
    tokio::time::advance(BREAKER_COOLDOWN).await;
    gate.admit(ScrapePriority::Background).await.expect("trial");
    gate.record_outcome(false);
    assert_eq!(
        gate.admit(ScrapePriority::Background).await,
        Err(GateClosed)
    );
    // And the next trial needs a fresh cooldown, not just a slot.
    tokio::time::advance(BACKGROUND_INTERVAL).await;
    assert_eq!(
        gate.admit(ScrapePriority::Background).await,
        Err(GateClosed)
    );
    tokio::time::advance(BREAKER_COOLDOWN).await;
    assert!(gate.admit(ScrapePriority::Background).await.is_ok());
}

#[tokio::test(start_paused = true)]
async fn abandoned_half_open_trial_unblocks_after_the_stale_window() {
    // A trial whose future was dropped (cancelled prefetch) never
    // records an outcome. It must not wedge the gate shut forever —
    // after the stale window a new trial may start.
    let gate = ScraperGate::new();
    for _ in 0..FAILURE_THRESHOLD {
        gate.record_outcome(false);
    }
    tokio::time::advance(BREAKER_COOLDOWN).await;
    gate.admit(ScrapePriority::Background)
        .await
        .expect("first trial, then abandoned");
    tokio::time::advance(HALF_OPEN_TRIAL_STALE).await;
    assert!(
        gate.admit(ScrapePriority::Background).await.is_ok(),
        "stale trial must not block a new probe"
    );
}

#[tokio::test(start_paused = true)]
async fn success_closes_the_breaker_and_resets_the_run() {
    let gate = ScraperGate::new();
    for _ in 0..FAILURE_THRESHOLD {
        gate.record_outcome(false);
    }
    assert!(gate.admit(ScrapePriority::Background).await.is_err());
    gate.record_outcome(true);
    assert!(
        gate.admit(ScrapePriority::Background).await.is_ok(),
        "a success (e.g. an interactive request got through) closes the breaker"
    );
    // And the consecutive counter restarted: two more failures stay
    // under the threshold.
    gate.record_outcome(false);
    gate.record_outcome(false);
    assert!(gate.admit(ScrapePriority::Background).await.is_ok());
}
