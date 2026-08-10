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
        ScrapeOutcome::Success
    ));
    // Weather stays distress.
    let refusal = NativeError {
        error: AniError::Upstream { status: 403 },
        clean_miss: false,
    };
    assert!(matches!(
        breaker_outcome(&Err(refusal)),
        ScrapeOutcome::Failure
    ));
    let limited = NativeError {
        error: AniError::RateLimited {
            retry_after_secs: Some(30),
        },
        clean_miss: false,
    };
    assert!(matches!(
        breaker_outcome(&Err(limited)),
        ScrapeOutcome::RateLimited { .. }
    ));
    let transport = NativeError {
        error: AniError::Network,
        clean_miss: false,
    };
    assert!(matches!(
        breaker_outcome(&Err(transport)),
        ScrapeOutcome::Failure
    ));
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
        ScrapeOutcome::RateLimited { retry_after: None }
    ));
}
