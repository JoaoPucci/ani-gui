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
    Miss {
        clean: bool,
    },
    /// The show found, the episode not: the provider's own verdict
    /// on the episode.
    MissEpisode,
    Stall,
}

/// The attempt: what each provider does when asked, and who was asked.
struct Scripted {
    behavior: HashMap<ProviderId, Behavior>,
    asked: Mutex<Vec<ProviderId>>,
    /// The provider a miss is attributed to — what a negative row
    /// would name — as the walk reports it.
    answered_by: Option<ProviderId>,
}

impl Scripted {
    fn new(script: &[(ProviderId, Behavior)]) -> Self {
        Self {
            behavior: script.iter().cloned().collect(),
            asked: Mutex::new(Vec::new()),
            answered_by: None,
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
            Behavior::MissEpisode => Err(NativeError {
                error: AniError::EpisodeUnavailable,
                clean_miss: true,
                failed_at: None,
            }),
            Behavior::Stall => std::future::pending().await,
        }
    }

    fn missed_by(&mut self, provider: ProviderId) {
        self.answered_by = Some(provider);
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
    /// The provider answered a rate limit with a window: an
    /// advertised pause, which the gate keeps apart from the breaker.
    fn pause(&self, p: ProviderId) {
        self.of(p).record(
            crate::scraper::gate::ScrapeOutcome::RateLimited {
                retry_after: Some(Duration::from_secs(120)),
            },
            tokio::time::Instant::now(),
        );
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

/// An advertised rate-limit window is the provider refusing, as an
/// open breaker is: a walk asked during it is told to come back
/// later, so it moves on while another provider remains — on a
/// background walk too, where waiting inside the gate for the window
/// would spend the attempt budget before the fallback is asked.
#[tokio::test]
async fn a_provider_in_a_rate_limit_pause_is_skipped_while_another_remains() {
    for priority in [ScrapePriority::Interactive, ScrapePriority::Background] {
        let gates = Gates::new();
        gates.pause(ProviderId::Anidb);
        let mut attempt = Scripted::new(&[
            (ProviderId::Anidb, Behavior::Answer("must not be asked")),
            (ProviderId::Hianime, Behavior::Answer("from hianime")),
        ]);
        let got = run(&gates, priority, &mut attempt).await.expect("answered");
        assert_eq!(got.provider, ProviderId::Hianime, "{priority:?}");
        assert_eq!(attempt.asked(), [ProviderId::Hianime], "{priority:?}");
    }
}

#[tokio::test]
async fn the_last_provider_is_tried_even_in_a_rate_limit_pause() {
    let gates = Gates::new();
    gates.pause(ProviderId::Hianime);
    let mut attempt = Scripted::new(&[
        (
            ProviderId::Anidb,
            Behavior::Unreachable(|| AniError::Network),
        ),
        (ProviderId::Hianime, Behavior::Answer("from hianime")),
    ]);
    let got = run(&gates, ScrapePriority::Interactive, &mut attempt)
        .await
        .expect("the last provider is always tried");
    assert_eq!(got.provider, ProviderId::Hianime);
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

/// A skipped provider is owed its half-open trial on an interactive
/// walk, so the walk keeps an attempt budget in reserve for it: a
/// fallback that stalls is cut off at its attempt budget, not handed
/// the whole remainder, and the recovered primary gets its turn.
#[tokio::test(start_paused = true)]
async fn a_stalled_fallback_leaves_the_skipped_primary_its_trial() {
    let gates = Gates::new();
    gates.open(ProviderId::Anidb);
    let mut attempt = Scripted::new(&[
        (ProviderId::Anidb, Behavior::Answer("from anidb")),
        (ProviderId::Hianime, Behavior::Stall),
    ]);
    let started = tokio::time::Instant::now();
    let got = run(&gates, ScrapePriority::Interactive, &mut attempt)
        .await
        .expect("the recovered primary answered");
    assert_eq!(got.provider, ProviderId::Anidb);
    let waited = started.elapsed();
    assert!(
        waited >= Duration::from_secs(20) && waited < Duration::from_secs(21),
        "the fallback got one attempt budget, then the primary its trial: {waited:?}"
    );
    assert_eq!(
        attempt.asked(),
        vec![ProviderId::Hianime, ProviderId::Anidb],
        "skipped first, retried after the stall"
    );
}

/// A background walk never retries a skipped provider, so nothing is
/// held back for it: the fallback keeps the whole remainder.
#[tokio::test(start_paused = true)]
async fn a_background_walk_gives_a_stalled_fallback_the_whole_remainder() {
    let gates = Gates::new();
    gates.open(ProviderId::Anidb);
    let mut attempt = Scripted::new(&[
        (ProviderId::Anidb, Behavior::Answer("must not be asked")),
        (ProviderId::Hianime, Behavior::Stall),
    ]);
    let started = tokio::time::Instant::now();
    let err = run(&gates, ScrapePriority::Background, &mut attempt)
        .await
        .expect_err("the stall is the verdict");
    assert!(matches!(err.error, AniError::Timeout), "{:?}", err.error);
    let waited = started.elapsed();
    assert!(
        waited >= Duration::from_secs(60) && waited < Duration::from_secs(61),
        "the whole remainder went to the fallback: {waited:?}"
    );
    assert_eq!(attempt.asked(), vec![ProviderId::Hianime]);
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
        !fails_over(&AniError::EpisodeUnavailable),
        "an episode the provider does not carry is an answer too"
    );
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

    fn missed_by(&mut self, _provider: ProviderId) {}
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

mod failover_props {
    use super::fails_over;
    use crate::error::AniError;
    use proptest::prelude::*;

    /// An error a walk can meet, paired with whether it is the provider
    /// being unreachable, refusing or broken: transport failures, a
    /// timeout, a gate refusal, a rate limit with any hint, a page the
    /// parser no longer reads, and an upstream block — 403, 429 or a
    /// server error — move the walk on; an answer, whatever its kind,
    /// does not. The expectation is stated by that rule, not by the
    /// predicate under test.
    fn error_and_whether_it_fails_over() -> impl Strategy<Value = (AniError, bool)> {
        prop_oneof![
            any::<()>().prop_map(|()| (AniError::Network, true)),
            any::<()>().prop_map(|()| (AniError::Timeout, true)),
            any::<()>().prop_map(|()| (AniError::GateRefused, true)),
            prop::option::of(0u64..100_000)
                .prop_map(|retry_after_secs| (AniError::RateLimited { retry_after_secs }, true)),
            "[a-z ]{0,24}".prop_map(|detail| (AniError::ParseFailed { detail }, true)),
            (100u16..600).prop_map(|status| {
                let block = status == 403 || status == 429 || status >= 500;
                (AniError::Upstream { status }, block)
            }),
            any::<()>().prop_map(|()| (AniError::NoResults, false)),
            any::<()>().prop_map(|()| (AniError::Cache, false)),
            any::<()>().prop_map(|()| (AniError::Io, false)),
            any::<()>().prop_map(|()| (AniError::Config, false)),
            any::<()>().prop_map(|()| (AniError::Metadata, false)),
            any::<()>().prop_map(|()| (AniError::FfmpegMissing, false)),
        ]
    }

    proptest! {
        /// The walk moves on exactly for the unreachable, refusing and
        /// broken kinds, and never for an answer.
        #[test]
        fn the_walk_moves_on_exactly_for_the_unreachable_refusing_and_broken(
            (error, expected) in error_and_whether_it_fails_over(),
        ) {
            prop_assert_eq!(fails_over(&error), expected, "{:?}", error);
        }
    }
}

mod budget_props {
    use super::walk_budget;
    use proptest::prelude::*;
    use std::time::Duration;

    proptest! {
        /// Whatever the attempt gets, the providers still owed a trial
        /// keep one attempt budget each of what remains; the last
        /// attempt with nobody owed keeps the whole remainder; nothing
        /// is handed out once the total, or what is left after the
        /// reserve, is spent.
        #[test]
        fn the_owed_trials_keep_their_budget(
            total_secs in 1u64..120,
            attempt_secs in 1u64..60,
            elapsed_secs in 0u64..150,
            last in prop::bool::ANY,
            owed in 0usize..4,
        ) {
            let total = Duration::from_secs(total_secs);
            let attempt = Duration::from_secs(attempt_secs);
            let elapsed = Duration::from_secs(elapsed_secs);
            let overall = tokio::time::Instant::now() - elapsed;
            let remaining = total.saturating_sub(elapsed);
            let reserve = attempt * u32::try_from(owed).expect("small");
            let got = walk_budget(overall, total, attempt, last, owed);
            if remaining.is_zero() {
                prop_assert_eq!(got, None);
            } else if last && owed == 0 {
                // The clock moved between the two readings by at most
                // a few microseconds; compare loosely.
                let budget = got.expect("the remainder");
                prop_assert!(remaining.abs_diff(budget) < Duration::from_millis(50));
            } else if remaining <= reserve {
                prop_assert_eq!(got, None);
            } else {
                let budget = got.expect("an attempt");
                prop_assert!(budget <= attempt);
                prop_assert!(budget + reserve <= remaining + Duration::from_millis(50));
            }
        }
    }
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

mod miss_props {
    use super::firmer_verdict;
    use crate::commands::play_native_resolve::NativeError;
    use crate::error::AniError;
    use crate::scraper::provider::ProviderId;
    use proptest::prelude::*;

    /// A miss as a provider reports it: a title miss or an episode
    /// verdict, with either clean-miss flag.
    fn miss() -> impl Strategy<Value = NativeError> {
        (prop::bool::ANY, prop::bool::ANY).prop_map(|(episode, clean_miss)| NativeError {
            error: if episode {
                AniError::EpisodeUnavailable
            } else {
                AniError::NoResults
            },
            clean_miss,
            failed_at: None,
        })
    }

    fn is_episode(ne: &NativeError) -> bool {
        matches!(ne.error, AniError::EpisodeUnavailable)
    }

    fn provider() -> impl Strategy<Value = Option<ProviderId>> {
        prop_oneof![
            Just(None),
            Just(Some(ProviderId::Anidb)),
            Just(Some(ProviderId::Hianime)),
        ]
    }

    proptest! {
        /// The same rule over a verdict and its provider: whichever
        /// miss is kept, its own provider comes with it.
        #[test]
        fn the_kept_verdicts_provider_comes_with_it(
            earlier in miss(),
            earlier_by in provider(),
            later in miss(),
            later_by in provider(),
        ) {
            let keep_earlier = is_episode(&earlier) && !is_episode(&later);
            let expected_by = if keep_earlier { earlier_by } else { later_by };
            let expected_flag = if keep_earlier { earlier.clean_miss } else { later.clean_miss };
            let (kept, by) = firmer_verdict((earlier, earlier_by), (later, later_by));
            prop_assert_eq!(by, expected_by);
            prop_assert_eq!(kept.clean_miss, expected_flag);
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

/// A remembered provider that found the show but not the episode has
/// said something the rest of the order cannot unsay: a later title
/// miss means that provider lacks the show, not that the episode
/// verdict was wrong. The episode's verdict is the one the user sees.
#[tokio::test]
async fn a_remembered_providers_episode_verdict_outranks_a_later_title_miss() {
    let gates = Gates::new();
    let mut attempt = Scripted::new(&[
        (ProviderId::Hianime, Behavior::MissEpisode),
        (ProviderId::Anidb, Behavior::Miss { clean: true }),
    ]);
    let err = run_with(
        &gates,
        &[ProviderId::Hianime, ProviderId::Anidb],
        Some(ProviderId::Hianime),
        ScrapePriority::Interactive,
        &mut attempt,
    )
    .await
    .expect_err("nobody served the episode");
    assert!(
        matches!(err.error, AniError::EpisodeUnavailable),
        "{:?}",
        err.error
    );
    assert_eq!(
        attempt.asked(),
        vec![ProviderId::Hianime, ProviderId::Anidb],
        "the rest of the order was still asked"
    );
    assert_eq!(
        attempt.answered_by,
        Some(ProviderId::Hianime),
        "the kept verdict's provider travels with it"
    );
}

/// The same when the title miss comes from a skipped provider's
/// half-open trial: the episode verdict from the provider that was
/// tried first stands over the trial's title miss.
#[tokio::test]
async fn an_episode_verdict_outranks_a_retried_providers_title_miss() {
    let gates = Gates::new();
    gates.open(ProviderId::Anidb);
    let mut attempt = Scripted::new(&[
        (ProviderId::Anidb, Behavior::Miss { clean: true }),
        (ProviderId::Hianime, Behavior::MissEpisode),
    ]);
    let err = run_with(
        &gates,
        &ORDER,
        None,
        ScrapePriority::Interactive,
        &mut attempt,
    )
    .await
    .expect_err("nobody served the episode");
    assert!(
        matches!(err.error, AniError::EpisodeUnavailable),
        "{:?}",
        err.error
    );
    assert_eq!(
        attempt.asked(),
        vec![ProviderId::Hianime, ProviderId::Anidb]
    );
    assert_eq!(
        attempt.answered_by,
        Some(ProviderId::Hianime),
        "the episode verdict's provider, not the trial's"
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

// ── whose miss a verdict is ──────────────────────────────────────────

/// A miss is attributed to the provider whose miss it is. A skipped
/// primary retried on its half-open trial and found still unreachable
/// does not become the author of the fallback's verdict — named as
/// the primary's, the negative row would be served the moment its
/// breaker closed, hiding a show only the primary carries.
#[tokio::test]
async fn a_saved_miss_keeps_the_provider_that_produced_it_when_the_retried_one_is_unreachable() {
    let gates = Gates::new();
    gates.open(ProviderId::Anidb);
    let mut attempt = Scripted::new(&[
        (
            ProviderId::Anidb,
            Behavior::Unreachable(|| AniError::Network),
        ),
        (ProviderId::Hianime, Behavior::Miss { clean: true }),
    ]);
    let err = run_with(
        &gates,
        &ORDER,
        None,
        ScrapePriority::Interactive,
        &mut attempt,
    )
    .await
    .expect_err("the fallback's miss stands");
    assert!(matches!(err.error, AniError::NoResults), "{:?}", err.error);
    assert!(err.clean_miss);
    assert_eq!(
        attempt.asked(),
        vec![ProviderId::Hianime, ProviderId::Anidb]
    );
    assert_eq!(
        attempt.answered_by,
        Some(ProviderId::Hianime),
        "the miss is the fallback's, not the retried primary's"
    );
}

/// When the retried primary answers a miss of its own, that miss —
/// the last answer given — is the verdict, and it is the primary's.
#[tokio::test]
async fn a_retried_providers_own_miss_is_attributed_to_it() {
    let gates = Gates::new();
    gates.open(ProviderId::Anidb);
    let mut attempt = Scripted::new(&[
        (ProviderId::Anidb, Behavior::Miss { clean: true }),
        (ProviderId::Hianime, Behavior::Miss { clean: true }),
    ]);
    let err = run_with(
        &gates,
        &ORDER,
        None,
        ScrapePriority::Interactive,
        &mut attempt,
    )
    .await
    .expect_err("both missed");
    assert!(matches!(err.error, AniError::NoResults), "{:?}", err.error);
    assert_eq!(
        attempt.asked(),
        vec![ProviderId::Hianime, ProviderId::Anidb]
    );
    assert_eq!(attempt.answered_by, Some(ProviderId::Anidb));
}
