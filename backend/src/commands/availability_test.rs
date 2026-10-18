//! The availability probe's own tests: the cache rows it serves and
//! writes, the walk it runs across the providers, the warm's pacing,
//! and the batch lookup.

use super::*;
use crate::error::AniError;
use crate::scraper::provider::{BrowseHit, EpisodeRef, ProviderId};
use crate::scraper::table_provider::{Answer, Asked, TableProvider};
use std::sync::Arc;

// ── providers as tables ─────────────────────────────────────────────

/// The hits a pool of `(slug, title)` pairs names.
fn hits(entries: &[(&str, &str)]) -> Vec<BrowseHit> {
    entries
        .iter()
        .map(|(slug, title)| BrowseHit {
            slug: (*slug).to_string(),
            title: (*title).to_string(),
            kind: None,
            jname: None,
        })
        .collect()
}

/// Rows with these ids, numbered from 1.
fn numbered(ids: &[u64]) -> Vec<EpisodeRef> {
    ids.iter()
        .zip(1..)
        .map(|(id, number)| EpisodeRef {
            id: *id,
            number,
            number2: None,
            former_tag: None,
            title: None,
        })
        .collect()
}

/// A provider answering as `id` whose every search finds one show,
/// `(slug, title, badge)`, listing `rows`; each row answers the mode
/// question with `sub` and `dub`.
fn one_show(
    id: ProviderId,
    (slug, title, kind): (&str, &str, Option<&str>),
    rows: &[u64],
    sub: bool,
    dub: bool,
) -> TableProvider {
    let show = crate::scraper::provider::slug_numeric_id(slug).expect("a slug-shaped id");
    let hit = BrowseHit {
        slug: slug.to_string(),
        title: title.to_string(),
        kind: kind.map(str::to_string),
        jname: None,
    };
    rows.iter().fold(
        TableProvider::new()
            .answering_as(id)
            .search_default(vec![hit])
            .listing(show, numbered(rows)),
        |t, row| t.mode(*row, "sub", sub).mode(*row, "dub", dub),
    )
}

/// A provider answering as `id` that answers every search with
/// nothing.
pub(super) fn no_results(id: ProviderId) -> Arc<TableProvider> {
    Arc::new(TableProvider::new().answering_as(id))
}

/// A provider answering as `id` whose every request dies on
/// transport.
pub(super) fn unreachable(id: ProviderId) -> Arc<TableProvider> {
    Arc::new(TableProvider::unreachable().answering_as(id))
}

/// A provider answering as `id` whose every request answers `status`.
fn refusing(id: ProviderId, status: u16) -> Arc<TableProvider> {
    Arc::new(TableProvider::refusing(status).answering_as(id))
}

/// Whether `provider` was asked the mode question at all.
fn asked_the_mode(provider: &TableProvider) -> bool {
    provider.count(|a| matches!(a, Asked::Mode(..))) > 0
}

/// Whether a probe answered without a verdict: a normal answer whose
/// wire shape says `unsettled` — with the reason it settled nothing —
/// saying the show is not available and meaning nothing by it. Read as
/// the renderer reads it — from the serialized answer — since that
/// shape is what the route sends.
pub(super) fn unsettled(got: &Result<AvailabilityResponse>) -> bool {
    unsettled_reason(got).is_some()
}

/// The reason an unsettled answer gives, read from the wire.
pub(super) fn unsettled_reason(got: &Result<AvailabilityResponse>) -> Option<String> {
    let r = got.as_ref().ok()?;
    let wire = serde_json::to_value(r).expect("serializes");
    let reason = wire["unsettled"].as_str()?.to_string();
    (!r.available && r.episode_count.is_none()).then_some(reason)
}

/// [`check_availability`] with `table` answering in the slot of the
/// provider it answers as: the state's own cache, rows and gates, one
/// provider's answers swapped for the call.
pub(super) async fn check_with(
    state: &AppState,
    args: &AvailabilityArgs,
    table: &Arc<TableProvider>,
) -> Result<AvailabilityResponse> {
    let mut state = state.clone();
    state.providers.set_stub(Arc::clone(table));
    check_availability(&state, args).await
}

#[test]
fn warm_backoff_waits_out_the_advertised_rate_limit_window() {
    // A rate-limited probe must not be followed by another request
    // 500 ms later — the upstream said "try again in N seconds",
    // and continuing at full pace issues doomed requests that
    // deepen the limit while the user's click path competes for
    // the same budget.
    let outcome: Result<AvailabilityResponse> = Err(crate::error::AniError::RateLimited {
        retry_after_secs: Some(9),
    });
    assert_eq!(warm_backoff(&outcome), std::time::Duration::from_secs(9));
}

#[test]
fn warm_backoff_defaults_conservatively_when_the_hint_is_missing() {
    let outcome: Result<AvailabilityResponse> = Err(crate::error::AniError::RateLimited {
        retry_after_secs: None,
    });
    assert_eq!(warm_backoff(&outcome), std::time::Duration::from_secs(10));
}

#[test]
fn warm_backoff_keeps_the_pacing_gap_for_other_outcomes() {
    let ok: Result<AvailabilityResponse> = Ok(AvailabilityResponse {
        available: true,
        episode_count: None,
        episode_count_approximate: false,
        gate_refused: false,
        unsettled: None,
        provider: Some(crate::provider_set::ProviderSet::TEST_DEFAULT),
        extra_episodes: Vec::new(),
    });
    assert_eq!(warm_backoff(&ok), std::time::Duration::from_millis(500));
    let other: Result<AvailabilityResponse> = Err(crate::error::AniError::Network);
    assert_eq!(warm_backoff(&other), std::time::Duration::from_millis(500));
}

proptest::proptest! {
    // Pure-function contract: every advertised wait maps to
    // exactly that many seconds (floored at 1 so a "0 seconds"
    // answer can't turn the backoff into a hot loop).
    #[test]
    fn warm_backoff_honours_every_advertised_wait(n in 0u64..=100_000u64) {
        let outcome: Result<AvailabilityResponse> =
            Err(crate::error::AniError::RateLimited { retry_after_secs: Some(n) });
        let want = std::time::Duration::from_secs(n.max(1));
        proptest::prop_assert_eq!(warm_backoff(&outcome), want);
    }
}

/// The cache body is the AvailabilityResponse JSON. Adding the new
/// episode_count field must round-trip — the detail page reads it
/// to size the episode list and gate the Download All / range cap.
#[test]
fn response_round_trips_with_episode_count_and_extras() {
    let r = AvailabilityResponse {
        available: true,
        episode_count: Some(1160),
        episode_count_approximate: false,
        gate_refused: false,
        unsettled: None,
        provider: Some(crate::provider_set::ProviderSet::TEST_DEFAULT),
        extra_episodes: vec!["1061.5".into()],
    };
    let json = serde_json::to_string(&r).expect("serialize");
    let back: AvailabilityResponse = serde_json::from_str(&json).expect("deserialize");
    assert!(back.available);
    assert_eq!(back.episode_count, Some(1160));
    assert_eq!(back.extra_episodes, vec!["1061.5".to_string()]);
}

/// Old cache rows (written before the new fields existed) must
/// still parse — TTL hasn't expired them yet, so during the
/// rollout window we'll see them. They yield episode_count=None
/// and extra_episodes=[], which the frontend treats as "fall back
/// to Kitsu's count, no recap tiles".
#[test]
fn legacy_response_parses_with_defaults() {
    let legacy = r#"{"available":true}"#;
    let r: AvailabilityResponse = serde_json::from_str(legacy).expect("legacy parses");
    assert!(r.available);
    assert_eq!(r.episode_count, None);
    assert!(r.extra_episodes.is_empty());
}

/// `positive_ttl_for` is the load-bearing branch on Kitsu's
/// status string. Misclassifying a finished show as ongoing only
/// wastes a network probe; misclassifying an ongoing show as
/// finished would hide weekly drops for 30 days. Both directions
/// must be guarded.
// negative_ttl_for: the premiere-week fix. A show probed
// unavailable just before airing must not stay hidden for the
// full 7-day negative window after its first episode lands.
const NOW: u64 = 1_784_000_000;

#[test]
fn negative_ttl_finished_keeps_the_seven_day_window() {
    assert_eq!(
        negative_ttl_for(Some("finished"), Some(NOW + 3600), NOW),
        AVAILABILITY_TTL_NEGATIVE_SECS
    );
    assert_eq!(
        negative_ttl_for(Some("finished"), None, NOW),
        AVAILABILITY_TTL_NEGATIVE_SECS
    );
}

#[test]
fn negative_ttl_expires_shortly_after_a_known_premiere() {
    // Premiere in 2 days → negative dies 3h after it airs.
    let two_days = 2 * 24 * 60 * 60;
    assert_eq!(
        negative_ttl_for(Some("unreleased"), Some(NOW + two_days), NOW),
        two_days + 3 * 60 * 60
    );
}

#[test]
fn negative_ttl_floors_at_one_hour_when_the_premiere_passed() {
    // Stale airing row or the episode just dropped — re-probe soon.
    assert_eq!(
        negative_ttl_for(Some("current"), Some(NOW - 600), NOW),
        60 * 60
    );
}

#[test]
fn negative_ttl_current_show_caps_at_the_ongoing_window() {
    // Codex P2 #3565701272: for an already-premiered show,
    // next_airing_at points at the FOLLOWING episode — stretching
    // the negative to it would hide a catalog-lagged show for
    // almost a week after episode 1 appears. Current shows cap at
    // the 24h ongoing window; the schedule stretch is only for
    // pre-premiere rows.
    let five_days = 5 * 24 * 60 * 60;
    assert_eq!(
        negative_ttl_for(Some("current"), Some(NOW + five_days), NOW),
        AVAILABILITY_TTL_ONGOING_SECS
    );
}

#[test]
fn negative_ttl_unknown_status_caps_at_the_ongoing_window() {
    // Unknown status could be an already-airing show — same cap.
    let five_days = 5 * 24 * 60 * 60;
    assert_eq!(
        negative_ttl_for(None, Some(NOW + five_days), NOW),
        AVAILABILITY_TTL_ONGOING_SECS
    );
}

#[test]
fn negative_ttl_caps_at_seven_days_for_far_premieres() {
    let thirty_days = 30 * 24 * 60 * 60;
    assert_eq!(
        negative_ttl_for(Some("unreleased"), Some(NOW + thirty_days), NOW),
        AVAILABILITY_TTL_NEGATIVE_SECS
    );
}

#[test]
fn negative_ttl_unfinished_without_schedule_uses_the_ongoing_window() {
    assert_eq!(
        negative_ttl_for(Some("current"), None, NOW),
        AVAILABILITY_TTL_ONGOING_SECS
    );
    assert_eq!(
        negative_ttl_for(None, None, NOW),
        AVAILABILITY_TTL_ONGOING_SECS
    );
}

#[test]
fn positive_ttl_for_finished_returns_30_day_window() {
    assert_eq!(positive_ttl_for(Some("finished")), 30 * 24 * 60 * 60);
}

#[test]
fn positive_ttl_for_current_returns_24_hour_window() {
    // Kitsu's "current" maps to ongoing — the 24h window keeps
    // weekly drops visible within a day of upload.
    assert_eq!(positive_ttl_for(Some("current")), 24 * 60 * 60);
}

/// Build a minimal AppState backed by an in-memory SQLite pool —
/// enough for the cache-write / cache-read tests below. Anything
/// that touches Kitsu / the provider / fs would error out (the
/// `kitsu_base` is unreachable on purpose), but those paths
/// aren't exercised here.
/// The state's provider set has to reach the public entry point.
/// `check_availability` hard-coded `None` the same way
/// `pick_title_and_index` did, so an `AppState` pointed at a stub
/// still sent this endpoint's search to the real API — the exact
/// hole the override exists to close, left open on the caller that
/// fires on every detail-page mount and every rail warm.
#[tokio::test]
async fn check_availability_honours_the_state_level_override() {
    let server = no_results(ProviderId::TestA);
    let td = tempfile::tempdir().expect("tempdir");
    let mut state = cache_only_state(&td);
    state.providers.set_stub(Arc::clone(&server));

    let args = AvailabilityArgs {
        title: "Anything".into(),
        mode: "sub".into(),
        alt_titles: Vec::new(),
        episode_count: None,
        year: None,
        subtype: None,
        kitsu_id: None,
        status: None,
        background: false,
        bypass_cache: false,
    };
    let _ = check_availability(&state, &args).await;

    let hits = server.asked().len();
    assert!(
        hits >= 1,
        "the search must reach the state's stub, saw {hits} requests"
    );
}

/// One show: every search finds `probe-show-5`, whose listing
/// carries `count` episodes.
fn stub_one_show(count: u32) -> Arc<TableProvider> {
    let rows: Vec<u64> = (1..=u64::from(count)).map(|n| 5000 + n).collect();
    let last = *rows.last().expect("a listed show");
    Arc::new(
        TableProvider::new()
            .search_default(hits(&[("probe-show-5", "Probe Show")]))
            .listing(5, numbered(&rows))
            // The mode probe's answer for the listing's last row; the
            // rest are not served.
            .mode(last, "sub", true)
            .mode(last, "dub", false),
    )
}

#[tokio::test]
async fn the_response_names_the_provider_whose_catalogue_answered() {
    // "Available" needs an owner: the row says whose catalogue it
    // describes, in the identifier the renderer's `StreamProvider`
    // uses, and rows written before the field parse as unattributed
    // rather than failing.
    let rows: Vec<u64> = (5001..=5007).collect();
    let server = Arc::new(one_show(
        ProviderId::Hianime,
        ("probe-show-5", "Probe Show", None),
        &rows,
        true,
        false,
    ));
    let td = tempfile::tempdir().expect("td");
    let mut state = cache_only_state(&td);
    state.providers.set_order(vec![ProviderId::Hianime]);
    let args: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Probe Show",
        "mode": "sub",
        "kitsu_id": "556",
        "episode_count": 7
    }))
    .expect("args");
    let got = check_with(&state, &args, &server)
        .await
        .expect("probe succeeds");
    assert_eq!(got.provider, Some(ProviderId::Hianime));
    let body = serde_json::to_value(&got).expect("json");
    assert_eq!(body["provider"], "hianime");
    let legacy: AvailabilityResponse =
        serde_json::from_str(r#"{"available":true,"episode_count":7,"extra_episodes":[]}"#)
            .expect("a row from before the field");
    assert_eq!(legacy.provider, None);
}

#[tokio::test]
async fn the_cached_row_keeps_the_provider_that_answered() {
    // The response names its provider; so must the row it leaves
    // behind, or the next lookup — a cache hit — answers as a row
    // from before the field, and nothing can start a play from
    // the provider that proved the show playable.
    let server = stub_one_show(7);
    let td = tempfile::tempdir().expect("td");
    let state = cache_only_state(&td);
    let args: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Probe Show",
        "mode": "sub",
        "kitsu_id": "557",
        "episode_count": 7
    }))
    .expect("args");
    check_with(&state, &args, &server)
        .await
        .expect("probe succeeds");
    let row = meta_cache_get(&state.cache_pool, &cache_key("557", "sub"))
        .expect("cache read")
        .expect("row written");
    let row: AvailabilityResponse = serde_json::from_str(&row).expect("row parses");
    assert_eq!(
        row.provider,
        Some(ProviderId::TestA),
        "the row names the provider whose catalogue answered"
    );
}

/// A provider answering as `id` carrying one show whose episodes
/// offer sub only.
pub(super) fn stub_sub_only(id: ProviderId) -> Arc<TableProvider> {
    Arc::new(one_show(
        id,
        ("fallback-show-7", "Fallback Show", Some("TV")),
        &[7001, 7002],
        true,
        false,
    ))
}

pub(super) fn open_breaker(gate: &crate::scraper::gate::ScraperGate) {
    for _ in 0..crate::scraper::gate::FAILURE_THRESHOLD {
        gate.record(
            crate::scraper::gate::ScrapeOutcome::Failure,
            tokio::time::Instant::now(),
        );
    }
}

pub(super) fn close_breaker(gate: &crate::scraper::gate::ScraperGate) {
    gate.record(
        crate::scraper::gate::ScrapeOutcome::Success,
        tokio::time::Instant::now(),
    );
}

/// The provider answered a rate limit with a window: an
/// advertised pause, which the gate keeps apart from the breaker.
pub(super) fn pause_provider(gate: &crate::scraper::gate::ScraperGate) {
    gate.record(
        crate::scraper::gate::ScrapeOutcome::RateLimited {
            retry_after: Some(std::time::Duration::from_secs(120)),
        },
        tokio::time::Instant::now(),
    );
}

#[tokio::test]
async fn a_fallbacks_miss_during_the_primarys_outage_is_the_fallbacks_row_served_while_the_outage_lasts(
) {
    // The primary never answered; the fallback found the show and
    // has no dub of it. That is a verdict too — the fallback's —
    // and the row says so: served while the primary is down and
    // the fallback up, so the outage does not re-walk an absent
    // show on every look, and not once the primary is back, since
    // absence on the fallback proves nothing about it.
    let fallback = stub_sub_only(ProviderId::TestB);
    let td = tempfile::tempdir().expect("td");
    let mut state = cache_only_state(&td);
    state
        .providers
        .set_order(vec![ProviderId::TestA, ProviderId::TestB]);
    state.providers.set_stub(Arc::clone(&fallback));
    let args: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Fallback Show",
        "mode": "dub",
        "kitsu_id": "558",
        "episode_count": 2
    }))
    .expect("args");
    let got = check_with(&state, &args, &unreachable(ProviderId::TestA))
        .await
        .expect("the fallback answered");
    assert!(!got.available, "the fallback carries no dub");
    let row = meta_cache_get(&state.cache_pool, &cache_key("558", "dub"))
        .expect("cache read")
        .expect("the fallback's verdict is persisted");
    let row: AvailabilityResponse = serde_json::from_str(&row).expect("row parses");
    assert_eq!(
        row.provider,
        Some(ProviderId::TestB),
        "and named as the fallback's"
    );

    // The outage is on: the row is served, the lists hide.
    open_breaker(state.providers.gate(ProviderId::TestA));
    let listed = batch_cached(
        &state,
        &AvailabilityBatchArgs {
            kitsu_ids: vec!["558".into()],
            mode: "dub".into(),
        },
    );
    assert_eq!(listed.cached.get("558"), Some(&false));
    let asked_before = fallback.asked().len();
    check_with(&state, &args, &unreachable(ProviderId::TestA))
        .await
        .expect("served");
    let asked_after = fallback.asked().len();
    assert_eq!(
        asked_before, asked_after,
        "a cache hit asks the fallback nothing"
    );

    // The primary is back: the fallback's row no longer stands
    // for it.
    close_breaker(state.providers.gate(ProviderId::TestA));
    let listed = batch_cached(
        &state,
        &AvailabilityBatchArgs {
            kitsu_ids: vec!["558".into()],
            mode: "dub".into(),
        },
    );
    assert!(
        !listed.cached.contains_key("558"),
        "the fallback's negative is not served once the primary is reachable"
    );
}

#[tokio::test]
async fn a_fallbacks_clean_miss_during_the_primarys_outage_is_persisted_as_its_own() {
    let fallback = no_results(ProviderId::TestB);
    let td = tempfile::tempdir().expect("td");
    let mut state = cache_only_state(&td);
    state
        .providers
        .set_order(vec![ProviderId::TestA, ProviderId::TestB]);
    state.providers.set_stub(Arc::clone(&fallback));
    let args: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Nowhere Show",
        "mode": "sub",
        "kitsu_id": "563"
    }))
    .expect("args");
    let got = check_with(&state, &args, &unreachable(ProviderId::TestA))
        .await
        .expect("a clean miss on the fallback is an answer");
    assert!(!got.available);
    let row = meta_cache_get(&state.cache_pool, &cache_key("563", "sub"))
        .expect("cache read")
        .expect("persisted");
    let row: AvailabilityResponse = serde_json::from_str(&row).expect("row parses");
    assert_eq!(row.provider, Some(ProviderId::TestB));
}

#[tokio::test]
async fn a_fallbacks_clean_miss_stays_the_fallbacks_when_the_skipped_primary_is_retried_and_still_down(
) {
    // The primary's breaker is open, so the interactive probe
    // skips it; the fallback misses cleanly; the primary gets its
    // half-open trial and is still down. The verdict is the
    // fallback's miss, and the row has to name the fallback:
    // named as the primary's it would be served the moment the
    // breaker closed, for the rest of the negative TTL, hiding a
    // show only the primary carries.
    let fallback = no_results(ProviderId::TestB);
    let td = tempfile::tempdir().expect("td");
    let mut state = cache_only_state(&td);
    state
        .providers
        .set_order(vec![ProviderId::TestA, ProviderId::TestB]);
    state.providers.set_stub(Arc::clone(&fallback));
    open_breaker(state.providers.gate(ProviderId::TestA));
    let args: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Nowhere Show",
        "mode": "sub",
        "kitsu_id": "564"
    }))
    .expect("args");
    let got = check_with(&state, &args, &unreachable(ProviderId::TestA))
        .await
        .expect("a clean miss on the fallback is an answer");
    assert!(!got.available);
    let row = meta_cache_get(&state.cache_pool, &cache_key("564", "sub"))
        .expect("cache read")
        .expect("persisted");
    let row: AvailabilityResponse = serde_json::from_str(&row).expect("row parses");
    assert_eq!(
        row.provider,
        Some(ProviderId::TestB),
        "the miss is the fallback's, not the retried primary's"
    );
}

#[test]
fn a_negative_row_is_not_served_while_its_provider_is_in_a_rate_limit_pause() {
    // A rate limit with a window is the provider refusing, as an
    // open breaker is: a walk asked now is told to come back
    // later and moves on to the next provider, which may carry
    // the show. The primary's negative therefore stops standing
    // through the primary's pause, and the fallback's negative,
    // written during an outage, stands through it.
    let td = tempfile::tempdir().expect("td");
    let mut state = cache_only_state(&td);
    state
        .providers
        .set_order(vec![ProviderId::TestA, ProviderId::TestB]);
    // Both seen answering, so either can stand behind a row.
    close_breaker(state.providers.gate(ProviderId::TestA));
    close_breaker(state.providers.gate(ProviderId::TestB));
    write_cache(&state, "570", "sub", false, Some(ProviderId::TestA));
    write_cache(&state, "571", "sub", false, Some(ProviderId::TestB));
    let served = |state: &AppState| {
        let listed = batch_cached(
            state,
            &AvailabilityBatchArgs {
                kitsu_ids: vec!["570".into(), "571".into()],
                mode: "sub".into(),
            },
        );
        (
            listed.cached.contains_key("570"),
            listed.cached.contains_key("571"),
        )
    };
    assert_eq!(
        served(&state),
        (true, false),
        "both providers reachable: the primary's negative stands, the fallback's does not"
    );

    pause_provider(state.providers.gate(ProviderId::TestA));
    assert!(
        !state.providers.gate(ProviderId::TestA).is_open(),
        "a pause is not the breaker opening"
    );
    assert_eq!(
        served(&state),
        (false, true),
        "the primary pausing: its negative yields, the fallback's stands"
    );

    pause_provider(state.providers.gate(ProviderId::TestB));
    assert_eq!(
        served(&state),
        (false, false),
        "both pausing: nobody stands behind either row"
    );

    close_breaker(state.providers.gate(ProviderId::TestA));
    close_breaker(state.providers.gate(ProviderId::TestB));
    assert_eq!(
        served(&state),
        (true, false),
        "a fresh success on each ends its pause"
    );
}

/// The backing rule over every state the two gates can be in —
/// closed, breaker open, pausing, both — for a row from either
/// provider or from before the field: a row is served exactly
/// when its provider is not refusing and every provider ahead of
/// it is, and a pause counts as refusing wherever a breaker does;
/// a row from before the field names nobody and is never served.
/// Driven by hand on one state, whose gates are reset between
/// cases by the success that clears both a breaker and a pause.

#[tokio::test]
async fn a_reprobe_of_a_count_less_positive_row_starts_from_the_provider_it_remembers() {
    // A resolve through the fallback stamps a positive row without
    // a count, which the probe does not serve; the reprobe it runs
    // instead has to start from the provider that proved the show,
    // or the primary's clean miss ends the walk and overwrites the
    // fallback's proof with a negative that disables Play.
    let primary = no_results(ProviderId::TestA);
    let fallback = stub_sub_only(ProviderId::TestB);
    let td = tempfile::tempdir().expect("td");
    let mut state = cache_only_state(&td);
    state
        .providers
        .set_order(vec![ProviderId::TestA, ProviderId::TestB]);
    state.providers.set_stub(Arc::clone(&fallback));
    write_cache(&state, "580", "sub", true, Some(ProviderId::TestB));
    let args: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Fallback Show",
        "mode": "sub",
        "kitsu_id": "580"
    }))
    .expect("args");
    let got = check_with(&state, &args, &primary).await.expect("answered");
    assert!(
        got.available,
        "the provider the row remembers still carries the show"
    );
    assert_eq!(got.provider, Some(ProviderId::TestB));
    assert!(
        primary.asked().is_empty(),
        "the remembered provider answered; the primary was not asked"
    );
    assert_eq!(
        cached_provider(&state, "580", "sub"),
        Some(ProviderId::TestB),
        "the row keeps its provider"
    );
}

#[tokio::test]
async fn a_reprobe_keeps_the_positive_row_when_its_provider_is_down_and_the_rest_miss() {
    // The row's provider is unreachable for the reprobe, so the
    // walk moves on and the primary answers a clean miss. That
    // miss proves nothing about the show the fallback listed, and
    // neither would the title's absence from the catalogue: what
    // the probe reports is the fallback's own unavailability, and
    // nothing persists — the primary's negative would outlive the
    // fallback's recovery and keep a playable title disabled for
    // the negative's whole TTL.
    let primary = no_results(ProviderId::TestA);
    let fallback = refusing(ProviderId::TestB, 503);
    let td = tempfile::tempdir().expect("td");
    let mut state = cache_only_state(&td);
    state
        .providers
        .set_order(vec![ProviderId::TestA, ProviderId::TestB]);
    state.providers.set_stub(Arc::clone(&fallback));
    write_cache(&state, "580", "sub", true, Some(ProviderId::TestB));
    let args: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Fallback Show",
        "mode": "sub",
        "kitsu_id": "580"
    }))
    .expect("args");
    let got = check_with(&state, &args, &primary).await;
    assert!(
        matches!(&got, Err(e) if crate::commands::providers::fails_over(e)),
        "the fallback's own unavailability is the verdict the caller sees: {got:?}"
    );
    assert!(
        !primary.asked().is_empty(),
        "the walk moved past the unreachable provider to the primary"
    );
    assert_eq!(
        cached_provider(&state, "580", "sub"),
        Some(ProviderId::TestB),
        "the positive row stands until its provider is reachable again"
    );
}

#[tokio::test]
async fn a_skipped_primary_gets_its_trial_when_the_fallback_finds_the_show_without_the_mode() {
    // The primary's breaker is open, so the interactive probe
    // skips it and the fallback finds the show without a dub.
    // That is a negative verdict like a miss, and the gate would
    // admit the click through the open breaker as its half-open
    // trial: the primary is asked before the page is told the
    // title has no dub, and when it carries one, its answer is
    // the walk's. Without the trial the row would name the
    // fallback's absence and the page would disable playback for
    // the breaker's whole window on a show the primary dubs.
    let primary = Arc::new(one_show(
        ProviderId::TestA,
        ("fallback-show-72", "Fallback Show", None),
        &[721, 722],
        true,
        true,
    ));
    let fallback = stub_sub_only(ProviderId::TestB);
    let td = tempfile::tempdir().expect("td");
    let mut state = cache_only_state(&td);
    state
        .providers
        .set_order(vec![ProviderId::TestA, ProviderId::TestB]);
    state.providers.set_stub(Arc::clone(&fallback));
    open_breaker(state.providers.gate(ProviderId::TestA));
    let args: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Fallback Show",
        "mode": "dub",
        "kitsu_id": "581",
        "episode_count": 2
    }))
    .expect("args");
    let got = check_with(&state, &args, &primary)
        .await
        .expect("the primary's trial answered");
    assert!(got.available, "the primary carries the dub: {got:?}");
    assert_eq!(
        got.provider,
        Some(ProviderId::TestA),
        "the answer is the retried primary's"
    );
    assert!(
        asked_the_mode(&fallback),
        "the fallback was asked first and answered absence"
    );
    assert!(
        asked_the_mode(&primary),
        "the skipped primary got its trial"
    );
    let row = meta_cache_get(&state.cache_pool, &cache_key("581", "dub"))
        .expect("cache read")
        .expect("persisted");
    let row: AvailabilityResponse = serde_json::from_str(&row).expect("row parses");
    assert!(row.available, "the row is the primary's positive");
    assert_eq!(row.provider, Some(ProviderId::TestA));
}

#[tokio::test]
async fn a_remembered_providers_absence_yields_to_a_primary_that_carries_the_mode() {
    // A positive row remembers the fallback for the dub, so the
    // reprobe starts there; the fallback now lists the show with
    // its sub alone. That absence is the fallback's own denial
    // of the mode, but not the walk's verdict: the rest of the
    // order is asked, as after the fallback's episode dead end,
    // and the primary carries the dub. Ending the walk on the
    // fallback's absence would tell the page the title has no
    // dub and disable playback for the row's lifetime on a show
    // the primary dubs.
    let primary = Arc::new(one_show(
        ProviderId::TestA,
        ("fallback-show-73", "Fallback Show", None),
        &[731, 732],
        true,
        true,
    ));
    let fallback = stub_sub_only(ProviderId::TestB);
    let td = tempfile::tempdir().expect("td");
    let mut state = cache_only_state(&td);
    state
        .providers
        .set_order(vec![ProviderId::TestA, ProviderId::TestB]);
    state.providers.set_stub(Arc::clone(&fallback));
    write_cache(&state, "582", "dub", true, Some(ProviderId::TestB));
    let args: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Fallback Show",
        "mode": "dub",
        "kitsu_id": "582",
        "episode_count": 2
    }))
    .expect("args");
    let got = check_with(&state, &args, &primary)
        .await
        .expect("the primary answered the mode");
    assert!(got.available, "the primary carries the dub: {got:?}");
    assert_eq!(
        got.provider,
        Some(ProviderId::TestA),
        "the answer is the primary's"
    );
    assert!(
        asked_the_mode(&fallback),
        "the remembered provider was asked first and answered absence"
    );
    assert!(asked_the_mode(&primary), "the walk went on to the primary");
    let row = meta_cache_get(&state.cache_pool, &cache_key("582", "dub"))
        .expect("cache read")
        .expect("persisted");
    let row: AvailabilityResponse = serde_json::from_str(&row).expect("row parses");
    assert!(row.available, "the row is the primary's positive");
    assert_eq!(row.provider, Some(ProviderId::TestA));
}

#[tokio::test]
async fn a_reprobe_keeps_the_positive_row_when_its_provider_is_down_and_the_rest_lack_the_mode() {
    // The row's provider is unreachable for the reprobe, so the
    // walk moves on and the primary finds the show but answers
    // absence for the requested audio. Like the primary's clean
    // miss, that absence is the verdict the caller sees and
    // proves nothing about the audio the fallback listed:
    // persisting it as the primary's negative would stay backed
    // by a healthy primary and keep a playable title disabled
    // long after the fallback recovered.
    let primary = Arc::new(one_show(
        ProviderId::TestA,
        ("fallback-show-71", "Fallback Show", None),
        &[711, 712],
        true,
        false,
    ));
    let fallback = refusing(ProviderId::TestB, 503);
    let td = tempfile::tempdir().expect("td");
    let mut state = cache_only_state(&td);
    state
        .providers
        .set_order(vec![ProviderId::TestA, ProviderId::TestB]);
    state.providers.set_stub(Arc::clone(&fallback));
    write_cache(&state, "580", "dub", true, Some(ProviderId::TestB));
    let args: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Fallback Show",
        "mode": "dub",
        "kitsu_id": "580"
    }))
    .expect("args");
    let got = check_with(&state, &args, &primary).await;
    assert!(
        unsettled(&got),
        "the primary's absence is the verdict the caller sees, unpersisted and not an error: {got:?}"
    );
    assert!(
        asked_the_mode(&primary),
        "the walk moved past the unreachable provider and the primary answered the mode"
    );
    assert_eq!(
        cached_provider(&state, "580", "dub"),
        Some(ProviderId::TestB),
        "the positive row stands until its provider is reachable again"
    );
}

#[tokio::test]
async fn a_primarys_absence_persists_once_the_retried_row_provider_denies_the_show() {
    // A positive row remembers the fallback for the dub and the
    // fallback's breaker is open, so the interactive reprobe
    // skips it, the primary finds the show without a dub, and
    // the fallback's half-open trial then answers a clean miss:
    // the show it once listed is gone from its catalogue. Both
    // providers have now denied the mode. The primary's absence
    // is the verdict, and it persists as the primary's own
    // negative — held back, it would keep a stale positive row
    // and a page enabled for a mode nobody carries.
    let primary = Arc::new(one_show(
        ProviderId::TestA,
        ("fallback-show-74", "Fallback Show", None),
        &[741, 742],
        true,
        false,
    ));
    let fallback = no_results(ProviderId::TestB);
    let td = tempfile::tempdir().expect("td");
    let mut state = cache_only_state(&td);
    state
        .providers
        .set_order(vec![ProviderId::TestA, ProviderId::TestB]);
    state.providers.set_stub(Arc::clone(&fallback));
    write_cache(&state, "584", "dub", true, Some(ProviderId::TestB));
    open_breaker(state.providers.gate(ProviderId::TestB));
    let args: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Fallback Show",
        "mode": "dub",
        "kitsu_id": "584",
        "episode_count": 2
    }))
    .expect("args");
    let got = check_with(&state, &args, &primary)
        .await
        .expect("the primary's absence is the verdict");
    assert!(!got.available, "nobody carries the dub: {got:?}");
    assert_eq!(
        got.provider,
        Some(ProviderId::TestA),
        "the absence is the primary's own"
    );
    assert!(
        !fallback.searched().is_empty(),
        "the skipped row provider got its trial"
    );
    let row = meta_cache_get(&state.cache_pool, &cache_key("584", "dub"))
        .expect("cache read")
        .expect("a row");
    let row: AvailabilityResponse = serde_json::from_str(&row).expect("row parses");
    assert!(
        !row.available,
        "the positive row gave way to the primary's negative: {row:?}"
    );
    assert_eq!(row.provider, Some(ProviderId::TestA));
}

/// A primary that finds the show and lists two episodes whose rows
/// it no longer serves — every mode question answers not-found — so
/// the mode search touches only missing rows.
fn stub_stale_rows(provider: ProviderId, id: u64) -> Arc<TableProvider> {
    Arc::new(
        TableProvider::new()
            .answering_as(provider)
            .search_default(hits(&[(&format!("fallback-show-{id}"), "Fallback Show")]))
            .listing(id, numbered(&[id * 10 + 1, id * 10 + 2])),
    )
}

#[tokio::test]
async fn an_inconclusive_primary_moves_the_probe_on_to_a_fallback_that_carries_the_mode() {
    // The primary finds the show, but every episode row the mode
    // search touches answers nothing: stale ids the provider no
    // longer serves. That is not a verdict about the mode, so it
    // ends nothing — the walk moves on and the fallback, which
    // carries the show with the mode, answers. Stopping at the
    // primary would fail the check for a title the fallback
    // plays, every time the primary's rows go stale.
    let primary = stub_stale_rows(ProviderId::TestA, 75);
    let fallback = stub_sub_only(ProviderId::TestB);
    let td = tempfile::tempdir().expect("td");
    let mut state = cache_only_state(&td);
    state
        .providers
        .set_order(vec![ProviderId::TestA, ProviderId::TestB]);
    state.providers.set_stub(Arc::clone(&fallback));
    let args: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Fallback Show",
        "mode": "sub",
        "kitsu_id": "585",
        "episode_count": 2
    }))
    .expect("args");
    let got = check_with(&state, &args, &primary)
        .await
        .expect("the fallback answered");
    assert!(got.available, "the fallback carries the show: {got:?}");
    assert_eq!(
        got.provider,
        Some(ProviderId::TestB),
        "the answer is the fallback's"
    );
    assert!(
        asked_the_mode(&primary),
        "the primary's rows were asked and answered nothing"
    );
    let row = meta_cache_get(&state.cache_pool, &cache_key("585", "sub"))
        .expect("cache read")
        .expect("persisted");
    let row: AvailabilityResponse = serde_json::from_str(&row).expect("row parses");
    assert!(row.available);
    assert_eq!(row.provider, Some(ProviderId::TestB));
}

#[tokio::test]
async fn an_inconclusive_probe_everywhere_is_unknown_and_persists_nothing() {
    // Both providers find the show and neither's sampled rows
    // answer for the mode: nobody answered, the verdict is
    // unknown, and no row is written — a stale row costs a
    // re-probe, not a wrong cache row for its whole life.
    let primary = stub_stale_rows(ProviderId::TestA, 76);
    // The fallback finds the show too, and every row it is asked about
    // answers not-found.
    let fallback = Arc::new(
        TableProvider::new()
            .answering_as(ProviderId::TestB)
            .search_default(vec![BrowseHit {
                slug: "fallback-show-7".into(),
                title: "Fallback Show".into(),
                kind: Some("TV".into()),
                jname: None,
            }])
            .listing(7, numbered(&[7001, 7002])),
    );
    let td = tempfile::tempdir().expect("td");
    let mut state = cache_only_state(&td);
    state
        .providers
        .set_order(vec![ProviderId::TestA, ProviderId::TestB]);
    state.providers.set_stub(Arc::clone(&fallback));
    let args: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Fallback Show",
        "mode": "sub",
        "kitsu_id": "586",
        "episode_count": 2
    }))
    .expect("args");
    let got = check_with(&state, &args, &primary).await;
    assert_eq!(
        unsettled_reason(&got).as_deref(),
        Some("unanswered"),
        "nobody answered for the mode — unknown, not an error: {got:?}"
    );
    assert!(asked_the_mode(&fallback), "the fallback was asked too");
    assert!(
        meta_cache_get(&state.cache_pool, &cache_key("586", "sub"))
            .expect("cache read")
            .is_none(),
        "an unknown verdict persists nothing"
    );
}

#[tokio::test]
async fn an_inconclusive_primary_outranks_the_fallbacks_clean_miss() {
    // The primary finds the show, and every episode row the mode
    // search touches answers nothing; the fallback does not list
    // the show at all. The fallback's clean miss says nothing
    // about the mode where the primary found the show — a row
    // the probe did not sample may carry it — so the verdict is
    // unknown, and nothing is written: persisted as the
    // fallback's negative, it would be served while the primary
    // answers and hide a title the primary carries.
    let primary = stub_stale_rows(ProviderId::TestA, 77);
    let fallback = no_results(ProviderId::TestB);
    let td = tempfile::tempdir().expect("td");
    let mut state = cache_only_state(&td);
    state
        .providers
        .set_order(vec![ProviderId::TestA, ProviderId::TestB]);
    state.providers.set_stub(Arc::clone(&fallback));
    let args: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Fallback Show",
        "mode": "sub",
        "kitsu_id": "587",
        "episode_count": 2
    }))
    .expect("args");
    let got = check_with(&state, &args, &primary).await;
    assert!(
        unsettled(&got),
        "the verdict is unknown, not the fallback's miss and not an error: {got:?}"
    );
    assert!(!fallback.searched().is_empty(), "the fallback was asked");
    assert!(
        meta_cache_get(&state.cache_pool, &cache_key("587", "sub"))
            .expect("cache read")
            .is_none(),
        "an unknown verdict persists nothing"
    );
}

#[tokio::test]
async fn the_warm_reprobes_a_negative_row_nobody_stands_behind() {
    // The batch read already refuses to serve the fallback's
    // negative once the primary is answering again, and the lists
    // then send the id to the warm; the warm must not take the
    // same row for a fresh one, or the card stays visible until
    // the negative expires.
    let primary = no_results(ProviderId::TestA);
    let td = tempfile::tempdir().expect("td");
    let mut state = cache_only_state(&td);
    state
        .providers
        .set_order(vec![ProviderId::TestA, ProviderId::TestB]);
    state.providers.set_stub(Arc::clone(&primary));
    write_cache(&state, "590", "sub", false, Some(ProviderId::TestB));
    let state = std::sync::Arc::new(state);
    let args: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Nowhere Show",
        "mode": "sub",
        "kitsu_id": "590",
        "status": "finished"
    }))
    .expect("args");
    warm(std::sync::Arc::clone(&state), vec![args]).await;
    assert!(
        !primary.asked().is_empty(),
        "the warm re-probed the row nobody stands behind"
    );
    let row = meta_cache_get(&state.cache_pool, &cache_key("590", "sub"))
        .expect("cache read")
        .expect("persisted");
    let row: AvailabilityResponse = serde_json::from_str(&row).expect("row parses");
    assert_eq!(
        row.provider,
        Some(ProviderId::TestA),
        "the row is the primary's own verdict now"
    );
}

#[test]
fn a_negative_row_is_backed_exactly_when_its_provider_answers_and_those_ahead_refuse() {
    use crate::scraper::provider::ProviderId;
    let td = tempfile::tempdir().expect("td");
    let mut state = cache_only_state(&td);
    state
        .providers
        .set_order(vec![ProviderId::TestA, ProviderId::TestB]);
    let cases = (
        proptest::bool::ANY,
        proptest::bool::ANY,
        proptest::bool::ANY,
        proptest::bool::ANY,
        0u8..3,
    );
    proptest::test_runner::TestRunner::default()
        .run(
            &cases,
            |(a_broken, a_paused, b_broken, b_paused, row_provider)| {
                close_breaker(state.providers.gate(ProviderId::TestA));
                close_breaker(state.providers.gate(ProviderId::TestB));
                if a_broken {
                    open_breaker(state.providers.gate(ProviderId::TestA));
                }
                if a_paused {
                    pause_provider(state.providers.gate(ProviderId::TestA));
                }
                if b_broken {
                    open_breaker(state.providers.gate(ProviderId::TestB));
                }
                if b_paused {
                    pause_provider(state.providers.gate(ProviderId::TestB));
                }
                let provider = match row_provider {
                    0 => None,
                    1 => Some(ProviderId::TestA),
                    _ => Some(ProviderId::TestB),
                };
                let row = AvailabilityResponse {
                    available: false,
                    episode_count: None,
                    extra_episodes: Vec::new(),
                    episode_count_approximate: false,
                    gate_refused: false,
                    unsettled: None,
                    provider,
                };
                let a_refusing = a_broken || a_paused;
                let b_refusing = b_broken || b_paused;
                let expected = match provider {
                    None => false,
                    Some(ProviderId::TestA) => !a_refusing,
                    Some(ProviderId::TestB) => !b_refusing && a_refusing,
                    Some(other) => unreachable!("the cases draw no {other:?} row"),
                };
                proptest::prop_assert_eq!(negative_row_is_backed(&state, &row), expected);
                Ok(())
            },
        )
        .expect("the backing rule holds over every gate state");
}

#[test]
fn the_remembered_provider_is_read_off_a_positive_row_alone() {
    // A positive row names the provider that proved the show
    // playable; a play starts there. Negative rows, rows from
    // before the field and missing rows remember nothing.
    let td = tempfile::tempdir().expect("td");
    let state = cache_only_state(&td);
    let row = |available: bool, provider| AvailabilityResponse {
        available,
        episode_count: available.then_some(12),
        extra_episodes: Vec::new(),
        episode_count_approximate: false,
        gate_refused: false,
        unsettled: None,
        provider,
    };
    write_cache_full(
        &state,
        "kid-9",
        "sub",
        None,
        &row(true, Some(ProviderId::Hianime)),
    );
    write_cache_full(&state, "kid-10", "sub", None, &row(false, None));
    write_cache_full(&state, "kid-11", "sub", None, &row(true, None));
    assert_eq!(
        cached_provider(&state, "kid-9", "sub"),
        Some(ProviderId::Hianime)
    );
    assert_eq!(cached_provider(&state, "kid-9", "dub"), None, "per mode");
    assert_eq!(
        cached_provider(&state, "kid-10", "sub"),
        None,
        "negative rows"
    );
    assert_eq!(
        cached_provider(&state, "kid-11", "sub"),
        None,
        "unattributed rows"
    );
    assert_eq!(
        cached_provider(&state, "kid-12", "sub"),
        None,
        "missing rows"
    );
}

/// A primary whose search answers with nothing — the one answer
/// that proves absence.
fn stub_no_results() -> Arc<TableProvider> {
    no_results(ProviderId::TestA)
}

#[tokio::test]
async fn a_clean_miss_row_names_the_provider_that_answered_it() {
    // A negative row is one provider's verdict — a miss does not
    // fail over — so the row says whose, and the read side can
    // ask whether that provider is still there to stand behind it.
    let server = stub_no_results();
    let td = tempfile::tempdir().expect("td");
    let state = cache_only_state(&td);
    let args: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Nowhere Show",
        "mode": "sub",
        "kitsu_id": "559"
    }))
    .expect("args");
    let got = check_with(&state, &args, &server)
        .await
        .expect("a clean miss is an answer");
    assert!(!got.available);
    let row = meta_cache_get(&state.cache_pool, &cache_key("559", "sub"))
        .expect("cache read")
        .expect("the miss is persisted");
    let row: AvailabilityResponse = serde_json::from_str(&row).expect("row parses");
    assert_eq!(
        row.provider,
        Some(ProviderId::TestA),
        "the negative row names the provider whose miss it is"
    );
}

#[tokio::test]
async fn a_negative_row_is_not_served_while_its_provider_is_unreachable() {
    // The primary answered a clean miss and then went down. Its
    // negative row is the primary's verdict alone, and serving it
    // through the outage hides — from the lists and from the
    // page's play and download — a show the fallback carries, for
    // the row's whole lifetime, in the exact outage failover
    // exists for. An unattributed negative is nobody's verdict and is
    // not served either.
    let fallback = stub_sub_only(ProviderId::TestB);
    let td = tempfile::tempdir().expect("td");
    let mut state = cache_only_state(&td);
    state
        .providers
        .set_order(vec![ProviderId::TestA, ProviderId::TestB]);
    state.providers.set_stub(Arc::clone(&fallback));
    write_cache(&state, "560", "sub", false, Some(ProviderId::TestA));
    write_cache(&state, "561", "sub", false, None);
    for _ in 0..crate::scraper::gate::FAILURE_THRESHOLD {
        state.providers.gate(ProviderId::TestA).record(
            crate::scraper::gate::ScrapeOutcome::Failure,
            tokio::time::Instant::now(),
        );
    }
    let listed = batch_cached(
        &state,
        &AvailabilityBatchArgs {
            kitsu_ids: vec!["560".into(), "561".into()],
            mode: "sub".into(),
        },
    );
    assert!(
        !listed.cached.contains_key("560") && !listed.cached.contains_key("561"),
        "the lists do not hide on a verdict nobody can stand behind: {:?}",
        listed.cached
    );
    let args: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Fallback Show",
        "mode": "sub",
        "kitsu_id": "560",
        "episode_count": 2
    }))
    .expect("args");
    let got = check_with(&state, &args, &unreachable(ProviderId::TestA))
        .await
        .expect("the probe re-runs and the fallback answers");
    assert!(got.available, "the fallback carries the show");
    assert_eq!(got.provider, Some(ProviderId::TestB));
}

#[test]
fn a_negative_row_is_served_while_its_provider_is_reachable() {
    let td = tempfile::tempdir().expect("td");
    let state = cache_only_state(&td);
    // Reachable means seen answering: a fresh gate has answered
    // nothing, and a row served on it would stand on a provider
    // that may be down since before the process started.
    close_breaker(state.providers.gate(ProviderId::TestA));
    write_cache(&state, "562", "sub", false, Some(ProviderId::TestA));
    let listed = batch_cached(
        &state,
        &AvailabilityBatchArgs {
            kitsu_ids: vec!["562".into()],
            mode: "sub".into(),
        },
    );
    assert_eq!(listed.cached.get("562"), Some(&false));
}

#[tokio::test]
async fn a_native_probe_reports_the_exact_count_with_no_second_fetch() {
    // The pick already paid for the episodes list, so the cap is
    // exact and free: no approximate flag, no gate-refused
    // degradation — the mechanisms those flags existed for are
    // gone from this path.
    let server = stub_one_show(7);
    let td = tempfile::tempdir().expect("td");
    let state = cache_only_state(&td);
    let args: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Probe Show",
        "mode": "sub",
        "kitsu_id": "555",
        "episode_count": 7
    }))
    .expect("args");
    let got = check_with(&state, &args, &server)
        .await
        .expect("probe succeeds");
    assert!(got.available);
    assert_eq!(got.episode_count, Some(7));
    assert!(got.extra_episodes.is_empty());
    assert!(!got.episode_count_approximate);
    assert!(!got.gate_refused);

    // And the verdict round-trips through the cache.
    let cached = batch_cached(
        &state,
        &AvailabilityBatchArgs {
            kitsu_ids: vec!["555".into()],
            mode: "sub".into(),
        },
    );
    assert_eq!(cached.cached.get("555"), Some(&true));
}

#[tokio::test]
async fn a_native_probe_reports_the_listings_fractional_extras() {
    // The picked listing already names the playable fractional
    // rows (a recap under "2.5"); discarding them loses the
    // episode-strip entries the play path can launch, until some
    // other writer happens to overwrite the row. The listing is
    // paid for — the extras ride the same response and cache row
    // as the cap.
    let server = Arc::new(
        TableProvider::new()
            .hits("Recap Show", hits(&[("recap-show-31", "Recap Show")]))
            .listing(
                31,
                vec![
                    EpisodeRef {
                        id: 311,
                        number: 1,
                        number2: None,
                        former_tag: None,
                        title: None,
                    },
                    EpisodeRef {
                        id: 312,
                        number: 2,
                        number2: None,
                        former_tag: None,
                        title: None,
                    },
                    EpisodeRef {
                        id: 313,
                        number: 3,
                        number2: Some("2.5".into()),
                        former_tag: None,
                        title: None,
                    },
                    EpisodeRef {
                        id: 314,
                        number: 4,
                        number2: Some("3".into()),
                        former_tag: None,
                        title: None,
                    },
                ],
            )
            .mode(314, "sub", true)
            .mode(314, "dub", false),
    );
    let td = tempfile::tempdir().expect("td");
    let state = cache_only_state(&td);
    let args: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Recap Show",
        "mode": "sub",
        "kitsu_id": "558"
    }))
    .expect("args");
    let got = check_with(&state, &args, &server)
        .await
        .expect("probe succeeds");
    assert!(got.available);
    assert_eq!(
        got.extra_episodes,
        vec!["2.5".to_string()],
        "the fractional tag rides the response; the integer number2 is a re-display, not an extra"
    );
    // And the extras survive the cache round-trip.
    let row = cache_key("558", "sub");
    let stored = meta_cache_get(&state.cache_pool, &row)
        .expect("cache read")
        .expect("row written");
    let parsed: AvailabilityResponse = serde_json::from_str(&stored).expect("row parses");
    assert_eq!(parsed.extra_episodes, vec!["2.5".to_string()]);
}

#[tokio::test]
async fn one_probe_leaves_the_breaker_one_outcome() {
    // The walk succeeds and the mode probe then dies on the
    // provider. Recording the walk's success first resets the
    // failure run, so every such request leaves the counter at
    // one and the breaker never opens — background warmers keep
    // hammering a failing endpoint forever. One request must
    // contribute one final outcome.
    let server = Arc::new(
        TableProvider::new()
            .search_default(hits(&[("breaker-show-3", "Breaker Show")]))
            .listing(3, numbered(&[31, 32]))
            // The mode question — asked only by the mode probe — answers
            // with a block-shaped status.
            .mode(32, "sub", Answer::fails(|| AniError::Upstream { status: 503 })),
    );
    let td = tempfile::tempdir().expect("td");
    let state = cache_only_state(&td);
    // One short of open, so a single recorded failure opens it
    // and a success-then-failure pair does not.
    for _ in 0..crate::scraper::gate::FAILURE_THRESHOLD - 1 {
        state
            .providers
            .gate(ProviderId::TestA)
            .record_outcome(false, tokio::time::Instant::now());
    }
    let args: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Breaker Show",
        "mode": "sub",
    }))
    .expect("args");
    let got = check_with(&state, &args, &server).await;
    assert!(got.is_err(), "the mode probe's failure surfaces");
    assert!(
        state
            .providers
            .gate(ProviderId::TestA)
            .admit(crate::scraper::gate::ScrapePriority::Background)
            .await
            .is_err(),
        "the failure counted, so the breaker is open"
    );
}

#[tokio::test(start_paused = true)]
async fn the_whole_probe_stops_at_the_resolution_deadline() {
    // Searches and candidate listings answer near their own
    // per-request timeout. With several aliases the walk alone
    // can outrun the resolver's ceiling before the mode probe
    // starts, so the ceiling has to cover the whole probe.
    let server = Arc::new(TableProvider::new().delaying(std::time::Duration::from_secs(19)));
    let td = tempfile::tempdir().expect("td");
    let state = cache_only_state(&td);
    let args: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Slow Show",
        "mode": "sub",
        "alt_titles": ["a", "b", "c", "d", "e", "f"],
    }))
    .expect("args");
    let started = tokio::time::Instant::now();
    let got = check_with(&state, &args, &server).await;
    assert!(
        matches!(got, Err(crate::error::AniError::Timeout)),
        "a stalled probe ends as a timeout: {got:?}"
    );
    assert!(
        started.elapsed()
            <= crate::commands::play_native_resolve::RESOLVE_DEADLINE
                + std::time::Duration::from_secs(1),
        "and it ends at the ceiling, not after every alias"
    );
}

#[tokio::test]
async fn a_partially_dubbed_show_reports_the_mode_as_present() {
    // Dubs trail subs: a show four episodes in may carry eng rows
    // for the first two only. A first-row probe alone would cache
    // the full mode-independent cap under the dub key and enable
    // episodes that fail at playback — the cap must describe the
    // rows the requested mode actually has.
    let server = Arc::new(
        TableProvider::new()
            .hits(
                "Partial Dub Show",
                hits(&[("partial-dub-81", "Partial Dub Show")]),
            )
            .listing(81, numbered(&[811, 812, 813, 814]))
            .mode(811, "sub", true)
            .mode(811, "dub", true)
            .mode(812, "sub", true)
            .mode(812, "dub", true)
            .mode(813, "sub", true)
            .mode(813, "dub", false)
            .mode(814, "sub", true)
            .mode(814, "dub", false),
    );
    let td = tempfile::tempdir().expect("td");
    let state = cache_only_state(&td);
    let args: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Partial Dub Show",
        "mode": "dub",
        "kitsu_id": "559"
    }))
    .expect("args");
    let got = check_with(&state, &args, &server)
        .await
        .expect("probe succeeds");
    assert!(got.available, "two dubbed episodes exist");
    assert_eq!(
        got.episode_count,
        Some(4),
        "the cap is the listing's; which episodes carry the dub is \
         per-episode data availability does not claim"
    );

    // And the same show under sub keeps the full cap.
    let sub_args: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Partial Dub Show",
        "mode": "sub",
        "kitsu_id": "559"
    }))
    .expect("args");
    let sub = check_with(&state, &sub_args, &server)
        .await
        .expect("probe succeeds");
    assert!(sub.available);
    assert_eq!(sub.episode_count, Some(4));
}

#[tokio::test]
async fn a_sub_only_show_probed_for_dub_caches_unavailable() {
    // The walk's search and episode listing are mode-independent;
    // only a languages row says whether the requested audio
    // exists. A sub-only show asked for dub must not cache
    // available: the dub CTA it would enable fails at playback.
    // The provider ANSWERED absence for this mode, so the
    // negative verdict is cacheable — like the clean search miss.
    let server = Arc::new(one_show(
        ProviderId::TestA,
        ("sub-only-71", "Sub Only Show", None),
        &[711, 712],
        true,
        false,
    ));
    let td = tempfile::tempdir().expect("td");
    let state = cache_only_state(&td);
    let args: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Sub Only Show",
        "mode": "dub",
        "kitsu_id": "557"
    }))
    .expect("args");
    let got = check_with(&state, &args, &server)
        .await
        .expect("the provider answered; the verdict is a verdict");
    assert!(!got.available, "no eng row means no dub to enable");
    assert_eq!(got.episode_count, None);
    // The answered absence round-trips through the dub cache row.
    let cached = batch_cached(
        &state,
        &AvailabilityBatchArgs {
            kitsu_ids: vec!["557".into()],
            mode: "dub".into(),
        },
    );
    assert_eq!(cached.cached.get("557"), Some(&false));
}

#[tokio::test]
async fn a_continuation_entry_caps_in_per_entry_numbering() {
    // The provider numbers continuation cours cumulatively (TYBW's
    // fourth part lists 41 and 42). The cap the frontend gates the
    // strip with must speak Kitsu's per-entry numbering: 2 aired,
    // not a raw 42 that unlocks episodes that don't exist.
    let server = Arc::new(
        TableProvider::new()
            .hits("Sequel Show", hits(&[("sequel-show-9", "Sequel Show")]))
            .listing(
                9,
                vec![
                    EpisodeRef {
                        id: 941,
                        number: 41,
                        number2: None,
                        former_tag: None,
                        title: None,
                    },
                    EpisodeRef {
                        id: 942,
                        number: 42,
                        number2: None,
                        former_tag: None,
                        title: None,
                    },
                ],
            )
            .mode(942, "sub", true)
            .mode(942, "dub", false),
    );
    let td = tempfile::tempdir().expect("td");
    let state = cache_only_state(&td);
    let args: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Sequel Show",
        "mode": "sub",
    }))
    .expect("args");
    let got = check_with(&state, &args, &server)
        .await
        .expect("probe succeeds");
    assert!(got.available);
    assert_eq!(got.episode_count, Some(2));
}

#[tokio::test]
async fn a_transient_provider_failure_surfaces_network_and_writes_nothing() {
    let server = refusing(ProviderId::TestA, 500);
    let td = tempfile::tempdir().expect("td");
    let state = cache_only_state(&td);
    let args: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Down Show",
        "mode": "sub",
        "kitsu_id": "556"
    }))
    .expect("args");
    let got = check_with(&state, &args, &server).await;
    assert!(matches!(
        got,
        Err(crate::error::AniError::Network | crate::error::AniError::Upstream { .. })
    ));
    let cached = batch_cached(
        &state,
        &AvailabilityBatchArgs {
            kitsu_ids: vec!["556".into()],
            mode: "sub".into(),
        },
    );
    assert!(
        !cached.cached.contains_key("556"),
        "weather must not be persisted"
    );
}

pub(super) fn cache_only_state(td: &tempfile::TempDir) -> AppState {
    use crate::meta::kitsu::KitsuClient;
    use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
    use std::path::PathBuf;
    AppState {
        secret: AppSecret::random(),
        sessions: SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        host_budget: crate::proxy::host_budget::HostBudget::fresh(),
        meta_http: reqwest::Client::new(),
        proxy_origin: ProxyOrigin::new("127.0.0.1", 12_345),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path: td.path().join("history"),
        providers: crate::provider_set::ProviderSet::for_tests(),
        image_cache_dir: td.path().join("images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem cache pool"),
        kitsu: KitsuClient::with_base(reqwest::Client::new(), "http://127.0.0.1:1"),
        config_path: td.path().join("config.toml"),
        state_dir: PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    }
}

/// Like [`cache_only_state`], but with the Kitsu client pointed at
/// a live wiremock server so the airing-seed path can resolve the
/// anilist mapping.
fn state_with_kitsu(td: &tempfile::TempDir, kitsu_uri: &str) -> AppState {
    let mut state = cache_only_state(td);
    state.kitsu = crate::meta::kitsu::KitsuClient::with_base(reqwest::Client::new(), kitsu_uri);
    state
}

/// Yani Neko's mapping shape: anilist/anime only.
const KITSU_MAPPING_BODY: &str = r#"{
    "data": { "id": "50551", "type": "anime", "attributes": { "canonicalTitle": "Yani Neko" } },
    "included": [{
        "id": "1",
        "type": "mappings",
        "attributes": { "externalSite": "anilist/anime", "externalId": "207141" }
    }]
}"#;

const ANILIST_UNRELEASED_BODY: &str = r#"{"data":{"Media":{
    "status":"NOT_YET_RELEASED","episodes":12,
    "nextAiringEpisode":{"episode":1,"airingAt":1784215800}
}}}"#;

// Codex P2 #3566017944: the premiere-aware negative TTL only
// works when an airing:v2 row exists, but only the detail/play
// pages fetch the airing endpoint. When the FIRST negative probe
// for an upcoming title comes from the list-view warmer (or races
// detail's airingGet), negative_ttl_for saw None and wrote a 24h
// row that outlives a premiere hours away. Pre-premiere negatives
// must seed the airing cache before the TTL decision.

#[tokio::test]
async fn pre_premiere_negative_seeds_the_airing_cache() {
    use wiremock::matchers::{method, path};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/50551"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(KITSU_MAPPING_BODY))
        .mount(&kitsu)
        .await;
    let anilist = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(ANILIST_UNRELEASED_BODY))
        .mount(&anilist)
        .await;
    let td = tempfile::tempdir().expect("tempdir");
    let state = state_with_kitsu(&td, &kitsu.uri());
    assert_eq!(cached_next_airing_at(&state, "50551"), None);
    seed_airing_for_negative_with_base(
        &state,
        "50551",
        false,
        Some("upcoming"),
        Some(&anilist.uri()),
    )
    .await;
    assert_eq!(
        cached_next_airing_at(&state, "50551"),
        Some(1_784_215_800),
        "the seed must leave an airing:v2 row for negative_ttl_for to read"
    );
}

#[tokio::test]
async fn non_premiere_negative_skips_the_airing_fetch() {
    // Already-airing shows cap at the ongoing window regardless of
    // schedule (negative_ttl_current_show_caps_at_the_ongoing_window),
    // so spending a fetch here would buy nothing.
    let kitsu = wiremock::MockServer::start().await;
    let anilist = wiremock::MockServer::start().await;
    // No mounts: any request would 404 and, more importantly, the
    // received-requests assertion below would catch it.
    let td = tempfile::tempdir().expect("tempdir");
    let state = state_with_kitsu(&td, &kitsu.uri());
    seed_airing_for_negative_with_base(
        &state,
        "50551",
        false,
        Some("current"),
        Some(&anilist.uri()),
    )
    .await;
    assert!(kitsu
        .received_requests()
        .await
        .expect("recorded")
        .is_empty());
    assert_eq!(cached_next_airing_at(&state, "50551"), None);
}

#[tokio::test]
async fn positive_results_skip_the_airing_fetch() {
    let kitsu = wiremock::MockServer::start().await;
    let anilist = wiremock::MockServer::start().await;
    let td = tempfile::tempdir().expect("tempdir");
    let state = state_with_kitsu(&td, &kitsu.uri());
    seed_airing_for_negative_with_base(
        &state,
        "50551",
        true,
        Some("upcoming"),
        Some(&anilist.uri()),
    )
    .await;
    assert!(kitsu
        .received_requests()
        .await
        .expect("recorded")
        .is_empty());
}

/// `write_cache_full` is the load-bearing serializer for every
/// availability decision. A round-trip via `batch_cached` proves
/// the JSON shape parses back without losing the fields the
/// frontend depends on.
#[test]
fn write_cache_full_round_trips_via_batch_cached() {
    let td = tempfile::tempdir().expect("tempdir");
    let state = cache_only_state(&td);
    write_cache_full(
        &state,
        "kid-1",
        "sub",
        Some("current"),
        &AvailabilityResponse {
            available: true,
            episode_count: Some(1160),
            extra_episodes: vec!["1061.5".into()],
            episode_count_approximate: false,
            gate_refused: false,
            unsettled: None,
            provider: Some(crate::provider_set::ProviderSet::TEST_DEFAULT),
        },
    );
    let resp = batch_cached(
        &state,
        &AvailabilityBatchArgs {
            kitsu_ids: vec!["kid-1".into()],
            mode: "sub".into(),
        },
    );
    assert_eq!(resp.cached.get("kid-1"), Some(&true));
}

/// Empty kitsu_id is the legacy-caller signal "I don't know who
/// this is yet" — it must NOT write a row. Otherwise we'd pollute
/// the cache under the empty-string key.
#[test]
fn write_cache_full_skips_empty_kitsu_id() {
    let td = tempfile::tempdir().expect("tempdir");
    let state = cache_only_state(&td);
    write_cache_full(
        &state,
        "",
        "sub",
        None,
        &AvailabilityResponse {
            available: true,
            episode_count: Some(12),
            extra_episodes: Vec::new(),
            episode_count_approximate: false,
            gate_refused: false,
            unsettled: None,
            provider: Some(crate::provider_set::ProviderSet::TEST_DEFAULT),
        },
    );
    // No row in the cache → batch_cached returns an empty map
    // for any id we ask about.
    let resp = batch_cached(
        &state,
        &AvailabilityBatchArgs {
            kitsu_ids: vec!["".into(), "kid-1".into()],
            mode: "sub".into(),
        },
    );
    assert!(resp.cached.is_empty());
}

/// The simpler `write_cache` entry point delegates to
/// `write_cache_full` with `episode_count = None` and unknown
/// status. Two assertions matter: the row is written, and the
/// status path defaults to ongoing TTL (which means the row
/// shows up in batch_cached for at least 24h — what the home
/// page filter relies on).
#[test]
fn write_cache_round_trips_for_negative_results() {
    let td = tempfile::tempdir().expect("tempdir");
    let state = cache_only_state(&td);
    // A negative row is usable only on a provider seen answering.
    close_breaker(state.providers.gate(ProviderId::TestA));
    write_cache(
        &state,
        "kid-2",
        "dub",
        false,
        Some(crate::provider_set::ProviderSet::TEST_DEFAULT),
    );
    let resp = batch_cached(
        &state,
        &AvailabilityBatchArgs {
            kitsu_ids: vec!["kid-2".into()],
            mode: "dub".into(),
        },
    );
    assert_eq!(resp.cached.get("kid-2"), Some(&false));
}

/// Cross-mode cache isolation: writing a sub row must not
/// surface under dub lookup, since the provider can have a show
/// in one mode but not the other.
#[test]
fn batch_cached_keys_by_mode() {
    let td = tempfile::tempdir().expect("tempdir");
    let state = cache_only_state(&td);
    write_cache(
        &state,
        "kid-3",
        "sub",
        true,
        Some(crate::provider_set::ProviderSet::TEST_DEFAULT),
    );
    let dub_resp = batch_cached(
        &state,
        &AvailabilityBatchArgs {
            kitsu_ids: vec!["kid-3".into()],
            mode: "dub".into(),
        },
    );
    assert!(dub_resp.cached.is_empty());
    let sub_resp = batch_cached(
        &state,
        &AvailabilityBatchArgs {
            kitsu_ids: vec!["kid-3".into()],
            mode: "sub".into(),
        },
    );
    assert_eq!(sub_resp.cached.get("kid-3"), Some(&true));
}

/// Empty-string ids in the batch input must be skipped — the
/// frontend can pass them when a card hasn't been resolved yet,
/// and the backend mustn't return a "false" for the empty key
/// (which would short-circuit the caller's render decision).
#[test]
fn batch_cached_skips_empty_ids_in_input() {
    let td = tempfile::tempdir().expect("tempdir");
    let state = cache_only_state(&td);
    write_cache(
        &state,
        "kid-4",
        "sub",
        true,
        Some(crate::provider_set::ProviderSet::TEST_DEFAULT),
    );
    let resp = batch_cached(
        &state,
        &AvailabilityBatchArgs {
            kitsu_ids: vec!["".into(), "kid-4".into(), "".into()],
            mode: "sub".into(),
        },
    );
    assert_eq!(resp.cached.len(), 1);
    assert_eq!(resp.cached.get("kid-4"), Some(&true));
}

/// An entry with a count and no year whose every search answers with
/// other shows, one of them fitting the count: the pool is refused on
/// how the hit is named, which the walk reports as the dead end rather
/// than the clean miss. The probe answers that as what it is — nothing
/// established, nothing persisted — and not as an error the renderer
/// has to swallow: a normal answer that says `unsettled`, so a card
/// keeps rendering and a page keeps its lazy play.
#[tokio::test]
async fn a_refused_pool_for_an_undated_entry_answers_unsettled_and_persists_nothing() {
    let other = Arc::new(one_show(
        ProviderId::TestA,
        ("other-7", "Other", Some("TV")),
        &[
            7001, 7002, 7003, 7004, 7005, 7006, 7007, 7008, 7009, 7010, 7011, 7012,
        ],
        true,
        false,
    ));
    let td = tempfile::tempdir().expect("td");
    let state = cache_only_state(&td);
    let args: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Show: Arc",
        "mode": "sub",
        "kitsu_id": "591",
        "episode_count": 12,
        "subtype": "TV"
    }))
    .expect("args");
    let got = check_with(&state, &args, &other).await;
    assert_eq!(
        unsettled_reason(&got).as_deref(),
        Some("refused"),
        "a refusal by name is unsettled, says so, and is no error: {got:?}"
    );
    assert!(
        other.count(|a| matches!(a, Asked::Episodes(_))) > 0,
        "the fitting hit was probed before it was refused"
    );
    assert!(
        meta_cache_get(&state.cache_pool, &cache_key("591", "sub"))
            .expect("cache read")
            .is_none(),
        "a refusal by name persists nothing"
    );
}

/// A dated entry with a count is refused a hit no name vouches for —
/// the dated-name rule — and the answer says the refusal is by name,
/// as the count-alone rule's does; nothing is persisted.
#[tokio::test]
async fn a_refused_pool_for_a_dated_entry_answers_refused_and_persists_nothing() {
    let other = Arc::new(one_show(
        ProviderId::TestA,
        ("other-7", "Other", Some("TV")),
        &[
            7001, 7002, 7003, 7004, 7005, 7006, 7007, 7008, 7009, 7010, 7011, 7012,
        ],
        true,
        false,
    ));
    let td = tempfile::tempdir().expect("td");
    let state = cache_only_state(&td);
    let args: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Show: Arc",
        "mode": "sub",
        "kitsu_id": "592",
        "episode_count": 12,
        "year": 2016,
        "subtype": "TV"
    }))
    .expect("args");
    let got = check_with(&state, &args, &other).await;
    assert_eq!(
        unsettled_reason(&got).as_deref(),
        Some("refused"),
        "{got:?}"
    );
    assert!(
        meta_cache_get(&state.cache_pool, &cache_key("592", "sub"))
            .expect("cache read")
            .is_none(),
        "a refusal by name persists nothing"
    );
}

/// A pool whose candidate's listing answers not-found is the answered
/// dead end too, but no refusal by name: the dead listing may have been
/// the show's. The answer is unsettled for another reason, and the
/// renderer keeps the page's play for it.
#[tokio::test]
async fn a_pool_of_dead_listings_answers_unanswered_not_refused() {
    let dead = Arc::new(
        TableProvider::new()
            .answering_as(ProviderId::TestA)
            .search_default(hits(&[("ghost-show-9", "Ghost Show")])),
    );
    let td = tempfile::tempdir().expect("td");
    let state = cache_only_state(&td);
    let args: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Ghost Show",
        "mode": "sub",
        "kitsu_id": "593",
        "episode_count": 12
    }))
    .expect("args");
    let got = check_with(&state, &args, &dead).await;
    assert_eq!(
        unsettled_reason(&got).as_deref(),
        Some("unanswered"),
        "{got:?}"
    );
}

/// `cache_key` is what makes the schema bumps self-heal possible —
/// each bump's new prefix supersedes any prior-version row with
/// the same (kitsu_id, mode). Pin both shapes (sub/dub) so a typo
/// in the key generator gets caught immediately.
#[test]
fn cache_key_is_versioned_per_mode() {
    assert_eq!(cache_key("kid-1", "sub"), "availability:v21:kid-1:sub");
    assert_eq!(cache_key("kid-1", "dub"), "availability:v21:kid-1:dub");
}

/// A verdict stamped before the extras a provider appends after a
/// show's last episode were read as extras is not served: Noragami's
/// row can hold the listing's fourteen as its cap, the two OADs as
/// episodes 13 and 14.
#[test]
fn a_verdict_stamped_before_appended_extras_were_extras_is_not_read() {
    let current = cache_key("7881", "sub");
    assert!(
        !current.starts_with("availability:v19:"),
        "the key must have moved past v19: {current}"
    );
}

/// A verdict the picker stamped before a dated entry with a count
/// needed a name is not served: HenSemi (TV)'s row can hold Medaka Box
/// Abnormal's twelve episodes as its own, a same-year show whose count
/// merely fit, and nothing in a row says which hit it was stamped from.
#[test]
fn a_verdict_stamped_before_a_dated_counted_entry_needed_a_name_is_not_read() {
    let current = cache_key("6067", "sub");
    assert!(
        !current.starts_with("availability:v18:"),
        "the key must have moved past v18: {current}"
    );
}

/// A verdict the picker stamped before an entry with a count and no
/// year took only a hit named for it is not served: seven of Kitsu's
/// fourteen such entries were played as another show whose count fit
/// — Mo Du Zhe's row holds Girlfriend, Girlfriend's twelve episodes as
/// its own — and nothing in a row says whether its entry was dated.
#[test]
fn a_verdict_stamped_before_a_counted_undated_entry_needed_a_name_is_not_read() {
    let current = cache_key("45637", "sub");
    assert!(
        !current.starts_with("availability:v17:"),
        "the key must have moved past v17: {current}"
    );
}

/// A verdict the picker stamped before an entry with no count or year
/// took only a hit named as it, and before a single video needed its
/// name or its year, is not served: Dragon Ball Super: Ginga Patrol's
/// row holds Dragon Ball Super's 131 episodes as its own.
#[test]
fn a_verdict_stamped_before_names_vouched_for_countless_and_single_video_picks_is_not_read() {
    let current = cache_key("50496", "sub");
    assert!(
        !current.starts_with("availability:v15:"),
        "the key must have moved past v15: {current}"
    );
}

/// A verdict the picker stamped before a countless entry with a year
/// took only a hit its year or its name vouched for, and before the
/// probe head's evidence reached as far as its five, is not served: a
/// sequel's row can hold the series' listing as its own, and a pool
/// whose broad listing went uncut can hold a spinoff's.
#[test]
fn a_verdict_stamped_before_the_countless_year_vouched_or_the_evidence_reached_the_five_is_not_read(
) {
    let current = cache_key("50496", "sub");
    assert!(
        !current.starts_with("availability:v16:"),
        "the key must have moved past v16: {current}"
    );
}

#[test]
fn positive_ttl_for_unknown_or_missing_falls_back_to_ongoing() {
    // The "unknown status" branch has to favour ongoing — a
    // 30-day cap on a maybe-airing show is the worst failure
    // mode (silently stale episode count).
    for s in [
        None,
        Some("upcoming"),
        Some("tba"),
        Some("unreleased"),
        Some(""),
    ] {
        assert_eq!(
            positive_ttl_for(s),
            24 * 60 * 60,
            "expected ongoing TTL for status {s:?}"
        );
    }
}

/// The home Continue Watching card and the detail page must agree
/// on what "Continue" means, which means they must derive the same
/// `defaultEpisode` cap. The detail page reads `playableEpisodeCount`
/// (the provider's true count) via an inline `check_availability` probe;
/// the home strip can't afford a per-card probe, so the cap has to
/// ride out of the batch lookup it already does. The cache body
/// already stores `episode_count` per (kitsu_id, mode) — surface
/// it through `batch_cached` so the home page can derive the same
/// cap without an extra round trip.
#[test]
fn batch_cached_returns_playable_episode_counts() {
    let td = tempfile::tempdir().expect("tempdir");
    let state = cache_only_state(&td);
    // The negative row below stands on a provider seen answering.
    close_breaker(state.providers.gate(ProviderId::TestA));
    write_cache_full(
        &state,
        "ongoing-show",
        "sub",
        Some("current"),
        &AvailabilityResponse {
            available: true,
            episode_count: Some(1107),
            extra_episodes: Vec::new(),
            episode_count_approximate: false,
            gate_refused: false,
            unsettled: None,
            provider: Some(crate::provider_set::ProviderSet::TEST_DEFAULT),
        },
    );
    write_cache_full(
        &state,
        "finished-show",
        "sub",
        Some("finished"),
        &AvailabilityResponse {
            available: true,
            episode_count: Some(12),
            extra_episodes: Vec::new(),
            episode_count_approximate: false,
            gate_refused: false,
            unsettled: None,
            provider: Some(crate::provider_set::ProviderSet::TEST_DEFAULT),
        },
    );
    // Cached as unavailable — no playable count to surface.
    write_cache_full(
        &state,
        "blocked-show",
        "sub",
        None,
        &AvailabilityResponse {
            available: false,
            episode_count: None,
            extra_episodes: Vec::new(),
            episode_count_approximate: false,
            gate_refused: false,
            unsettled: None,
            provider: Some(crate::provider_set::ProviderSet::TEST_DEFAULT),
        },
    );
    let resp = batch_cached(
        &state,
        &AvailabilityBatchArgs {
            kitsu_ids: vec![
                "ongoing-show".into(),
                "finished-show".into(),
                "blocked-show".into(),
                "uncached-show".into(),
            ],
            mode: "sub".into(),
        },
    );
    // Existing `cached` map still works for the filter callers.
    assert_eq!(resp.cached.get("ongoing-show"), Some(&true));
    assert_eq!(resp.cached.get("finished-show"), Some(&true));
    assert_eq!(resp.cached.get("blocked-show"), Some(&false));
    // New field surfaces the playable count for entries that have
    // one. Unavailable + uncached entries don't appear.
    assert_eq!(
        resp.playable_episode_counts.get("ongoing-show"),
        Some(&1107)
    );
    assert_eq!(resp.playable_episode_counts.get("finished-show"), Some(&12));
    assert!(!resp.playable_episode_counts.contains_key("blocked-show"));
    assert!(!resp.playable_episode_counts.contains_key("uncached-show"));
}

/// A cached count is a snapshot. For an ongoing show it is kept
/// for 24 hours, and the provider adds episodes inside that window —
/// so a user clicking a tile the count says is unavailable is
/// asking a question the cache cannot answer. Serving the stored
/// number back is exactly the reply that makes the tile look
/// broken: it confirms what it was already showing, having asked
/// nobody.
///
/// `bypass_cache` is how a caller says "do not tell me what you
/// remember, go and look". The row is still written afterwards on
/// the normal path, so what comes back replaces the stale entry
/// for everything else that reads it.
#[tokio::test]
async fn bypass_cache_goes_to_the_network_despite_a_usable_row() {
    let server = Arc::new(TableProvider::new());
    let td = tempfile::tempdir().expect("td");
    let state = cache_only_state(&td);

    // Exactly what a mount-time probe leaves behind: available,
    // with a confirmed count. `cache_hit_is_usable` accepts it.
    write_cache_full(
        &state,
        "777",
        "sub",
        Some("current"),
        &AvailabilityResponse {
            available: true,
            episode_count: Some(4),
            extra_episodes: Vec::new(),
            episode_count_approximate: false,
            gate_refused: false,
            unsettled: None,
            provider: Some(crate::provider_set::ProviderSet::TEST_DEFAULT),
        },
    );

    let cached: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Bypass Test",
        "mode": "sub",
        "kitsu_id": "777"
    }))
    .expect("args");
    let served = check_with(&state, &cached, &server).await;
    assert_eq!(
        served.expect("cached row is served").episode_count,
        Some(4),
        "without the flag the stored snapshot answers"
    );
    assert!(
        server.asked().is_empty(),
        "a usable row must not reach the network"
    );

    let fresh: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Bypass Test",
        "mode": "sub",
        "kitsu_id": "777",
        "bypass_cache": true
    }))
    .expect("args");
    let _ = check_with(&state, &fresh, &server).await;
    assert!(
        !server.asked().is_empty(),
        "bypass_cache must reach the provider rather than replay the stored count"
    );
}

#[tokio::test]
async fn a_lookup_leaves_the_row_alone_when_a_refresh_answered_first() {
    // The wiring, not the rule. `may_write_cache` is unit-tested on
    // its own; this drives the whole lookup, concurrently, because
    // the generation is captured INSIDE the call — the refresh has
    // to land while this one is still out for the guard to mean
    // anything.
    let server = Arc::new(TableProvider::new().delaying(std::time::Duration::from_millis(300)));
    let td = tempfile::tempdir().expect("td");
    let state = cache_only_state(&td);
    let row = cache_key("888", "sub");

    // No row yet: this lookup will reach the network and, left to
    // itself, install a negative one.
    let stale: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Ordering Test",
        "mode": "sub",
        "kitsu_id": "888"
    }))
    .expect("args");
    let bg = {
        let state = state.clone();
        let server = Arc::clone(&server);
        tokio::spawn(async move {
            let _ = check_with(&state, &stale, &server).await;
        })
    };

    // The user's re-ask answers while that one is still in flight.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    state.availability_refreshes.bump(&row);
    write_cache_full(
        &state,
        "888",
        "sub",
        Some("current"),
        &AvailabilityResponse {
            available: true,
            episode_count: Some(9),
            extra_episodes: Vec::new(),
            episode_count_approximate: false,
            gate_refused: false,
            unsettled: None,
            provider: Some(crate::provider_set::ProviderSet::TEST_DEFAULT),
        },
    );

    bg.await.expect("the background lookup finishes");

    let stored = meta_cache_get(&state.cache_pool, &row)
        .expect("cache read")
        .expect("the refresh's row is still there");
    let kept: AvailabilityResponse = serde_json::from_str(&stored).expect("row parses");
    assert!(
        kept.available && kept.episode_count == Some(9),
        "an older lookup must not replace the refresh's row, got {kept:?}"
    );
}

#[tokio::test]
async fn a_lookup_holds_the_row_while_it_decides_whether_to_write() {
    // Reading the generation and writing the row are two steps, and
    // the lookup can be overtaken between them — the counter says
    // "clear" and by the time the write lands it no longer is. So
    // the reading has to be taken and used under the same lock, and
    // that is what this pins: while the row is held, a lookup that
    // has already got its answer must still be waiting.
    let server = Arc::new(TableProvider::new());
    let td = tempfile::tempdir().expect("td");
    let state = cache_only_state(&td);
    let row = cache_key("889", "sub");

    // Stand in for the refresh that is mid-section: it owns the row
    // and has not written yet.
    let held = state.availability_refreshes.for_row(&row);
    let guard = held.lock().await;

    let args: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Section Test",
        "mode": "sub",
        "kitsu_id": "889"
    }))
    .expect("args");
    let bg = {
        let state = state.clone();
        let server = Arc::clone(&server);
        tokio::spawn(async move {
            let _ = check_with(&state, &args, &server).await;
        })
    };

    // Long enough for the unguarded lookup to finish its round-trip
    // and write. Nothing may be in the row: the answer is in hand,
    // but the decision about it is not this lookup's to make yet.
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    assert!(
        meta_cache_get(&state.cache_pool, &row)
            .expect("cache read")
            .is_none(),
        "a lookup wrote the row while another writer held it"
    );

    // Now the refresh finishes: its row lands, and the count says
    // so. The waiting lookup must see both.
    state.availability_refreshes.bump(&row);
    write_cache_full(
        &state,
        "889",
        "sub",
        Some("current"),
        &AvailabilityResponse {
            available: true,
            episode_count: Some(4),
            extra_episodes: Vec::new(),
            episode_count_approximate: false,
            gate_refused: false,
            unsettled: None,
            provider: Some(crate::provider_set::ProviderSet::TEST_DEFAULT),
        },
    );
    drop(guard);

    bg.await.expect("the background lookup finishes");

    let stored = meta_cache_get(&state.cache_pool, &row)
        .expect("cache read")
        .expect("the refresh's row is still there");
    let kept: AvailabilityResponse = serde_json::from_str(&stored).expect("row parses");
    assert!(
        kept.available && kept.episode_count == Some(4),
        "the lookup released to find a fresher row and overwrote it anyway, got {kept:?}"
    );
}

#[tokio::test]
async fn background_probe_skips_the_network_while_the_breaker_is_open() {
    // Once the breaker is open, a background probe must not touch
    // the provider at all — the provider's gate refuses before any
    // request, the typed refusal surfaces, and the row stays
    // unwritten. (That a real client is wired to that gate is
    // `providers_test`'s to pin.)
    let server = Arc::new(TableProvider::new());
    let td = tempfile::tempdir().expect("td");
    let state = cache_only_state(&td);
    for _ in 0..crate::scraper::gate::FAILURE_THRESHOLD {
        state
            .providers
            .gate(ProviderId::TestA)
            .record_outcome(false, tokio::time::Instant::now());
    }
    let args: AvailabilityArgs = serde_json::from_value(serde_json::json!({
        "title": "Gate Test",
        "mode": "sub",
        "kitsu_id": "999",
        "background": true
    }))
    .expect("args");
    let got = check_with(&state, &args, &server).await;
    assert!(matches!(got, Err(crate::error::AniError::GateRefused)));
    assert!(
        server.asked().is_empty(),
        "an open breaker must produce zero provider requests"
    );
}

/// The gate-refused fallback caches the search hit's count, which
/// runs one high for shows carrying half episodes. The cache
/// short-circuit returns any numeric row before the interactive
/// lane is even considered, so that degraded value is served back
/// to every later caller — including a click trying to confirm
/// the cap before advancing onto it. Confirmation against a cache
/// that replays the same approximation confirms nothing.
///
/// So an approximate row is not a usable hit. It self-heals the
/// same way legacy count-less rows do: the next read re-probes,
/// and once the gate allows the detail fetch the exact count
/// replaces it.
#[test]
fn an_approximate_cached_count_is_not_a_usable_hit() {
    let td = tempfile::tempdir().expect("td");
    let state = cache_only_state(&td);
    let exact = AvailabilityResponse {
        available: true,
        episode_count: Some(12),
        extra_episodes: Vec::new(),
        episode_count_approximate: false,
        gate_refused: false,
        unsettled: None,
        provider: Some(crate::provider_set::ProviderSet::TEST_DEFAULT),
    };
    assert!(
        cache_hit_is_usable(&state, &exact),
        "an exact count is usable"
    );

    let approximate = AvailabilityResponse {
        episode_count_approximate: true,
        gate_refused: false,
        unsettled: None,
        provider: Some(crate::provider_set::ProviderSet::TEST_DEFAULT),
        ..exact.clone()
    };
    assert!(
        !cache_hit_is_usable(&state, &approximate),
        "an approximate count must re-probe rather than be served"
    );
}

/// The pre-existing self-healing rules are unchanged: an
/// available row with no count re-probes, and a negative row is
/// kept as-is because episode_count is meaningless with no
/// candidate.
#[test]
fn usable_hit_rules_for_legacy_and_negative_rows() {
    let td = tempfile::tempdir().expect("td");
    let state = cache_only_state(&td);
    // A negative row is usable only on a provider seen answering.
    close_breaker(state.providers.gate(ProviderId::TestA));
    let legacy = AvailabilityResponse {
        available: true,
        episode_count: None,
        extra_episodes: Vec::new(),
        episode_count_approximate: false,
        gate_refused: false,
        unsettled: None,
        provider: Some(crate::provider_set::ProviderSet::TEST_DEFAULT),
    };
    assert!(
        !cache_hit_is_usable(&state, &legacy),
        "a count-less row re-probes"
    );

    let negative = AvailabilityResponse {
        available: false,
        episode_count: None,
        extra_episodes: Vec::new(),
        episode_count_approximate: false,
        gate_refused: false,
        unsettled: None,
        provider: Some(crate::provider_set::ProviderSet::TEST_DEFAULT),
    };
    assert!(
        cache_hit_is_usable(&state, &negative),
        "a negative row is kept"
    );

    // Nobody's verdict: neither a positive nor a negative that names
    // no provider is served.
    for available in [true, false] {
        let unattributed = AvailabilityResponse {
            available,
            episode_count: Some(12),
            provider: None,
            ..negative.clone()
        };
        assert!(
            !cache_hit_is_usable(&state, &unattributed),
            "a row naming no provider re-probes (available: {available})"
        );
    }
}

/// The rule over the whole input space rather than the four
/// corners the examples spell: a row is servable exactly when it
/// is negative — and, on the state these run against, backed by
/// a reachable primary — or carries a count that the backend
/// confirmed. Anything else re-probes.
///
/// Stated as an equivalence, not a re-implementation — the point
/// is that no combination of the three inputs, and no count
/// value, reaches a fifth outcome. Driven by hand so the state,
/// whose pool spawns a reaper thread, is built once for every
/// case rather than once per case: macOS's thread ceiling is
/// lower than a property's case count.
#[test]
fn usability_is_negative_or_a_confirmed_count() {
    let td = tempfile::tempdir().expect("td");
    let state = cache_only_state(&td);
    // A negative row is usable only on a provider seen answering.
    close_breaker(state.providers.gate(ProviderId::TestA));
    let cases = (
        proptest::bool::ANY,
        proptest::option::of(0u32..2000),
        proptest::bool::ANY,
        proptest::collection::vec("[0-9]{1,3}(\\.5)?", 0..4),
    );
    proptest::test_runner::TestRunner::default()
        .run(&cases, |(available, count, approximate, extras)| {
            let row = AvailabilityResponse {
                available,
                episode_count: count,
                extra_episodes: extras,
                episode_count_approximate: approximate,
                gate_refused: false,
                unsettled: None,
                provider: Some(crate::provider_set::ProviderSet::TEST_DEFAULT),
            };
            proptest::prop_assert_eq!(
                cache_hit_is_usable(&state, &row),
                !available || (count.is_some() && !approximate)
            );
            Ok(())
        })
        .expect("the equivalence holds over the input space");
}

/// The safety direction on its own, because it is the one that
/// costs a user a phantom episode: an AVAILABLE row that is
/// approximate, or has no count, is never served. A click asking
/// the cache to confirm a cap must not be answered with the same
/// unconfirmed number it is questioning.
#[test]
fn an_unconfirmed_available_row_is_never_served() {
    let td = tempfile::tempdir().expect("td");
    let state = cache_only_state(&td);
    let cases = (proptest::option::of(0u32..2000), proptest::bool::ANY);
    proptest::test_runner::TestRunner::default()
        .run(&cases, |(count, approximate)| {
            proptest::prop_assume!(approximate || count.is_none());
            let row = AvailabilityResponse {
                available: true,
                episode_count: count,
                extra_episodes: Vec::new(),
                episode_count_approximate: approximate,
                gate_refused: false,
                unsettled: None,
                provider: Some(crate::provider_set::ProviderSet::TEST_DEFAULT),
            };
            proptest::prop_assert!(!cache_hit_is_usable(&state, &row));
            Ok(())
        })
        .expect("an unconfirmed available row is never served");
}

/// The flag has to survive the cache round-trip, or the re-probe
/// rule never fires in production.
#[test]
fn write_cache_full_round_trips_the_approximate_flag() {
    let td = tempfile::tempdir().expect("tempdir");
    let state = cache_only_state(&td);
    write_cache_full(
        &state,
        "kid-approx",
        "sub",
        Some("current"),
        &AvailabilityResponse {
            available: true,
            episode_count: Some(13),
            extra_episodes: Vec::new(),
            episode_count_approximate: true,
            gate_refused: false,
            unsettled: None,
            provider: Some(crate::provider_set::ProviderSet::TEST_DEFAULT),
        },
    );
    let raw = meta_cache_get(&state.cache_pool, &cache_key("kid-approx", "sub"))
        .expect("cache read")
        .expect("row present");
    let parsed: AvailabilityResponse = serde_json::from_str(&raw).expect("parse");
    assert!(parsed.episode_count_approximate, "the flag must persist");
    assert!(
        !cache_hit_is_usable(&state, &parsed),
        "and drive the re-probe"
    );
}

/// A row written before the flag existed has no such key, and
/// serde must read it as exact rather than refusing to parse.
#[test]
fn a_legacy_row_without_the_flag_parses_as_exact() {
    let td = tempfile::tempdir().expect("td");
    let state = cache_only_state(&td);
    // A row from before the flag, naming the provider the state walks.
    let parsed: AvailabilityResponse = serde_json::from_value(serde_json::json!({
        "available": true,
        "episode_count": 12,
        "provider": crate::provider_set::ProviderSet::TEST_DEFAULT,
    }))
    .expect("parse");
    assert!(!parsed.episode_count_approximate);
    assert!(cache_hit_is_usable(&state, &parsed));
}

/// A verdict stamped before a continuation's offset recap, and a
/// stitched recap before a cumulatively numbered part, were advertised
/// as the entry's 0.5 and 12.5 is not served: master's v20 rows carry
/// the old extras ("12.5" and "24.5"), and the probe that replaces them
/// is what puts a history row's stamp from then right. The key sits
/// above the one master moved to for the appended extras.
#[test]
fn a_verdict_stamped_before_offset_recaps_were_renumbered_is_not_read() {
    let current = cache_key("7881", "sub");
    assert!(
        !current.starts_with("availability:v20:"),
        "the key must have moved past master's v20: {current}"
    );
}
