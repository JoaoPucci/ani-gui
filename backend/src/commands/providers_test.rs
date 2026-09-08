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
        None,
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

/// The runner reports the answer as the provider gave it. A clean
/// miss on the fallback while the primary was unreachable is the
/// fallback's clean miss — whose verdict a negative row is, and
/// while whom it may be served, is the cache's rule, which names the
/// provider on the row and serves it only while every provider
/// ahead of that one is down.
#[tokio::test]
async fn a_fallbacks_clean_miss_after_an_unreachable_primary_stays_its_clean_miss() {
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
        err.clean_miss,
        "the fallback searched every alias and found nothing; that is its clean miss"
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

// ── the state's providers ───────────────────────────────────────────

use crate::app::AppState;
use crate::commands::play_native_resolve::NativeResolveRequest;

/// A state whose providers are all unroutable: every attempt is the
/// transport failing to connect, the shape that fails over.
fn unroutable_state(td: &tempfile::TempDir, order: &[ProviderId]) -> AppState {
    use crate::meta::kitsu::KitsuClient;
    use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
    use std::sync::Arc;
    AppState {
        secret: AppSecret::random(),
        sessions: SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        meta_http: reqwest::Client::new(),
        proxy_origin: ProxyOrigin::new("127.0.0.1", 12_345),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path: td.path().join("history"),
        anidb_base: Some("http://127.0.0.1:1".into()),
        anidb_gate: Arc::new(ScraperGate::new()),
        hianime_base: Some("http://127.0.0.1:1".into()),
        hianime_gate: Arc::new(ScraperGate::new()),
        provider_order: order.to_vec(),
        image_cache_dir: td.path().join("images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: KitsuClient::with_base(reqwest::Client::new(), "http://127.0.0.1:1"),
        config_path: td.path().join("config.toml"),
        state_dir: td.path().join("state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    }
}

/// One failure short of the breaker opening, so the next recorded
/// failure — and only a recorded failure — opens it.
fn one_short_of_open(gate: &ScraperGate) {
    for _ in 0..FAILURE_THRESHOLD - 1 {
        gate.record(
            crate::scraper::gate::ScrapeOutcome::Failure,
            tokio::time::Instant::now(),
        );
    }
}

async fn resolve_unreachable(state: &AppState) -> NativeError {
    let mut attempt = ResolveAttempt {
        request: NativeResolveRequest {
            title: "Unreachable Show",
            alt_titles: &[],
            episode: "1",
            mode: "sub",
            quality: "best",
            expected_count: None,
            year: None,
            subtype: None,
        },
        on_progress: &mut |_| {},
        answered_by: None,
    };
    super::run(state, ScrapePriority::Interactive, &mut attempt)
        .await
        .expect_err("nobody answers")
}

#[test]
fn each_provider_gets_a_client_of_its_own_on_its_own_gate() {
    let td = tempfile::tempdir().expect("td");
    let state = unroutable_state(&td, &ORDER);
    for p in ORDER {
        let client = client_for(&state, Origins::of(&state), p, ScrapePriority::Interactive)
            .expect("a curl on PATH");
        assert_eq!(client.id(), p);
    }
    assert!(std::ptr::eq(
        gate_of(&state, ProviderId::Anidb),
        &*state.anidb_gate
    ));
    assert!(std::ptr::eq(
        gate_of(&state, ProviderId::Hianime),
        &*state.hianime_gate
    ));
}

#[tokio::test]
async fn a_walk_runs_against_the_states_providers_and_each_hears_its_own_outcome() {
    let td = tempfile::tempdir().expect("td");
    let state = unroutable_state(&td, &ORDER);
    one_short_of_open(&state.anidb_gate);
    one_short_of_open(&state.hianime_gate);
    let err = resolve_unreachable(&state).await;
    assert!(fails_over(&err.error), "{:?}", err.error);
    assert!(
        state.anidb_gate.is_open(),
        "the primary's failure landed on its own breaker"
    );
    assert!(
        state.hianime_gate.is_open(),
        "the fallback's failure landed on its own breaker"
    );
}

#[tokio::test]
async fn only_the_providers_the_state_orders_are_asked() {
    let td = tempfile::tempdir().expect("td");
    let state = unroutable_state(&td, &[ProviderId::Anidb]);
    one_short_of_open(&state.anidb_gate);
    one_short_of_open(&state.hianime_gate);
    let err = resolve_unreachable(&state).await;
    assert!(fails_over(&err.error), "{:?}", err.error);
    assert!(state.anidb_gate.is_open());
    assert!(
        !state.hianime_gate.is_open(),
        "a provider outside the order is never asked"
    );
}

// ── a remembered provider ───────────────────────────────────────────

/// An attempt that records who it was asked of and answers nobody.
struct Recording {
    asked: Vec<ProviderId>,
}

#[async_trait::async_trait]
impl Attempt for Recording {
    type Output = ();
    async fn run(&mut self, provider: &dyn Provider) -> Result<(), NativeError> {
        self.asked.push(provider.id());
        Err(NativeError {
            error: AniError::Network,
            clean_miss: false,
            failed_at: None,
        })
    }
}

#[test]
fn a_remembered_provider_leads_the_order_and_a_foreign_one_changes_nothing() {
    assert_eq!(
        order_with_affinity(&ORDER, Some(ProviderId::Hianime)),
        vec![ProviderId::Hianime, ProviderId::Anidb]
    );
    assert_eq!(order_with_affinity(&ORDER, Some(ProviderId::Anidb)), ORDER);
    assert_eq!(order_with_affinity(&ORDER, None), ORDER);
    assert_eq!(
        order_with_affinity(&[ProviderId::Anidb], Some(ProviderId::Hianime)),
        vec![ProviderId::Anidb],
        "a provider the state does not list is not asked on a row's say-so"
    );
}

#[tokio::test]
async fn a_walk_starts_from_the_remembered_provider() {
    let td = tempfile::tempdir().expect("td");
    let state = unroutable_state(&td, &ORDER);
    let mut attempt = Recording { asked: Vec::new() };
    let _ = run_from(
        &state,
        Some(ProviderId::Hianime),
        ScrapePriority::Interactive,
        &mut attempt,
    )
    .await;
    assert_eq!(attempt.asked, vec![ProviderId::Hianime, ProviderId::Anidb]);
}

mod affinity_props {
    use super::order_with_affinity;
    use crate::scraper::provider::ProviderId;
    use proptest::prelude::*;

    fn provider() -> impl Strategy<Value = ProviderId> {
        prop_oneof![Just(ProviderId::Anidb), Just(ProviderId::Hianime)]
    }

    /// Any listing of the known providers, each at most once.
    fn order() -> impl Strategy<Value = Vec<ProviderId>> {
        prop::collection::vec(provider(), 0..3).prop_map(|mut v| {
            let mut seen = Vec::new();
            v.retain(|p| {
                let new = !seen.contains(p);
                seen.push(*p);
                new
            });
            v
        })
    }

    proptest! {
        /// The result is the order itself, reordered at most by
        /// moving the remembered provider to the front.
        #[test]
        fn the_order_is_kept_except_for_the_remembered_provider(
            order in order(),
            hint in prop::option::of(provider()),
        ) {
            let got = order_with_affinity(&order, hint);
            let mut sorted_got = got.clone();
            let mut sorted_order = order.clone();
            sorted_got.sort_by_key(|p| p.label());
            sorted_order.sort_by_key(|p| p.label());
            prop_assert_eq!(sorted_got, sorted_order);
            match hint {
                Some(h) if order.contains(&h) => prop_assert_eq!(got.first(), Some(&h)),
                _ => prop_assert_eq!(&got, &order),
            }
            let rest_got: Vec<_> = got.iter().filter(|p| Some(**p) != hint).collect();
            let rest_order: Vec<_> = order.iter().filter(|p| Some(**p) != hint).collect();
            prop_assert_eq!(rest_got, rest_order);
        }
    }
}

// ── affinity yields; skipped providers are retried ──────────────────

async fn run_with<'a>(
    gates: &'a Gates,
    order: &[ProviderId],
    remembered: Option<ProviderId>,
    priority: ScrapePriority,
    attempt: &mut Scripted,
) -> Result<Attempted<'a, &'static str>, NativeError> {
    with_failover(
        order,
        remembered,
        priority,
        Duration::from_secs(60),
        Duration::from_secs(20),
        stub,
        |p| gates.of(p),
        attempt,
    )
    .await
}

/// A positive availability row proves the show and its mode at the
/// show's level, not that every episode has an embed; a remembered
/// provider's answered dead end therefore yields to the rest of the
/// order instead of ending the walk on an episode another provider
/// may serve.
#[tokio::test]
async fn a_remembered_providers_answered_miss_yields_to_the_rest() {
    let gates = Gates::new();
    let mut attempt = Scripted::new(&[
        (ProviderId::Hianime, Behavior::Miss { clean: false }),
        (ProviderId::Anidb, Behavior::Answer("anidb")),
    ]);
    let got = run_with(
        &gates,
        &[ProviderId::Hianime, ProviderId::Anidb],
        Some(ProviderId::Hianime),
        ScrapePriority::Interactive,
        &mut attempt,
    )
    .await
    .expect("the rest of the order answered");
    assert_eq!(got.provider, ProviderId::Anidb);
    assert_eq!(got.value, "anidb");
    assert_eq!(
        attempt.asked(),
        vec![ProviderId::Hianime, ProviderId::Anidb]
    );
}

#[tokio::test]
async fn a_remembered_providers_miss_stands_when_the_rest_are_unreachable() {
    let gates = Gates::new();
    let mut attempt = Scripted::new(&[
        (ProviderId::Hianime, Behavior::Miss { clean: true }),
        (
            ProviderId::Anidb,
            Behavior::Unreachable(|| AniError::Network),
        ),
    ]);
    let err = run_with(
        &gates,
        &[ProviderId::Hianime, ProviderId::Anidb],
        Some(ProviderId::Hianime),
        ScrapePriority::Interactive,
        &mut attempt,
    )
    .await
    .expect_err("nobody served it");
    assert!(matches!(err.error, AniError::NoResults), "{:?}", err.error);
    assert!(
        err.clean_miss,
        "the remembered provider's clean miss stands as its own"
    );
}

/// The gate admits an interactive click through an open breaker as
/// its half-open trial; the skip is only the fast path. When every
/// provider that was tried answered a miss, the skipped ones are
/// asked before the user is told the show is nowhere.
#[tokio::test]
async fn a_skipped_provider_is_retried_when_the_rest_only_missed_on_an_interactive_walk() {
    let gates = Gates::new();
    gates.open(ProviderId::Anidb);
    let mut attempt = Scripted::new(&[
        (ProviderId::Anidb, Behavior::Answer("anidb")),
        (ProviderId::Hianime, Behavior::Miss { clean: true }),
    ]);
    let got = run_with(
        &gates,
        &ORDER,
        None,
        ScrapePriority::Interactive,
        &mut attempt,
    )
    .await
    .expect("the recovered primary answered");
    assert_eq!(got.provider, ProviderId::Anidb);
    assert_eq!(
        attempt.asked(),
        vec![ProviderId::Hianime, ProviderId::Anidb],
        "skipped first, asked last"
    );
}

#[tokio::test]
async fn a_skipped_provider_stays_skipped_on_a_background_walk() {
    let gates = Gates::new();
    gates.open(ProviderId::Anidb);
    let mut attempt = Scripted::new(&[
        (ProviderId::Anidb, Behavior::Answer("anidb")),
        (ProviderId::Hianime, Behavior::Miss { clean: true }),
    ]);
    let err = run_with(
        &gates,
        &ORDER,
        None,
        ScrapePriority::Background,
        &mut attempt,
    )
    .await
    .expect_err("background traffic does not trial an open breaker");
    assert!(matches!(err.error, AniError::NoResults));
    assert_eq!(attempt.asked(), vec![ProviderId::Hianime]);
}

/// The same half-open trial when nothing answered at all: a skipped
/// primary and an unreachable fallback leave an interactive walk with
/// a provider it never asked and a gate that would admit the click,
/// so the skipped one is tried before the request fails.
#[tokio::test]
async fn a_skipped_provider_is_retried_when_the_rest_were_unreachable_on_an_interactive_walk() {
    let gates = Gates::new();
    gates.open(ProviderId::Anidb);
    let mut attempt = Scripted::new(&[
        (ProviderId::Anidb, Behavior::Answer("anidb")),
        (
            ProviderId::Hianime,
            Behavior::Unreachable(|| AniError::Network),
        ),
    ]);
    let got = run_with(
        &gates,
        &ORDER,
        None,
        ScrapePriority::Interactive,
        &mut attempt,
    )
    .await
    .expect("the recovered primary answered");
    assert_eq!(got.provider, ProviderId::Anidb);
    assert_eq!(
        attempt.asked(),
        vec![ProviderId::Hianime, ProviderId::Anidb],
        "skipped first, asked last"
    );
}

/// A skipped provider asked last answers for itself: its miss is the
/// walk's verdict, as given, not the unreachable fallback's error.
#[tokio::test]
async fn a_retried_skipped_providers_miss_is_the_verdict_when_the_rest_were_unreachable() {
    let gates = Gates::new();
    gates.open(ProviderId::Anidb);
    let mut attempt = Scripted::new(&[
        (ProviderId::Anidb, Behavior::Miss { clean: true }),
        (
            ProviderId::Hianime,
            Behavior::Unreachable(|| AniError::Network),
        ),
    ]);
    let err = run_with(
        &gates,
        &ORDER,
        None,
        ScrapePriority::Interactive,
        &mut attempt,
    )
    .await
    .expect_err("the primary answered a miss");
    assert!(matches!(err.error, AniError::NoResults), "{:?}", err.error);
    assert!(err.clean_miss, "the miss stands as the primary gave it");
    assert_eq!(
        attempt.asked(),
        vec![ProviderId::Hianime, ProviderId::Anidb]
    );
}

#[tokio::test]
async fn a_skipped_provider_stays_skipped_when_the_rest_were_unreachable_on_a_background_walk() {
    let gates = Gates::new();
    gates.open(ProviderId::Anidb);
    let mut attempt = Scripted::new(&[
        (ProviderId::Anidb, Behavior::Answer("anidb")),
        (
            ProviderId::Hianime,
            Behavior::Unreachable(|| AniError::Network),
        ),
    ]);
    let err = run_with(
        &gates,
        &ORDER,
        None,
        ScrapePriority::Background,
        &mut attempt,
    )
    .await
    .expect_err("background traffic does not trial an open breaker");
    assert!(matches!(err.error, AniError::Network), "{:?}", err.error);
    assert_eq!(attempt.asked(), vec![ProviderId::Hianime]);
}
