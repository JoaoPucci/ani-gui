//! Tests for `crate::commands::anilist_ids`. Extracted via `#[path]`
//! so the wiremock fixtures stay off the command file's CRAP budget.

use std::path::PathBuf;
use std::sync::Arc;

use wiremock::matchers::{body_string_contains, method};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::{mal_id_for_media_id, media_id_for_mal, media_ids_for_mals};
use crate::app::AppState;
use crate::cache::meta_cache_row;
use crate::error::AniError;

/// `AppState` with an in-memory cache; the Kitsu client points at an
/// unbound port because nothing here reaches Kitsu.
fn state() -> Arc<AppState> {
    Arc::new(AppState {
        secret: crate::proxy::AppSecret::random(),
        sessions: crate::proxy::SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        host_budget: crate::proxy::host_budget::HostBudget::fresh(),
        meta_http: reqwest::Client::new(),
        proxy_origin: crate::proxy::ProxyOrigin::new("127.0.0.1", 12_345),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path: PathBuf::from("/tmp/ani-gui/history"),
        providers: crate::provider_set::ProviderSet::for_tests(),
        image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: crate::meta::kitsu::KitsuClient::with_base(
            reqwest::Client::new(),
            "http://127.0.0.1:1",
        ),
        config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
        state_dir: PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    })
}

/// An AniList answering every POST with `body`, `times` times at most.
async fn anilist_answering(template: ResponseTemplate, times: u64) -> MockServer {
    let anilist = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(template)
        .expect(times)
        .mount(&anilist)
        .await;
    anilist
}

const DAY: u64 = 24 * 60 * 60;

fn ttl_of(state: &AppState, key: &str) -> u64 {
    meta_cache_row(&state.cache_pool, key)
        .expect("cache read")
        .expect("row written")
        .ttl_seconds
}

// ─── MAL id → AniList media id ──────────────────────────────────────

#[tokio::test]
async fn a_second_translation_of_the_same_mal_id_asks_anilist_nothing() {
    let anilist = anilist_answering(
        ResponseTemplate::new(200).set_body_string(r#"{"data":{"Media":{"id":154587}}}"#),
        1,
    )
    .await;
    let state = state();

    for _ in 0..2 {
        let got = media_id_for_mal(&state, 21, Some(&anilist.uri()))
            .await
            .expect("translation");
        assert_eq!(got, Some(154587));
    }
    assert_eq!(
        ttl_of(&state, "anilist:media-id-for-mal:v1:21"),
        30 * DAY,
        "an id mapping does not move"
    );
}

#[tokio::test]
async fn anilist_not_indexing_a_mal_id_is_remembered_for_a_day() {
    let anilist = anilist_answering(
        ResponseTemplate::new(200).set_body_string(r#"{"data":{"Media":null}}"#),
        1,
    )
    .await;
    let state = state();

    for _ in 0..2 {
        let got = media_id_for_mal(&state, 21, Some(&anilist.uri()))
            .await
            .expect("translation");
        assert_eq!(got, None);
    }
    assert_eq!(
        ttl_of(&state, "anilist:media-id-for-mal:v1:21"),
        DAY,
        "AniList indexes new shows within days"
    );
}

#[tokio::test]
async fn a_failed_translation_is_not_remembered() {
    let anilist = anilist_answering(ResponseTemplate::new(500), 2).await;
    let state = state();

    for _ in 0..2 {
        let err = media_id_for_mal(&state, 21, Some(&anilist.uri()))
            .await
            .unwrap_err();
        assert!(matches!(err, AniError::Upstream { status: 500 }), "{err:?}");
    }
    assert!(
        meta_cache_row(&state.cache_pool, "anilist:media-id-for-mal:v1:21")
            .expect("cache read")
            .is_none(),
        "a failure is not evidence about the mapping"
    );
}

// ─── AniList media id → MAL id ──────────────────────────────────────

#[tokio::test]
async fn a_second_translation_of_the_same_media_id_asks_anilist_nothing() {
    let anilist = anilist_answering(
        ResponseTemplate::new(200).set_body_string(r#"{"data":{"Media":{"idMal":63403}}}"#),
        1,
    )
    .await;
    let state = state();

    for _ in 0..2 {
        let got = mal_id_for_media_id(&state, 207141, Some(&anilist.uri()))
            .await
            .expect("translation");
        assert_eq!(got, Some(63403));
    }
    assert_eq!(
        ttl_of(&state, "anilist:mal-id-for-media:v1:207141"),
        30 * DAY
    );
}

#[tokio::test]
async fn anilist_recording_no_mal_id_is_remembered_for_a_day() {
    let anilist = anilist_answering(
        ResponseTemplate::new(200).set_body_string(r#"{"data":{"Media":{"idMal":null}}}"#),
        1,
    )
    .await;
    let state = state();

    for _ in 0..2 {
        let got = mal_id_for_media_id(&state, 207141, Some(&anilist.uri()))
            .await
            .expect("translation");
        assert_eq!(got, None);
    }
    assert_eq!(ttl_of(&state, "anilist:mal-id-for-media:v1:207141"), DAY);
}

// ─── The batch ──────────────────────────────────────────────────────

#[tokio::test]
async fn a_batch_asks_only_for_the_ids_it_does_not_know() {
    let anilist = MockServer::start().await;
    // The single translation warms 63403.
    Mock::given(method("POST"))
        .and(body_string_contains("MediaIdByMal"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"data":{"Media":{"id":207141}}}"#),
        )
        .expect(1)
        .mount(&anilist)
        .await;
    // The batch is then asked about 63404 alone.
    Mock::given(method("POST"))
        .and(body_string_contains("MediaIdsByMals"))
        .and(body_string_contains("63404"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"data":{"Page":{"media":[{"id":207142,"idMal":63404}]}}}"#),
        )
        .expect(1)
        .mount(&anilist)
        .await;
    let state = state();

    media_id_for_mal(&state, 63403, Some(&anilist.uri()))
        .await
        .expect("warm");
    let map = media_ids_for_mals(&state, &[63403, 63404], Some(&anilist.uri()))
        .await
        .expect("batch");

    assert_eq!(map.get(&63403), Some(&207141), "served from the cache");
    assert_eq!(map.get(&63404), Some(&207142), "fetched");
    let batch_bodies: Vec<String> = anilist
        .received_requests()
        .await
        .expect("recorded")
        .iter()
        .map(|r| String::from_utf8_lossy(&r.body).into_owned())
        .filter(|b| b.contains("MediaIdsByMals"))
        .collect();
    assert_eq!(batch_bodies.len(), 1);
    assert!(
        !batch_bodies[0].contains("63403"),
        "a known id is not asked again: {}",
        batch_bodies[0]
    );
}

#[tokio::test]
async fn a_batch_answer_serves_the_single_translation_afterwards() {
    let anilist = anilist_answering(
        ResponseTemplate::new(200)
            .set_body_string(r#"{"data":{"Page":{"media":[{"id":207141,"idMal":63403}]}}}"#),
        1,
    )
    .await;
    let state = state();

    media_ids_for_mals(&state, &[63403, 63404], Some(&anilist.uri()))
        .await
        .expect("batch");
    // Both ids were answered by the batch: one positively, one by
    // its absence from a page AniList did return.
    let known = media_id_for_mal(&state, 63403, Some(&anilist.uri()))
        .await
        .expect("single");
    let absent = media_id_for_mal(&state, 63404, Some(&anilist.uri()))
        .await
        .expect("single");
    assert_eq!(known, Some(207141));
    assert_eq!(absent, None);
    // `.expect(1)`: neither single translation reached AniList.
}

#[tokio::test]
async fn a_batch_served_wholly_from_the_cache_makes_no_request() {
    let anilist = anilist_answering(
        ResponseTemplate::new(200)
            .set_body_string(r#"{"data":{"Page":{"media":[{"id":207141,"idMal":63403}]}}}"#),
        1,
    )
    .await;
    let state = state();

    let first = media_ids_for_mals(&state, &[63403], Some(&anilist.uri()))
        .await
        .expect("batch");
    let second = media_ids_for_mals(&state, &[63403], Some(&anilist.uri()))
        .await
        .expect("batch");
    assert_eq!(first, second);
    assert_eq!(second.get(&63403), Some(&207141));
}

#[tokio::test]
async fn a_failed_batch_remembers_nothing() {
    let anilist = anilist_answering(ResponseTemplate::new(503), 2).await;
    let state = state();

    for _ in 0..2 {
        let err = media_ids_for_mals(&state, &[63403], Some(&anilist.uri()))
            .await
            .unwrap_err();
        assert!(matches!(err, AniError::Upstream { status: 503 }), "{err:?}");
    }
    assert!(
        meta_cache_row(&state.cache_pool, "anilist:media-id-for-mal:v1:63403")
            .expect("cache read")
            .is_none()
    );
}
