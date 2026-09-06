//! The failover orchestrator over stub providers and real gates.

use super::*;
use crate::commands::play_native_resolve::NativeError;
use crate::error::AniError;
use crate::scraper::gate::{ScrapePriority, ScraperGate, FAILURE_THRESHOLD};
use crate::scraper::provider::{BrowseHit, EpisodeRef, Provider, ProviderId, StreamSource};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

/// A provider that only knows its name; the attempt decides what
/// each one does.
struct Stub(ProviderId);

#[async_trait::async_trait]
impl Provider for Stub {
    fn id(&self) -> ProviderId {
        self.0
    }
    async fn search(&self, _q: &str) -> crate::error::Result<Vec<BrowseHit>> {
        unreachable!()
    }
    async fn episodes(&self, _s: &str) -> crate::error::Result<Vec<EpisodeRef>> {
        unreachable!()
    }
    async fn has_mode(&self, _e: u64, _m: &str) -> crate::error::Result<bool> {
        unreachable!()
    }
    async fn master_playlist_url(&self, _e: u64, _m: &str) -> crate::error::Result<StreamSource> {
        unreachable!()
    }
    async fn playlist(&self, _u: &str, _r: Option<&str>) -> crate::error::Result<String> {
        unreachable!()
    }
    async fn detail_year(&self, _s: &str) -> crate::error::Result<Option<u32>> {
        unreachable!()
    }
    fn last_attempt_at(&self) -> Option<tokio::time::Instant> {
        None
    }
}

#[derive(Clone)]
enum Behavior {
    Answer(&'static str),
    Unreachable(fn() -> AniError),
    Miss { clean: bool },
    Stall,
}

/// The attempt: what each provider does when asked, and who was asked.
struct Scripted {
    behavior: HashMap<ProviderId, Behavior>,
    asked: Mutex<Vec<ProviderId>>,
}

impl Scripted {
    fn new(script: &[(ProviderId, Behavior)]) -> Self {
        Self {
            behavior: script.iter().cloned().collect(),
            asked: Mutex::new(Vec::new()),
        }
    }
    fn asked(&self) -> Vec<ProviderId> {
        self.asked.lock().expect("asked").clone()
    }
}

#[async_trait::async_trait]
impl Attempt for Scripted {
    type Output = &'static str;
    async fn run(&mut self, provider: &dyn Provider) -> Result<&'static str, NativeError> {
        self.asked.lock().expect("asked").push(provider.id());
        match self
            .behavior
            .get(&provider.id())
            .cloned()
            .expect("scripted")
        {
            Behavior::Answer(v) => Ok(v),
            Behavior::Unreachable(make) => Err(NativeError {
                error: make(),
                clean_miss: false,
                failed_at: None,
            }),
            Behavior::Miss { clean } => Err(NativeError {
                error: AniError::NoResults,
                clean_miss: clean,
                failed_at: None,
            }),
            Behavior::Stall => std::future::pending().await,
        }
    }
}

struct Gates {
    anidb: ScraperGate,
    hianime: ScraperGate,
}

impl Gates {
    fn new() -> Self {
        Self {
            anidb: ScraperGate::new(),
            hianime: ScraperGate::new(),
        }
    }
    fn of(&self, p: ProviderId) -> &ScraperGate {
        match p {
            ProviderId::Anidb => &self.anidb,
            ProviderId::Hianime => &self.hianime,
        }
    }
    fn open(&self, p: ProviderId) {
        for _ in 0..FAILURE_THRESHOLD {
            self.of(p).record(
                crate::scraper::gate::ScrapeOutcome::Failure,
                tokio::time::Instant::now(),
            );
        }
    }
}

const ORDER: [ProviderId; 2] = [ProviderId::Anidb, ProviderId::Hianime];

fn stub(p: ProviderId) -> crate::error::Result<Box<dyn Provider>> {
    Ok(Box::new(Stub(p)))
}

async fn run<'a>(
    gates: &'a Gates,
    priority: ScrapePriority,
    attempt: &mut Scripted,
) -> Result<Attempted<'a, &'static str>, NativeError> {
    with_failover(
        &ORDER,
        priority,
        Duration::from_secs(60),
        Duration::from_secs(20),
        stub,
        |p| gates.of(p),
        attempt,
    )
    .await
}

#[tokio::test]
async fn the_fallback_answers_when_the_primary_is_unreachable() {
    let gates = Gates::new();
    let mut attempt = Scripted::new(&[
        (
            ProviderId::Anidb,
            Behavior::Unreachable(|| AniError::Network),
        ),
        (ProviderId::Hianime, Behavior::Answer("from hianime")),
    ]);
    let got = run(&gates, ScrapePriority::Interactive, &mut attempt)
        .await
        .expect("answered");
    assert_eq!(got.provider, ProviderId::Hianime);
    assert_eq!(got.value, "from hianime");
    assert_eq!(attempt.asked(), [ProviderId::Anidb, ProviderId::Hianime]);
}

#[tokio::test]
async fn each_attempts_outcome_lands_on_its_own_gate() {
    let gates = Gates::new();
    let mut attempt = Scripted::new(&[
        (
            ProviderId::Anidb,
            Behavior::Unreachable(|| AniError::Network),
        ),
        (ProviderId::Hianime, Behavior::Answer("ok")),
    ]);
    for _ in 0..FAILURE_THRESHOLD {
        run(&gates, ScrapePriority::Interactive, &mut attempt)
            .await
            .expect("answered");
    }
    assert!(
        gates.anidb.is_open(),
        "the primary's failures opened its breaker"
    );
    assert!(
        !gates.hianime.is_open(),
        "the fallback's successes were recorded on the fallback's gate, not the primary's"
    );
}

#[tokio::test]
async fn a_miss_does_not_fail_over() {
    let gates = Gates::new();
    let mut attempt = Scripted::new(&[
        (ProviderId::Anidb, Behavior::Miss { clean: true }),
        (ProviderId::Hianime, Behavior::Answer("never asked")),
    ]);
    let err = run(&gates, ScrapePriority::Interactive, &mut attempt)
        .await
        .expect_err("a miss is an answer");
    assert!(matches!(err.error, AniError::NoResults));
    assert!(
        err.clean_miss,
        "the primary answered cleanly; the verdict stands"
    );
    assert_eq!(attempt.asked(), [ProviderId::Anidb]);
}

#[tokio::test]
async fn an_open_breaker_skips_the_primary_while_another_remains() {
    let gates = Gates::new();
    gates.open(ProviderId::Anidb);
    let mut attempt = Scripted::new(&[
        (ProviderId::Anidb, Behavior::Answer("must not be asked")),
        (ProviderId::Hianime, Behavior::Answer("from hianime")),
    ]);
    let got = run(&gates, ScrapePriority::Interactive, &mut attempt)
        .await
        .expect("answered");
    assert_eq!(got.provider, ProviderId::Hianime);
    assert_eq!(attempt.asked(), [ProviderId::Hianime]);
}

#[tokio::test]
async fn the_last_provider_is_tried_even_with_an_open_breaker() {
    let gates = Gates::new();
    gates.open(ProviderId::Anidb);
    gates.open(ProviderId::Hianime);
    let mut attempt = Scripted::new(&[
        (ProviderId::Anidb, Behavior::Answer("must not be asked")),
        (ProviderId::Hianime, Behavior::Answer("tried anyway")),
    ]);
    let got = run(&gates, ScrapePriority::Interactive, &mut attempt)
        .await
        .expect("answered");
    assert_eq!(got.value, "tried anyway");
    assert_eq!(attempt.asked(), [ProviderId::Hianime]);
}

#[tokio::test(start_paused = true)]
async fn a_stalled_primary_yields_to_the_fallback_within_its_budget() {
    let gates = Gates::new();
    let mut attempt = Scripted::new(&[
        (ProviderId::Anidb, Behavior::Stall),
        (ProviderId::Hianime, Behavior::Answer("from hianime")),
    ]);
    let started = tokio::time::Instant::now();
    let got = run(&gates, ScrapePriority::Interactive, &mut attempt)
        .await
        .expect("answered");
    assert_eq!(got.provider, ProviderId::Hianime);
    let waited = started.elapsed();
    assert!(
        waited >= Duration::from_secs(20) && waited < Duration::from_secs(21),
        "the primary got its budget and no more: {waited:?}"
    );
}

#[tokio::test]
async fn a_fallbacks_clean_miss_after_an_unreachable_primary_is_not_a_verdict() {
    let gates = Gates::new();
    let mut attempt = Scripted::new(&[
        (
            ProviderId::Anidb,
            Behavior::Unreachable(|| AniError::Upstream { status: 503 }),
        ),
        (ProviderId::Hianime, Behavior::Miss { clean: true }),
    ]);
    let err = run(&gates, ScrapePriority::Interactive, &mut attempt)
        .await
        .expect_err("missed");
    assert!(matches!(err.error, AniError::NoResults));
    assert!(
        !err.clean_miss,
        "the primary never answered; absence on the fallback proves nothing about it"
    );
}

#[tokio::test]
async fn when_every_provider_is_unreachable_the_primarys_error_surfaces() {
    let gates = Gates::new();
    let mut attempt = Scripted::new(&[
        (
            ProviderId::Anidb,
            Behavior::Unreachable(|| AniError::Upstream { status: 503 }),
        ),
        (
            ProviderId::Hianime,
            Behavior::Unreachable(|| AniError::Network),
        ),
    ]);
    let err = run(&gates, ScrapePriority::Interactive, &mut attempt)
        .await
        .expect_err("nobody answered");
    assert!(
        matches!(err.error, AniError::Upstream { status: 503 }),
        "the primary's outage is what the user is told about: {:?}",
        err.error
    );
    assert_eq!(attempt.asked(), [ProviderId::Anidb, ProviderId::Hianime]);
}

#[test]
fn what_fails_over_is_the_provider_being_unreachable_refusing_or_broken() {
    for e in [
        AniError::Network,
        AniError::Timeout,
        AniError::GateRefused,
        AniError::RateLimited {
            retry_after_secs: None,
        },
        AniError::Upstream { status: 503 },
        AniError::Upstream { status: 403 },
    ] {
        assert!(fails_over(&e), "{e:?}");
    }
    assert!(
        fails_over(&AniError::ParseFailed { detail: "x".into() }),
        "a parse failure is the site having changed shape — broken, not answering"
    );
    assert!(!fails_over(&AniError::NoResults));
    assert!(
        !fails_over(&AniError::Upstream { status: 404 }),
        "an answered not-found is an answer"
    );
}
