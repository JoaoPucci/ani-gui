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
        gate.record_outcome(false, Instant::now());
    }
    assert!(gate.admit(ScrapePriority::Background).await.is_ok());
}

#[tokio::test(start_paused = true)]
async fn threshold_failures_open_the_breaker_for_background() {
    let gate = ScraperGate::new();
    for _ in 0..FAILURE_THRESHOLD {
        gate.record_outcome(false, Instant::now());
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
        gate.record_outcome(false, Instant::now());
    }
    assert!(gate.admit(ScrapePriority::Interactive).await.is_ok());
}

#[tokio::test(start_paused = true)]
async fn breaker_lets_a_probe_through_after_the_cooldown() {
    let gate = ScraperGate::new();
    for _ in 0..FAILURE_THRESHOLD {
        gate.record_outcome(false, Instant::now());
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
        gate.record_outcome(false, Instant::now());
    }
    tokio::time::advance(BREAKER_COOLDOWN).await;
    gate.admit(ScrapePriority::Background)
        .await
        .expect("half-open probe");
    // The probe failed too — the run of failures never broke, so the
    // breaker snaps shut without needing three more.
    gate.record_outcome(false, Instant::now());
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
        gate.record_outcome(false, Instant::now());
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
        gate.record_outcome(false, Instant::now());
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
    gate.record_outcome(true, Instant::now());
    assert!(gate.admit(ScrapePriority::Background).await.is_ok());
}

#[tokio::test(start_paused = true)]
async fn failed_half_open_trial_reopens_for_the_full_cooldown() {
    let gate = ScraperGate::new();
    for _ in 0..FAILURE_THRESHOLD {
        gate.record_outcome(false, Instant::now());
    }
    tokio::time::advance(BREAKER_COOLDOWN).await;
    gate.admit(ScrapePriority::Background).await.expect("trial");
    gate.record_outcome(false, Instant::now());
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
        gate.record_outcome(false, Instant::now());
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
async fn opening_the_breaker_drops_queued_slot_reservations() {
    // A cold burst can reserve slots minutes past the cooldown.
    // Those callers all get refused at wake once the breaker opens,
    // but their reservations must not make the half-open trial wait
    // behind requests that will never run — the documented 60 s
    // recovery would silently become minutes.
    let gate = ScraperGate::new();
    // Reserve 300 slots (~150 s of schedule) and drop the callers —
    // polling each admit once runs its reservation section, and the
    // reservation outlives the future. Exactly the starvation input.
    for _ in 0..300 {
        let mut fut = Box::pin(gate.admit(ScrapePriority::Background));
        let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
        let _ = std::future::Future::poll(fut.as_mut(), &mut cx);
    }
    for _ in 0..FAILURE_THRESHOLD {
        gate.record_outcome(false, Instant::now());
    }
    tokio::time::advance(BREAKER_COOLDOWN).await;
    let t0 = Instant::now();
    gate.admit(ScrapePriority::Background)
        .await
        .expect("half-open trial");
    assert!(
        Instant::now() - t0 < BACKGROUND_INTERVAL,
        "the trial must not wait behind dead reservations"
    );
}

#[tokio::test(start_paused = true)]
async fn straggler_failures_keep_the_original_staleness_boundary() {
    // While the breaker is open, a leftover in-flight failure must
    // not refresh opened_at: an interactive request that started
    // during the cooldown (after the real opening) is fresh evidence
    // and must still be able to close the breaker even when an older
    // failure reports in between.
    let gate = ScraperGate::new();
    for _ in 0..FAILURE_THRESHOLD {
        gate.record_outcome(false, Instant::now());
    }
    tokio::time::advance(Duration::from_secs(1)).await;
    let fresh_start = Instant::now();
    tokio::time::advance(Duration::from_secs(1)).await;
    gate.record_outcome(false, Instant::now());
    gate.record_outcome(true, fresh_start);
    assert!(
        gate.admit(ScrapePriority::Background).await.is_ok(),
        "a success started after the ORIGINAL opening must close the breaker"
    );
}

#[tokio::test(start_paused = true)]
async fn stale_success_does_not_close_an_open_breaker() {
    // Admits are spaced 500 ms apart but the HTTP calls complete
    // independently: a slow request admitted before the storm can
    // report success after three later failures opened the breaker.
    // That success is pre-storm evidence and must not cancel the
    // cooldown — only a request STARTED after the breaker opened
    // (the half-open trial, or an interactive request during the
    // cooldown) proves the upstream recovered.
    let gate = ScraperGate::new();
    let started_before_storm = Instant::now();
    tokio::time::advance(Duration::from_secs(1)).await;
    for _ in 0..FAILURE_THRESHOLD {
        gate.record_outcome(false, Instant::now());
    }
    assert!(gate.admit(ScrapePriority::Background).await.is_err());
    gate.record_outcome(true, started_before_storm);
    assert!(
        gate.admit(ScrapePriority::Background).await.is_err(),
        "a success that predates the opening must not close the breaker"
    );
}

#[tokio::test(start_paused = true)]
async fn fresh_success_during_the_cooldown_still_closes_the_breaker() {
    // An interactive request that started after the breaker opened
    // and succeeded is fresh evidence — it closes the breaker early.
    let gate = ScraperGate::new();
    for _ in 0..FAILURE_THRESHOLD {
        gate.record_outcome(false, Instant::now());
    }
    tokio::time::advance(Duration::from_secs(1)).await;
    gate.record_outcome(true, Instant::now());
    assert!(gate.admit(ScrapePriority::Background).await.is_ok());
}

#[tokio::test(start_paused = true)]
async fn success_closes_the_breaker_and_resets_the_run() {
    let gate = ScraperGate::new();
    for _ in 0..FAILURE_THRESHOLD {
        gate.record_outcome(false, Instant::now());
    }
    assert!(gate.admit(ScrapePriority::Background).await.is_err());
    gate.record_outcome(true, Instant::now());
    assert!(
        gate.admit(ScrapePriority::Background).await.is_ok(),
        "a success (e.g. an interactive request got through) closes the breaker"
    );
    // And the consecutive counter restarted: two more failures stay
    // under the threshold.
    gate.record_outcome(false, Instant::now());
    gate.record_outcome(false, Instant::now());
    assert!(gate.admit(ScrapePriority::Background).await.is_ok());
}

#[tokio::test(start_paused = true)]
async fn stale_failures_cannot_reopen_past_a_newer_recovery() {
    // Overlapping paced requests: three fast failures open the
    // breaker, a fresh interactive success closes it — and then three
    // stragglers that STARTED before that recovery finish with slow
    // failures. Counting them would reopen the breaker for a full
    // cooldown despite the newer evidence; they must be ignored, the
    // mirror of the stale-success filter against `opened_at`.
    let gate = ScraperGate::new();
    let straggler_started = Instant::now();
    tokio::time::advance(Duration::from_secs(1)).await;

    for _ in 0..FAILURE_THRESHOLD {
        gate.record_outcome(false, Instant::now());
    }
    tokio::time::advance(Duration::from_secs(1)).await;
    gate.record_outcome(true, Instant::now());

    for _ in 0..FAILURE_THRESHOLD {
        gate.record_outcome(false, straggler_started);
    }
    assert!(
        gate.admit(ScrapePriority::Background).await.is_ok(),
        "failures older than the accepted recovery must not reopen the breaker"
    );
}

#[tokio::test(start_paused = true)]
async fn failures_newer_than_the_successful_start_still_count() {
    // A slow request starts first and succeeds much later; paced
    // requests that started AFTER it may be watching a NEWER
    // rate-limit episode. Anchoring the recovery boundary to the
    // success's completion time would discard their failures as
    // stale and hold the breaker open to background traffic — the
    // boundary must be the successful request's START.
    let gate = ScraperGate::new();
    let slow_started = Instant::now();
    tokio::time::advance(Duration::from_secs(1)).await;
    let newer_started = Instant::now();
    tokio::time::advance(Duration::from_secs(1)).await;

    // The slow request lands now, long after the newer cohort began.
    gate.record_outcome(true, slow_started);
    for _ in 0..FAILURE_THRESHOLD {
        gate.record_outcome(false, newer_started);
    }
    assert!(
        gate.admit(ScrapePriority::Background).await.is_err(),
        "failures that started after the successful start are fresh evidence and must open the breaker"
    );
}

// ── advertised-window pause (typed rate limit) ──────────────────────

#[tokio::test(start_paused = true)]
async fn one_rate_limit_pauses_background_for_the_advertised_window() {
    // A single typed rate limit opens the pause — no threshold
    // counting — and background admits WAIT through it instead of
    // being refused, so their rows resolve late instead of never.
    let gate = ScraperGate::new();
    gate.record(
        ScrapeOutcome::RateLimited {
            retry_after: Some(Duration::from_secs(9)),
        },
        Instant::now(),
    );
    let t0 = Instant::now();
    gate.admit(ScrapePriority::Background).await.expect("admit");
    let waited = Instant::now() - t0;
    assert!(
        waited >= Duration::from_secs(9),
        "waited only {waited:?} of the 9 s advertised window"
    );
}

#[tokio::test(start_paused = true)]
async fn rate_limit_without_hint_pauses_for_the_default_cooldown() {
    let gate = ScraperGate::new();
    gate.record(
        ScrapeOutcome::RateLimited { retry_after: None },
        Instant::now(),
    );
    let t0 = Instant::now();
    gate.admit(ScrapePriority::Background).await.expect("admit");
    assert!(
        Instant::now() - t0 >= BREAKER_COOLDOWN,
        "hintless rate limit falls back to the breaker cooldown"
    );
}

#[tokio::test(start_paused = true)]
async fn interactive_admits_ignore_the_rate_limit_window() {
    let gate = ScraperGate::new();
    gate.record(
        ScrapeOutcome::RateLimited {
            retry_after: Some(Duration::from_secs(30)),
        },
        Instant::now(),
    );
    let t0 = Instant::now();
    gate.admit(ScrapePriority::Interactive)
        .await
        .expect("interactive");
    assert_eq!(Instant::now(), t0, "interactive is never paused");
}

#[tokio::test(start_paused = true)]
async fn resumed_admits_stay_paced_after_the_window() {
    // Callers queued during the window must not burst out together at
    // its end — the resume is paced at the background interval.
    let gate = ScraperGate::new();
    gate.record(
        ScrapeOutcome::RateLimited {
            retry_after: Some(Duration::from_secs(5)),
        },
        Instant::now(),
    );
    let t0 = Instant::now();
    let (a, b) = tokio::join!(
        gate.admit(ScrapePriority::Background),
        gate.admit(ScrapePriority::Background),
    );
    a.expect("first");
    b.expect("second");
    assert!(
        Instant::now() - t0 >= Duration::from_secs(5) + BACKGROUND_INTERVAL,
        "second resumed admit is spaced one interval past the window"
    );
}

#[tokio::test(start_paused = true)]
async fn success_after_the_window_opened_ends_the_pause() {
    // An interactive request that started after the window opened and
    // succeeded is live proof the limit lifted early.
    let gate = ScraperGate::new();
    gate.record(
        ScrapeOutcome::RateLimited {
            retry_after: Some(Duration::from_secs(60)),
        },
        Instant::now(),
    );
    tokio::time::advance(Duration::from_secs(1)).await;
    gate.record(ScrapeOutcome::Success, Instant::now());
    let t0 = Instant::now();
    gate.admit(ScrapePriority::Background).await.expect("admit");
    assert_eq!(Instant::now(), t0, "pause cleared by fresh success");
}

#[tokio::test(start_paused = true)]
async fn typed_failure_still_feeds_the_counting_breaker() {
    // Untyped failures keep today's semantics through the typed API.
    let gate = ScraperGate::new();
    for _ in 0..FAILURE_THRESHOLD {
        gate.record(ScrapeOutcome::Failure, Instant::now());
    }
    assert_eq!(
        gate.admit(ScrapePriority::Background).await,
        Err(GateClosed)
    );
}

// ── outcome classification ──────────────────────────────────────────

#[test]
fn outcome_of_maps_the_typed_rate_limit_with_its_hint() {
    let r: Result<(), crate::error::AniError> = Err(crate::error::AniError::RateLimited {
        retry_after_secs: Some(7),
    });
    assert_eq!(
        outcome_of(&r),
        ScrapeOutcome::RateLimited {
            retry_after: Some(Duration::from_secs(7)),
        }
    );
    let r: Result<(), crate::error::AniError> = Err(crate::error::AniError::RateLimited {
        retry_after_secs: None,
    });
    assert_eq!(
        outcome_of(&r),
        ScrapeOutcome::RateLimited { retry_after: None }
    );
}

#[test]
fn outcome_of_folds_everything_else_to_success_or_failure() {
    let ok: Result<u8, crate::error::AniError> = Ok(1);
    assert_eq!(outcome_of(&ok), ScrapeOutcome::Success);
    let err: Result<u8, crate::error::AniError> = Err(crate::error::AniError::Io);
    assert_eq!(outcome_of(&err), ScrapeOutcome::Failure);
}

#[tokio::test(start_paused = true)]
async fn an_expired_pause_admits_immediately() {
    // Nobody recorded a success — the window simply passed. The next
    // admit clears the stale pause lazily and goes straight through.
    let gate = ScraperGate::new();
    gate.record(
        ScrapeOutcome::RateLimited {
            retry_after: Some(Duration::from_secs(1)),
        },
        Instant::now(),
    );
    tokio::time::advance(Duration::from_secs(2)).await;
    let t0 = Instant::now();
    gate.admit(ScrapePriority::Background).await.expect("admit");
    assert_eq!(Instant::now(), t0, "expired window costs nothing");
}

#[tokio::test(start_paused = true)]
async fn overlapping_rate_limits_keep_the_later_deadline() {
    let gate = ScraperGate::new();
    gate.record(
        ScrapeOutcome::RateLimited {
            retry_after: Some(Duration::from_secs(5)),
        },
        Instant::now(),
    );
    gate.record(
        ScrapeOutcome::RateLimited {
            retry_after: Some(Duration::from_secs(3)),
        },
        Instant::now(),
    );
    let t0 = Instant::now();
    gate.admit(ScrapePriority::Background).await.expect("admit");
    assert!(
        Instant::now() - t0 >= Duration::from_secs(5),
        "the earlier, longer deadline stands"
    );
}

#[tokio::test(start_paused = true)]
async fn a_sleeper_wakes_refused_when_the_breaker_opened_meanwhile() {
    // A queued background caller re-checks on wake: failures reported
    // while it slept must refuse it, not let it fire into the storm.
    let gate = ScraperGate::new();
    gate.admit(ScrapePriority::Background).await.expect("first");
    let (second, ()) = tokio::join!(gate.admit(ScrapePriority::Background), async {
        for _ in 0..FAILURE_THRESHOLD {
            gate.record(ScrapeOutcome::Failure, Instant::now());
        }
    });
    assert_eq!(second, Err(GateClosed), "woke into an open breaker");
}

#[tokio::test(start_paused = true)]
async fn default_is_a_fresh_gate() {
    let gate = ScraperGate::default();
    gate.admit(ScrapePriority::Background).await.expect("admit");
}
