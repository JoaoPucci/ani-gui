//! Tests for `crate::commands::airing`. Extracted via `#[path]` per
//! `project_crap_inline_test_gotcha`.

use super::*;

/// AppState whose Kitsu client points at a wiremock server. Same
/// shape as `account_test::state_with_kitsu` (that helper is private
/// to its module).
fn state_with_kitsu(kitsu_uri: &str) -> std::sync::Arc<AppState> {
    use std::path::PathBuf;
    use std::sync::Arc;
    Arc::new(AppState {
        anidb_base: None,
        secret: crate::proxy::AppSecret::random(),
        sessions: crate::proxy::SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        meta_http: reqwest::Client::new(),
        proxy_origin: crate::proxy::ProxyOrigin::new("127.0.0.1", 12_345),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path: PathBuf::from("/tmp/ani-gui/history"),
        anidb_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
        hianime_base: None,
        hianime_gate: Arc::new(crate::scraper::gate::ScraperGate::new()),
        provider_order: vec![crate::scraper::provider::ProviderId::Anidb],
        image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: crate::meta::kitsu::KitsuClient::with_base(reqwest::Client::new(), kitsu_uri),
        config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
        state_dir: PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    })
}

/// Yani Neko's real mapping shape: anilist/anime only.
const KITSU_ANILIST_ONLY_MAPPING_BODY: &str = r#"{
    "data": { "id": "50551", "type": "anime", "attributes": { "canonicalTitle": "Yani Neko" } },
    "included": [{
        "id": "1",
        "type": "mappings",
        "attributes": { "externalSite": "anilist/anime", "externalId": "207141" }
    }]
}"#;

const ANILIST_RELEASING_BODY: &str = r#"{"data":{"Media":{
    "status":"RELEASING","episodes":12,
    "nextAiringEpisode":{"episode":3,"airingAt":1784215800}
}}}"#;

#[tokio::test]
async fn airing_get_bridges_kitsu_mappings_to_anilist() {
    use wiremock::matchers::{method, path};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/50551"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(KITSU_ANILIST_ONLY_MAPPING_BODY),
        )
        .mount(&kitsu)
        .await;
    let anilist = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(ANILIST_RELEASING_BODY))
        .mount(&anilist)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let got = airing_get_with_anilist_base(&state, "50551", Some(&anilist.uri()))
        .await
        .expect("ok");
    assert_eq!(got.aired, Some(2));
    assert_eq!(got.next_episode, Some(3));
    assert_eq!(got.next_airing_at, Some(1_784_215_800));
}

#[tokio::test]
async fn airing_get_defaults_when_kitsu_has_no_mappings() {
    use wiremock::matchers::{method, path};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/777"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(r#"{"data":{"id":"777","type":"anime"},"included":[]}"#),
        )
        .mount(&kitsu)
        .await;
    let anilist = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("{}"))
        .expect(0)
        .mount(&anilist)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let got = airing_get_with_anilist_base(&state, "777", Some(&anilist.uri()))
        .await
        .expect("ok");
    assert_eq!(got, AiringStatus::default());
}

// --- airing_ttl_for -----------------------------------------------
// Codex P2 #3565710322: a row cached shortly before a scheduled
// airing must not outlive the airing by the full fixed TTL, or the
// just-dropped episode stays greyed for hours. The TTL caps at the
// schedule boundary plus a short grace.

const NOW: u64 = 1_784_000_000;

#[test]
fn ttl_stays_at_the_fixed_window_without_a_schedule() {
    assert_eq!(airing_ttl_for(None, NOW), AIRING_TTL_SECS);
}

#[test]
fn ttl_caps_at_the_next_airing_plus_grace() {
    // Airing 30 minutes out: the row dies shortly after the drop, not
    // 3 hours later.
    let at = NOW + 30 * 60;
    assert_eq!(airing_ttl_for(Some(at), NOW), 30 * 60 + AIRING_GRACE_SECS);
}

#[test]
fn ttl_stretches_to_a_day_for_a_distant_airing() {
    // Next episode is 5 days away — the aired count cannot move until
    // then, so re-asking AniList every 3 hours buys nothing. A daily
    // ceiling keeps rate-limit pressure ~8x lower while still snapping
    // to the schedule boundary as it approaches.
    let at = NOW + 5 * 24 * 60 * 60;
    assert_eq!(airing_ttl_for(Some(at), NOW), AIRING_TTL_MAX_SECS);
}

#[test]
fn ttl_tracks_the_schedule_inside_the_daily_ceiling() {
    // Airing 5 hours out: the row dies just after the drop — the
    // ceiling only bites for airings further than a day away.
    let five_hours = 5 * 60 * 60;
    assert_eq!(
        airing_ttl_for(Some(NOW + five_hours), NOW),
        five_hours + AIRING_GRACE_SECS
    );
}

#[test]
fn ttl_collapses_to_grace_when_the_schedule_already_passed() {
    // Stale AniList row / clock skew: recheck soon, but not per-request.
    assert_eq!(airing_ttl_for(Some(NOW - 60), NOW), AIRING_GRACE_SECS);
    assert_eq!(airing_ttl_for(Some(NOW), NOW), AIRING_GRACE_SECS);
}

#[tokio::test]
async fn airing_get_caches_per_show() {
    use wiremock::matchers::{method, path};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/50551"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(KITSU_ANILIST_ONLY_MAPPING_BODY),
        )
        .expect(1)
        .mount(&kitsu)
        .await;
    let anilist = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(ANILIST_RELEASING_BODY))
        .expect(1)
        .mount(&anilist)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let first = airing_get_with_anilist_base(&state, "50551", Some(&anilist.uri()))
        .await
        .expect("ok");
    let second = airing_get_with_anilist_base(&state, "50551", Some(&anilist.uri()))
        .await
        .expect("ok");
    assert_eq!(first, second);
    assert_eq!(second.aired, Some(2));
    // Both MockServers verify their .expect(1) on drop — the second
    // call must be served from the cache.
}

// --- batch seeding ---------------------------------------------------
// The home-rail warm used to seed airing rows one AniList request per
// pre-premiere show; seed_airing_rows_batch collapses a rail into one.

const KITSU_MAPPING_49444: &str = r#"{
    "data": { "id": "49444", "type": "anime", "attributes": { "canonicalTitle": "Kashin-tan" } },
    "included": [{
        "id": "2",
        "type": "mappings",
        "attributes": { "externalSite": "anilist/anime", "externalId": "185874" }
    }]
}"#;

const ANILIST_BATCH_BODY: &str = r#"{"data":{"Page":{"media":[
    {"id":207141,"status":"RELEASING","episodes":12,
     "nextAiringEpisode":{"episode":3,"airingAt":1784215800}},
    {"id":185874,"status":"NOT_YET_RELEASED","episodes":13,
     "nextAiringEpisode":{"episode":1,"airingAt":1784988000}}
]}}}"#;

#[tokio::test]
async fn seed_airing_rows_batch_writes_all_rows_with_one_anilist_request() {
    use wiremock::matchers::{method, path};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/50551"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(KITSU_ANILIST_ONLY_MAPPING_BODY),
        )
        .mount(&kitsu)
        .await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/49444"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(KITSU_MAPPING_49444))
        .mount(&kitsu)
        .await;
    let anilist = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(ANILIST_BATCH_BODY))
        .expect(1)
        .mount(&anilist)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    seed_airing_rows_batch(
        &state,
        &["50551".to_string(), "49444".to_string()],
        Some(&anilist.uri()),
    )
    .await;
    for (id, expected_next) in [("50551", 1_784_215_800u64), ("49444", 1_784_988_000u64)] {
        let body = crate::cache::meta_cache_get(&state.cache_pool, &format!("airing:v2:{id}"))
            .expect("cache read")
            .expect("row written");
        let status: AiringStatus = serde_json::from_str(&body).expect("parses");
        assert_eq!(status.next_airing_at, Some(expected_next));
    }
    // AniList MockServer verifies expect(1) on drop.
}

#[tokio::test]
async fn seed_airing_rows_batch_skips_shows_with_fresh_rows() {
    let kitsu = wiremock::MockServer::start().await;
    let anilist = wiremock::MockServer::start().await;
    // No mounts: a fresh row must produce zero Kitsu or AniList calls.
    let state = state_with_kitsu(&kitsu.uri());
    let row = serde_json::to_string(&AiringStatus::default()).expect("serializes");
    crate::cache::meta_cache_put(&state.cache_pool, "airing:v2:50551", &row, 3600)
        .expect("seed row");
    seed_airing_rows_batch(&state, &["50551".to_string()], Some(&anilist.uri())).await;
    assert!(kitsu
        .received_requests()
        .await
        .expect("recorded")
        .is_empty());
    assert!(anilist
        .received_requests()
        .await
        .expect("recorded")
        .is_empty());
}

#[tokio::test]
async fn a_refresh_skips_the_cached_row_and_stores_what_it_fetched() {
    // The click on an unaired tile: the cached row says one episode is
    // out, the schedule now says two. The refresh must reach AniList
    // past the row, and the next plain read must see what it stored.
    use wiremock::matchers::{method, path};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/50551"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(KITSU_ANILIST_ONLY_MAPPING_BODY),
        )
        .mount(&kitsu)
        .await;
    let anilist = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(ANILIST_RELEASING_BODY))
        .expect(1)
        .mount(&anilist)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    crate::cache::meta_cache_put(
        &state.cache_pool,
        "airing:v2:50551",
        r#"{"aired":1,"next_episode":null,"next_airing_at":null,"upcoming":[]}"#,
        3600,
    )
    .expect("seed");
    let fresh = airing_refresh_with_anilist_base(&state, "50551", Some(&anilist.uri()))
        .await
        .expect("ok");
    assert_eq!(fresh.aired, Some(2));
    let stored = airing_get_with_anilist_base(&state, "50551", Some("http://127.0.0.1:1"))
        .await
        .expect("served from the row the refresh wrote");
    assert_eq!(stored.aired, Some(2));
}

// --- a schedule that arrives after the availability row ---------------
// A current show's positive row written before any airing row keeps
// the ongoing day; every path that writes the airing row must cut it.

fn epoch_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn seed_positive_day(state: &AppState, kitsu_id: &str) {
    let key = crate::commands::availability::cache_key(kitsu_id, "sub");
    crate::cache::meta_cache_put(
        &state.cache_pool,
        &key,
        r#"{"available":true,"episode_count":7}"#,
        24 * 60 * 60,
    )
    .expect("seed availability row");
}

fn positive_ttl(state: &AppState, kitsu_id: &str) -> u64 {
    let conn = state.cache_pool.get().expect("conn");
    let ttl: i64 = conn
        .query_row(
            "SELECT ttl_seconds FROM meta_cache WHERE key = ?1",
            [crate::commands::availability::cache_key(kitsu_id, "sub")],
            |r| r.get(0),
        )
        .expect("row");
    u64::try_from(ttl).expect("non-negative")
}

async fn mappings_and_anilist(body: String) -> (wiremock::MockServer, wiremock::MockServer) {
    use wiremock::matchers::{method, path};
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/anime/50551"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(KITSU_ANILIST_ONLY_MAPPING_BODY),
        )
        .mount(&kitsu)
        .await;
    let anilist = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(body))
        .mount(&anilist)
        .await;
    (kitsu, anilist)
}

fn releasing_in_two_hours() -> String {
    let at = epoch_now() + 2 * 60 * 60;
    format!(
        r#"{{"data":{{"Media":{{"status":"RELEASING","episodes":12,
        "nextAiringEpisode":{{"episode":8,"airingAt":{at}}}}}}}}}"#
    )
}

/// Two hours to the drop plus the provider's grace — not a day.
fn assert_cut_at_the_drop(ttl: u64) {
    assert!(ttl <= 3 * 60 * 60 + 5, "ttl {ttl}");
    assert!(ttl > 2 * 60 * 60, "ttl {ttl}");
}

#[tokio::test]
async fn a_detail_page_s_schedule_cuts_a_count_written_before_it() {
    let (kitsu, anilist) = mappings_and_anilist(releasing_in_two_hours()).await;
    let state = state_with_kitsu(&kitsu.uri());
    seed_positive_day(&state, "50551");
    airing_get_with_anilist_base(&state, "50551", Some(&anilist.uri()))
        .await
        .expect("ok");
    assert_cut_at_the_drop(positive_ttl(&state, "50551"));
}

#[tokio::test]
async fn a_recheck_s_schedule_cuts_a_count_written_before_it() {
    let (kitsu, anilist) = mappings_and_anilist(releasing_in_two_hours()).await;
    let state = state_with_kitsu(&kitsu.uri());
    seed_positive_day(&state, "50551");
    airing_refresh_with_anilist_base(&state, "50551", Some(&anilist.uri()))
        .await
        .expect("ok");
    assert_cut_at_the_drop(positive_ttl(&state, "50551"));
}

#[tokio::test]
async fn a_batch_seeded_schedule_cuts_a_count_written_before_it() {
    let at = epoch_now() + 2 * 60 * 60;
    let batch = format!(
        r#"{{"data":{{"Page":{{"media":[
        {{"id":207141,"status":"RELEASING","episodes":12,
         "nextAiringEpisode":{{"episode":8,"airingAt":{at}}}}}]}}}}}}"#
    );
    let (kitsu, anilist) = mappings_and_anilist(batch).await;
    let state = state_with_kitsu(&kitsu.uri());
    seed_positive_day(&state, "50551");
    seed_airing_rows_batch(&state, &["50551".to_string()], Some(&anilist.uri())).await;
    assert_cut_at_the_drop(positive_ttl(&state, "50551"));
}

#[tokio::test]
async fn a_batch_seeded_schedule_cuts_a_negative_written_before_it() {
    let at = epoch_now() + 2 * 60 * 60;
    let batch = format!(
        r#"{{"data":{{"Page":{{"media":[
        {{"id":207141,"status":"RELEASING","episodes":12,
         "nextAiringEpisode":{{"episode":8,"airingAt":{at}}}}}]}}}}}}"#
    );
    let (kitsu, anilist) = mappings_and_anilist(batch).await;
    let state = state_with_kitsu(&kitsu.uri());
    let key = crate::commands::availability::cache_key("50551", "sub");
    crate::cache::meta_cache_put(
        &state.cache_pool,
        &key,
        r#"{"available":false}"#,
        24 * 60 * 60,
    )
    .expect("seed negative row");
    seed_airing_rows_batch(&state, &["50551".to_string()], Some(&anilist.uri())).await;
    // Two hours to the drop plus the negative grace of three — not a day.
    let ttl = positive_ttl(&state, "50551");
    assert!(ttl <= 5 * 60 * 60 + 5, "ttl {ttl}");
    assert!(ttl > 4 * 60 * 60, "ttl {ttl}");
}
