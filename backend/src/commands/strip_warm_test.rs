//! A cold Continue Watching strip warmed at once from the history's
//! recorded Kitsu ids, while its cards' rows become ready one by one: the
//! strip's stills and banners cost one AniList request each, however far
//! apart the cards ask, and every card reads what its own lookup would
//! have.

use std::time::Duration;

use super::*;
use crate::cache::meta_cache_get;
use crate::meta::anilist_streaming_eps_fake_test::{
    asked_for_banners, asked_for_stills, asked_of, FakeAniList, FakeMedia,
};

const NULL_COVER_DETAIL: &[u8] =
    include_bytes!("../../../tests/fixtures/kitsu/anime_null_cover_detail.json");

/// Kitsu answering each show's mappings (MAL id = its index) and its
/// detail, with no cover.
struct Kitsu;

impl wiremock::Respond for Kitsu {
    fn respond(&self, request: &wiremock::Request) -> wiremock::ResponseTemplate {
        let id = request.url.path().trim_start_matches("/anime/").to_owned();
        let Some(mal) = id.strip_prefix("80").and_then(|m| m.parse::<u32>().ok()) else {
            return wiremock::ResponseTemplate::new(404);
        };
        let mappings = request
            .url
            .query_pairs()
            .any(|(k, v)| k == "include" && v == "mappings");
        let body = if mappings {
            serde_json::json!({
                "data": { "id": id, "type": "anime", "attributes": { "canonicalTitle": "x" } },
                "included": [{
                    "id": "1",
                    "type": "mappings",
                    "attributes": { "externalSite": "myanimelist/anime", "externalId": mal.to_string() },
                }],
            })
        } else {
            let mut detail: serde_json::Value =
                serde_json::from_slice(NULL_COVER_DETAIL).expect("fixture");
            detail["data"]["id"] = serde_json::json!(id);
            detail["data"]["attributes"]["coverImage"] = serde_json::Value::Null;
            detail
        };
        wiremock::ResponseTemplate::new(200)
            .insert_header("content-type", "application/vnd.api+json")
            .set_body_json(body)
    }
}

fn state_with_kitsu_at(uri: &str) -> crate::scratch::Scratch<AppState> {
    let dirs = crate::scratch::dir();
    let state = AppState {
        secret: crate::proxy::AppSecret::random(),
        sessions: crate::proxy::SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        host_budget: crate::proxy::host_budget::HostBudget::fresh(),
        meta_http: reqwest::Client::new(),
        proxy_origin: crate::proxy::ProxyOrigin::new("127.0.0.1", 12_345),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path: dirs.history(),
        providers: crate::provider_set::ProviderSet::for_tests(),
        image_cache_dir: dirs.images(),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: crate::meta::kitsu::KitsuClient::with_base(reqwest::Client::new(), uri),
        config_path: dirs.config(),
        state_dir: dirs.state(),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
        flags: crate::flags::FlagStore::fixed(crate::flags::DECLARED),
    };
    dirs.hold(state)
}

/// Ten shows, Kitsu ids 801..=810, MAL ids 1..=10.
fn strip() -> (Vec<String>, FakeAniList) {
    let ids = (1..=10).map(|i| format!("80{i}")).collect();
    let fake = FakeAniList {
        media: (1..=10)
            .map(|i| FakeMedia {
                id: 100 + i,
                mal: Some(i),
                listed: 2,
            })
            .collect(),
    };
    (ids, fake)
}

async fn serve(respond: impl wiremock::Respond + 'static, http: &str) -> wiremock::MockServer {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method(http))
        .respond_with(respond)
        .mount(&server)
        .await;
    server
}

/// The card for show `i` (0-based) becomes ready `(i + 1) × 250 ms` in,
/// the way a cold strip's rows land seconds apart — each after the warm
/// Home sends as its history loads, since a row is ready only once its
/// match has come back.
async fn staggered<F, Fut, T>(n: usize, card: F) -> Vec<T>
where
    F: Fn(usize) -> Fut,
    Fut: std::future::Future<Output = T>,
{
    let card = &card;
    futures_util::future::join_all((0..n).map(|i| async move {
        tokio::time::sleep(Duration::from_millis(250) * u32::try_from(i + 1).unwrap_or(1)).await;
        card(i).await
    }))
    .await
}

#[tokio::test]
async fn a_cold_strip_warmed_at_once_fills_its_stills_in_one_request_however_late_its_cards() {
    let (ids, fake) = strip();
    let kitsu = serve(Kitsu, "GET").await;
    let anilist = serve(fake.clone(), "POST").await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let base = anilist.uri();
    let warm = warm_strip(&state, &ids, Some(&base));
    let cards = staggered(ids.len(), |i| {
        crate::commands::anilist_eps_thumbs::thumbs_for_show_with_anilist_base(
            &state,
            &ids[i],
            Some(&base),
        )
    });
    let ((), got) = tokio::join!(warm, cards);
    for (listing, m) in got.iter().zip(&fake.media) {
        assert_eq!(*listing, m.listing());
    }
    // The warm's lookups go as one page, none sent ahead alone.
    let asked = asked_for_stills(&anilist).await;
    assert_eq!(
        asked,
        vec![crate::meta::anilist_streaming_eps_fake_test::Asked::PageByMal((1..=10).collect())],
        "{} requests",
        asked.len()
    );
}

#[tokio::test]
async fn a_cold_strip_warmed_at_once_reads_its_banners_in_one_request_however_late_its_cards() {
    let (ids, fake) = strip();
    let kitsu = serve(Kitsu, "GET").await;
    let anilist = serve(fake.clone(), "POST").await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let base = anilist.uri();
    let warm = warm_strip(&state, &ids, Some(&base));
    let cards = staggered(ids.len(), |i| {
        crate::commands::kitsu::kitsu_anime_detail_with_anilist_base(&state, &ids[i], Some(&base))
    });
    let ((), got) = tokio::join!(warm, cards);
    for (detail, m) in got.into_iter().zip(&fake.media) {
        let detail = detail.expect("a detail");
        let cover = detail.cover_image.and_then(|c| c.original);
        assert_eq!(cover, m.banner(), "kitsu {}", detail.id);
    }
    let asked = asked_for_banners(&anilist).await;
    assert_eq!(
        asked,
        vec![crate::meta::anilist_streaming_eps_fake_test::Asked::PageByMal((1..=10).collect())],
        "{} requests",
        asked.len()
    );
}

#[tokio::test]
async fn a_warm_skips_what_the_cache_already_holds() {
    let (ids, fake) = strip();
    let kitsu = serve(Kitsu, "GET").await;
    let anilist = serve(fake, "POST").await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let base = anilist.uri();
    warm_strip(&state, &ids, Some(&base)).await;
    let first = asked_of(&anilist).await.len();
    assert!(first > 0, "the first warm asked nothing");
    for id in &ids {
        let key = crate::commands::anilist_eps_thumbs::anilist_eps_thumbs_key(id);
        assert!(
            meta_cache_get(&state.cache_pool, &key)
                .expect("read")
                .is_some(),
            "kitsu {id} not kept"
        );
    }
    warm_strip(&state, &ids, Some(&base)).await;
    assert_eq!(
        asked_of(&anilist).await.len(),
        first,
        "a warm cache asked again"
    );
}

#[tokio::test]
async fn a_warm_whose_caller_left_sends_nothing_more() {
    // Home left: the warm's request is dropped, and so is every lookup
    // only it was waiting on.
    let (ids, _) = strip();
    let kitsu = serve(Kitsu, "GET").await;
    let anilist = serve(
        wiremock::ResponseTemplate::new(200)
            .set_body_string("{}")
            .set_delay(Duration::from_millis(50)),
        "POST",
    )
    .await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let base = anilist.uri();
    let _gate = crate::meta::anilist_gate::share_for_tests(
        &base,
        crate::meta::anilist_gate::Pace {
            min_gap: Duration::from_secs(2),
            background_wait: Duration::from_secs(5),
            ..crate::meta::anilist_gate::ANILIST
        },
    );
    let left = tokio::time::timeout(
        Duration::from_millis(400),
        warm_strip(&state, &ids, Some(&base)),
    );
    assert!(left.await.is_err(), "the warm was still out");
    tokio::time::sleep(Duration::from_millis(2500)).await;
    let asked = asked_of(&anilist).await;
    assert!(asked.len() <= 1, "requests sent for nobody: {asked:?}");
}
