use super::*;
use crate::commands::play_native_resolve::NativeError;
use crate::error::AniError;

#[test]
fn breaker_outcome_treats_answered_verdicts_as_health() {
    // NoResults only arises from provider-answered verdicts — a
    // rejected pool, an absent episode, a sub-only show asked for
    // dub. The provider answering is breaker health whatever the
    // answer was; recording it as failure opens the breaker on
    // repeated dub requests for sub-only shows and refuses unrelated
    // background traffic.
    let absent_episode = NativeError {
        error: AniError::NoResults,
        clean_miss: false,
    };
    assert!(matches!(
        breaker_outcome(&Err(absent_episode)),
        Some(ScrapeOutcome::Success)
    ));
    // Weather stays distress.
    let refusal = NativeError {
        error: AniError::Upstream { status: 403 },
        clean_miss: false,
    };
    assert!(matches!(
        breaker_outcome(&Err(refusal)),
        Some(ScrapeOutcome::Failure)
    ));
    let limited = NativeError {
        error: AniError::RateLimited {
            retry_after_secs: Some(30),
        },
        clean_miss: false,
    };
    assert!(matches!(
        breaker_outcome(&Err(limited)),
        Some(ScrapeOutcome::RateLimited { .. })
    ));
    let transport = NativeError {
        error: AniError::Network,
        clean_miss: false,
    };
    assert!(matches!(
        breaker_outcome(&Err(transport)),
        Some(ScrapeOutcome::Failure)
    ));
}

#[test]
fn a_gate_refusal_is_no_evidence_at_all() {
    // A refusal is the gate's own answer, not the provider's:
    // recording it as failure lets every background warmup refresh
    // the open breaker's cooldown without a single provider request,
    // so half-open recovery never arrives. The refusal produces NO
    // outcome — the breaker only ever hears about requests that got
    // past the gate.
    let refused = NativeError {
        error: AniError::GateRefused,
        clean_miss: false,
    };
    assert!(breaker_outcome(&Err(refused)).is_none());
}

#[test]
fn an_http_429_is_a_rate_limit_to_the_breaker() {
    // anidb answers HTTP 429 as a status, not the in-band payload
    // the typed RateLimited variant carries, so it surfaces as
    // Upstream { 429 }. Recording it as an ordinary failure lets the
    // gate keep admitting queued background resolutions until the
    // three-failure threshold, despite the provider explicitly
    // asking clients to stop — the hintless rate-limit outcome opens
    // the advertised pause immediately.
    let limited = NativeError {
        error: AniError::Upstream { status: 429 },
        clean_miss: false,
    };
    assert!(matches!(
        breaker_outcome(&Err(limited)),
        Some(ScrapeOutcome::RateLimited { retry_after: None })
    ));
}

#[test]
fn an_answered_not_found_at_the_episode_step_is_health() {
    // The picker already treats a stale candidate slug's 404 as an
    // answered verdict, but a 404 from the SELECTED episode's
    // languages or embed request rides resolve_episode's error
    // straight here — and three dead-source plays would open the
    // global breaker although the provider answered every request.
    // Not-found-shaped statuses are answered verdicts wherever they
    // arise; only block-shaped ones (403, 429, 5xx) are distress.
    for status in [400, 404, 410] {
        let dead_source = NativeError {
            error: AniError::Upstream { status },
            clean_miss: false,
        };
        assert!(
            matches!(
                breaker_outcome(&Err(dead_source)),
                Some(ScrapeOutcome::Success)
            ),
            "status {status} answered; it must be recorded as health"
        );
    }
    for status in [500, 502, 503] {
        let block = NativeError {
            error: AniError::Upstream { status },
            clean_miss: false,
        };
        assert!(
            matches!(breaker_outcome(&Err(block)), Some(ScrapeOutcome::Failure)),
            "status {status} is block-shaped distress"
        );
    }
}
