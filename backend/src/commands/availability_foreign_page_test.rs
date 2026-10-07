//! anidb.app's search redirected to an unrelated site, and its home
//! page was read as an empty search: a clean miss, so the walk
//! stopped at anidb, never asked the fallback, and wrote anidb's
//! negative row. Mounted by `#[path]` beside the availability tests
//! and borrowing their state builder, gate helpers and hianime stub.
//!
//! The probe half: a redirected search is no answer, and the walk
//! moves on. The cache half: the rows the broken reading wrote carry
//! nothing that tells them from a genuine miss, so an anidb negative
//! written before the guard is not served at all — the next look
//! probes and writes a row the guarded reading stands behind.

use super::tests::{cache_only_state, close_breaker, open_breaker, stub_hianime_sub_only};
use super::*;
use crate::scraper::provider::ProviderId;

/// An anidb negative exactly as the build before the guard stored
/// it.
const PRE_GUARD_ANIDB_NEGATIVE: &str = r#"{"available":false,"episode_count":null,"extra_episodes":[],"episode_count_approximate":false,"provider":"anidb"}"#;

/// The same, as hianime's verdict.
const PRE_GUARD_HIANIME_NEGATIVE: &str = r#"{"available":false,"episode_count":null,"extra_episodes":[],"episode_count_approximate":false,"provider":"hianime"}"#;

const NEGATIVE_TTL: u64 = 7 * 24 * 60 * 60;

fn fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root")
        .join("tests/fixtures/anidb")
        .join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn listed(state: &AppState, id: &str) -> Option<bool> {
    batch_cached(
        state,
        &AvailabilityBatchArgs {
            kitsu_ids: vec![id.into()],
            mode: "sub".into(),
        },
    )
    .cached
    .get(id)
    .copied()
}

fn fallback_show(kitsu_id: &str) -> AvailabilityArgs {
    serde_json::from_value(serde_json::json!({
        "title": "Fallback Show",
        "mode": "sub",
        "kitsu_id": kitsu_id,
        "episode_count": 2
    }))
    .expect("args")
}

/// An anidb whose search answers with a 302 to another origin, which
/// serves the recorded page the live redirect landed on.
async fn redirecting_anidb() -> (wiremock::MockServer, wiremock::MockServer) {
    use wiremock::matchers::{method, path};
    let elsewhere = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(fixture("redirected_browse_anilab.html")),
        )
        .mount(&elsewhere)
        .await;
    let anidb = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/browse"))
        .respond_with(
            wiremock::ResponseTemplate::new(302)
                .insert_header("location", format!("{}/", elsewhere.uri()).as_str()),
        )
        .mount(&anidb)
        .await;
    (anidb, elsewhere)
}

#[tokio::test]
async fn a_search_redirected_off_anidb_moves_the_walk_to_the_fallback() {
    let (anidb, elsewhere) = redirecting_anidb().await;
    let hianime = stub_hianime_sub_only().await;
    let td = tempfile::tempdir().expect("td");
    let mut state = cache_only_state(&td);
    state.provider_order = vec![ProviderId::Anidb, ProviderId::Hianime];
    state.hianime_base = Some(hianime.uri());

    let got = check_availability_with_base(&state, &fallback_show("590"), Some(&anidb.uri()))
        .await
        .expect("the fallback answered");

    assert!(
        !elsewhere
            .received_requests()
            .await
            .expect("recorded")
            .is_empty(),
        "the transport followed anidb's redirect"
    );
    assert!(
        got.available,
        "a redirected search is no answer: the fallback carries the show"
    );
    assert_eq!(got.provider, Some(ProviderId::Hianime));
    let row = meta_cache_get(&state.cache_pool, &cache_key("590", "sub"))
        .expect("cache read")
        .expect("the fallback's verdict is persisted");
    let row: AvailabilityResponse = serde_json::from_str(&row).expect("row parses");
    assert!(
        row.available && row.provider == Some(ProviderId::Hianime),
        "and no anidb negative stands in its place"
    );
}

#[tokio::test]
async fn an_anidb_negative_from_before_the_guard_is_not_served() {
    let hianime = stub_hianime_sub_only().await;
    let td = tempfile::tempdir().expect("td");
    let mut state = cache_only_state(&td);
    state.provider_order = vec![ProviderId::Anidb, ProviderId::Hianime];
    state.hianime_base = Some(hianime.uri());
    meta_cache_put(
        &state.cache_pool,
        &cache_key("591", "sub"),
        PRE_GUARD_ANIDB_NEGATIVE,
        NEGATIVE_TTL,
    )
    .expect("seed the row");
    // anidb has been seen answering: under the provider read rule
    // alone its negative would stand.
    close_breaker(&state.anidb_gate);

    assert_eq!(
        listed(&state, "591"),
        None,
        "the lists do not hide the show on a row the redirect may have written"
    );
    let got =
        check_availability_with_base(&state, &fallback_show("591"), Some("http://127.0.0.1:1"))
            .await
            .expect("the probe ran and the fallback answered");
    assert!(
        got.available,
        "the row did not short-circuit the probe, which found the show on the fallback"
    );
}

#[tokio::test]
async fn an_anidb_negative_written_now_is_served_as_before() {
    let td = tempfile::tempdir().expect("td");
    let mut state = cache_only_state(&td);
    state.provider_order = vec![ProviderId::Anidb, ProviderId::Hianime];
    close_breaker(&state.anidb_gate);
    write_cache(&state, "592", "sub", false, Some(ProviderId::Anidb));
    assert_eq!(
        listed(&state, "592"),
        Some(false),
        "a miss the guarded reading wrote stands under the provider read rule"
    );
}

#[tokio::test]
async fn the_fallbacks_negative_from_before_the_guard_is_untouched() {
    // Only anidb's reading changed; hianime's negatives were never
    // written from a page that was not hianime's.
    let td = tempfile::tempdir().expect("td");
    let mut state = cache_only_state(&td);
    state.provider_order = vec![ProviderId::Anidb, ProviderId::Hianime];
    meta_cache_put(
        &state.cache_pool,
        &cache_key("593", "sub"),
        PRE_GUARD_HIANIME_NEGATIVE,
        NEGATIVE_TTL,
    )
    .expect("seed the row");
    open_breaker(&state.anidb_gate);
    close_breaker(&state.hianime_gate);
    assert_eq!(listed(&state, "593"), Some(false));
}

/// An unattributed negative from before the guard, as the build that
/// put anidb first stored it.
const PRE_GUARD_UNATTRIBUTED_NEGATIVE: &str = r#"{"available":false,"episode_count":null,"extra_episodes":[],"episode_count_approximate":false,"provider":null}"#;

#[tokio::test]
async fn with_hianime_first_an_unattributed_pre_guard_negative_is_still_not_served() {
    // The read rule attributes a row naming nobody to the first
    // provider. Every such row was written while anidb.app was first,
    // so with hianime first it would be read as hianime's verdict —
    // a miss hianime never gave, possibly the redirect's.
    let td = tempfile::tempdir().expect("td");
    let mut state = cache_only_state(&td);
    state.provider_order = vec![ProviderId::Hianime, ProviderId::Anidb];
    meta_cache_put(
        &state.cache_pool,
        &cache_key("594", "sub"),
        PRE_GUARD_UNATTRIBUTED_NEGATIVE,
        NEGATIVE_TTL,
    )
    .expect("seed the row");
    close_breaker(&state.hianime_gate);
    assert_eq!(listed(&state, "594"), None);
}

#[tokio::test]
async fn with_hianime_first_its_pre_guard_negative_stands_as_the_primarys() {
    // hianime's own misses were never read off another site's page:
    // with hianime first they stand while hianime answers.
    let td = tempfile::tempdir().expect("td");
    let mut state = cache_only_state(&td);
    state.provider_order = vec![ProviderId::Hianime, ProviderId::Anidb];
    meta_cache_put(
        &state.cache_pool,
        &cache_key("595", "sub"),
        PRE_GUARD_HIANIME_NEGATIVE,
        NEGATIVE_TTL,
    )
    .expect("seed the row");
    close_breaker(&state.hianime_gate);
    assert_eq!(listed(&state, "595"), Some(false));
}
