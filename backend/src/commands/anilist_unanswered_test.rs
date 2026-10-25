//! Every AniList-backed input, read while AniList cannot answer — once
//! refusing this client (429 with a minute's wait) and once down (503
//! with no wait) — and checked against the one rule they share: a
//! missing answer is unknown. It is never read as "no", and never
//! cached as an absence, so the next read asks again.

use std::sync::Arc;

use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::app::AppState;
use crate::cache::meta_cache_get;
use crate::error::AniError;

/// How AniList fails to answer.
#[derive(Debug, Clone, Copy)]
enum Unanswered {
    RateLimited,
    Down,
}

const BOTH: [Unanswered; 2] = [Unanswered::RateLimited, Unanswered::Down];

async fn anilist(how: Unanswered) -> MockServer {
    let server = MockServer::start().await;
    let reply = match how {
        Unanswered::RateLimited => ResponseTemplate::new(429).insert_header("retry-after", "60"),
        Unanswered::Down => ResponseTemplate::new(503),
    };
    // Asked at least once: a test that never reached AniList would
    // pass without saying anything about it.
    Mock::given(method("POST"))
        .respond_with(reply)
        .expect(1..)
        .mount(&server)
        .await;
    server
}

/// Kitsu answering `/anime/<id>?include=mappings` for each show, and
/// `/mappings` with no mapping for any MAL id.
async fn kitsu(shows: &[(&str, &[(&str, &str)])]) -> MockServer {
    let server = MockServer::start().await;
    for (kitsu_id, mappings) in shows {
        let included: Vec<serde_json::Value> = mappings
            .iter()
            .enumerate()
            .map(|(i, (site, id))| {
                serde_json::json!({
                    "id": i.to_string(),
                    "type": "mappings",
                    "attributes": { "externalSite": site, "externalId": id }
                })
            })
            .collect();
        Mock::given(method("GET"))
            .and(path(format!("/anime/{kitsu_id}")))
            .and(query_param("include", "mappings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": { "id": kitsu_id, "type": "anime", "attributes": { "canonicalTitle": "t" } },
                "included": included,
            })))
            .mount(&server)
            .await;
    }
    Mock::given(method("GET"))
        .and(path("/mappings"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({ "data": [], "included": [] })),
        )
        .mount(&server)
        .await;
    server
}

fn state_with_kitsu(kitsu_uri: &str) -> Arc<AppState> {
    use std::path::PathBuf;
    Arc::new(AppState {
        secret: crate::proxy::AppSecret::random(),
        sessions: crate::proxy::SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        host_budget: crate::proxy::host_budget::HostBudget::fresh(),
        meta_http: reqwest::Client::new(),
        proxy_origin: crate::proxy::ProxyOrigin::new("127.0.0.1", 12_345),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path: PathBuf::from("/tmp/ani-gui-unanswered/history"),
        providers: crate::provider_set::ProviderSet::for_tests(),
        image_cache_dir: PathBuf::from("/tmp/ani-gui-unanswered/images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: crate::meta::kitsu::KitsuClient::with_base(reqwest::Client::new(), kitsu_uri),
        config_path: PathBuf::from("/tmp/ani-gui-unanswered/config.toml"),
        state_dir: PathBuf::from("/tmp/ani-gui-unanswered/state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    })
}

fn row(state: &AppState, key: &str) -> Option<String> {
    meta_cache_get(&state.cache_pool, key).expect("cache read")
}

/// The error AniList's failure surfaces as: the rate limit it is, or
/// the upstream status it was.
fn is_the_failure(how: Unanswered, err: &AniError) -> bool {
    match how {
        Unanswered::RateLimited => matches!(err, AniError::RateLimited { .. }),
        Unanswered::Down => matches!(err, AniError::Upstream { status: 503 }),
    }
}

// --- Airing: the detail page's unaired tiles, and every reader of the
// airing row (the availability window, the appended-extras evidence) ---

#[tokio::test]
async fn an_unanswered_airing_read_is_an_error_and_writes_no_row() {
    // The renderer reads the error as "unknown" and gates no tile; the
    // availability window and the evidence read no row, and fall back
    // to what they do without a schedule.
    for how in BOTH {
        let kitsu = kitsu(&[("50551", &[("anilist/anime", "207141")])]).await;
        let anilist = anilist(how).await;
        let state = state_with_kitsu(&kitsu.uri());
        let err = crate::commands::airing::airing_get_with_anilist_base(
            &state,
            "50551",
            Some(&anilist.uri()),
        )
        .await
        .expect_err("no answer is no status");
        assert!(is_the_failure(how, &err), "{how:?}: {err:?}");
        assert_eq!(row(&state, "airing:v2:50551"), None, "{how:?}");
    }
}

#[tokio::test]
async fn an_unanswered_batch_seed_writes_no_airing_row() {
    for how in BOTH {
        let kitsu = kitsu(&[
            ("50551", &[("anilist/anime", "207141")]),
            ("60001", &[("myanimelist/anime", "63403")]),
        ])
        .await;
        let anilist = anilist(how).await;
        let state = state_with_kitsu(&kitsu.uri());
        crate::commands::airing::seed_airing_rows_batch(
            &state,
            &["50551".to_string(), "60001".to_string()],
            Some(&anilist.uri()),
        )
        .await;
        assert_eq!(row(&state, "airing:v2:50551"), None, "{how:?}");
        assert_eq!(row(&state, "airing:v2:60001"), None, "{how:?}");
    }
}

// --- The id bridges between AniList and MAL -----------------------------

#[tokio::test]
async fn an_unanswered_id_translation_is_an_error_and_remembers_nothing() {
    for how in BOTH {
        let anilist = anilist(how).await;
        let state = state_with_kitsu("http://127.0.0.1:1");
        let uri = Some(anilist.uri());
        let one = crate::commands::anilist_ids::media_id_for_mal(&state, 21, uri.as_deref()).await;
        assert!(
            matches!(&one, Err(e) if is_the_failure(how, e)),
            "{how:?}: {one:?}"
        );
        let back =
            crate::commands::anilist_ids::mal_id_for_media_id(&state, 30, uri.as_deref()).await;
        assert!(
            matches!(&back, Err(e) if is_the_failure(how, e)),
            "{how:?}: {back:?}"
        );
        let many =
            crate::commands::anilist_ids::media_ids_for_mals(&state, &[21, 22], uri.as_deref())
                .await;
        assert!(
            matches!(&many, Err(e) if is_the_failure(how, e)),
            "{how:?}: {many:?}"
        );
        for key in [
            "anilist:media-id-for-mal:v1:21",
            "anilist:media-id-for-mal:v1:22",
            "anilist:mal-id-for-media:v1:30",
        ] {
            assert_eq!(row(&state, key), None, "{how:?}: {key}");
        }
    }
}

#[tokio::test]
async fn an_unanswered_watch_later_bridge_records_no_dead_end() {
    // Kitsu has no MAL mapping for the card, and the AniList hop that
    // would find it under AniList's id does not answer: the card is
    // dropped for this load, and nothing says it never maps.
    for how in BOTH {
        let kitsu = kitsu(&[]).await;
        let anilist = anilist(how).await;
        let state = state_with_kitsu(&kitsu.uri());
        let cards = crate::commands::account::kitsu_for_mal_ids_with_anilist_base(
            &state,
            vec![63403],
            Some(&anilist.uri()),
        )
        .await;
        assert!(cards.is_empty(), "{how:?}");
        assert_eq!(row(&state, "kitsu:mal-map:v1:63403"), None, "{how:?}");
    }
}

#[tokio::test]
async fn an_unanswered_write_back_resolve_is_an_error_not_an_unmappable_show() {
    // Kitsu maps the show to MAL 21 and to AniList 30. The MAL bridge
    // to AniList's id gets no answer: the write fails as AniList did,
    // and is neither skipped as unmappable nor sent to the other id.
    for how in BOTH {
        let kitsu = kitsu(&[(
            "12",
            &[("myanimelist/anime", "21"), ("anilist/anime", "30")],
        )])
        .await;
        let anilist = anilist(how).await;
        let state = state_with_kitsu(&kitsu.uri());
        let got = crate::commands::account::resolve_native_media_id(
            &state,
            crate::account::provider::ProviderKind::AniList,
            "12",
            Some(&anilist.uri()),
        )
        .await;
        assert!(
            matches!(&got, Err(e) if is_the_failure(how, e)),
            "{how:?}: {got:?}"
        );
    }
}

// --- The trending row and the banner backfill -----------------------------

#[tokio::test]
async fn an_unanswered_trending_fetch_keeps_nothing_under_anilists_feed() {
    for how in BOTH {
        let kitsu = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/anime"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({ "data": [], "included": [] })),
            )
            .mount(&kitsu)
            .await;
        let anilist = anilist(how).await;
        let state = state_with_kitsu(&kitsu.uri());
        let _ =
            crate::commands::kitsu::kitsu_trending_anilist_with_base(&state, Some(&anilist.uri()))
                .await;
        assert_eq!(row(&state, "kitsu:v6:trending_anilist"), None, "{how:?}");
    }
}
