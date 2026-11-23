//! A strip of shows Kitsu has no cover for, their details read at once:
//! the first detail's banner lookup goes out alone as it always did, and
//! the rest share one AniList request per id space instead of one each.
//! Each detail still keeps its own cached row, banner included.

use super::*;
use crate::meta::anilist_streaming_eps_fake_test::{asked_of, Asked, FakeAniList, FakeMedia};

const NULL_COVER_DETAIL: &[u8] =
    include_bytes!("../../../tests/fixtures/kitsu/anime_null_cover_detail.json");

/// Kitsu answering each show's detail, with no cover, and its mappings:
/// (kitsu id, its sites).
struct Kitsu(Vec<(String, Vec<(&'static str, u32)>)>);

impl wiremock::Respond for Kitsu {
    fn respond(&self, request: &wiremock::Request) -> wiremock::ResponseTemplate {
        let id = request.url.path().trim_start_matches("/anime/");
        let Some((_, sites)) = self.0.iter().find(|(k, _)| k == id) else {
            return wiremock::ResponseTemplate::new(404);
        };
        let mappings = request
            .url
            .query_pairs()
            .any(|(k, v)| k == "include" && v == "mappings");
        let body = if mappings {
            let included: Vec<serde_json::Value> = sites
                .iter()
                .enumerate()
                .map(|(i, (site, ext))| {
                    serde_json::json!({
                        "id": (i + 1).to_string(),
                        "type": "mappings",
                        "attributes": { "externalSite": site, "externalId": ext.to_string() },
                    })
                })
                .collect();
            serde_json::json!({
                "data": { "id": id, "type": "anime", "attributes": { "canonicalTitle": "x" } },
                "included": included,
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

async fn serve(respond: impl wiremock::Respond + 'static, http: &str) -> wiremock::MockServer {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method(http))
        .respond_with(respond)
        .mount(&server)
        .await;
    server
}

/// Read every show's detail at once, as the strip's cards do; their
/// mappings first, so the lookups reach AniList in the order given.
async fn read_all(state: &AppState, ids: &[String], base: &str) -> Vec<KitsuAnimeRef> {
    for id in ids {
        let _ = crate::commands::external_ids::external_ids_for(state, id).await;
    }
    futures_util::future::join_all(
        ids.iter()
            .map(|id| kitsu_anime_detail_with_anilist_base(state, id, Some(base))),
    )
    .await
    .into_iter()
    .map(|d| d.expect("a detail"))
    .collect()
}

fn banner_of(detail: &KitsuAnimeRef) -> Option<String> {
    detail.cover_image.as_ref().and_then(|c| c.original.clone())
}

#[tokio::test]
async fn a_strip_of_ten_coverless_shows_reads_its_banners_in_two_requests() {
    let media: Vec<FakeMedia> = (1..=10)
        .map(|i| FakeMedia {
            id: 100 + i,
            mal: Some(i),
            listed: 0,
        })
        .collect();
    let shows: Vec<(String, Vec<(&'static str, u32)>)> = (1..=10)
        .map(|i| (format!("{}", 7000 + i), vec![("myanimelist/anime", i)]))
        .collect();
    let ids: Vec<String> = shows.iter().map(|(k, _)| k.clone()).collect();
    let kitsu = serve(Kitsu(shows), "GET").await;
    let anilist = serve(
        FakeAniList {
            media: media.clone(),
        },
        "POST",
    )
    .await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let got = read_all(&state, &ids, &anilist.uri()).await;
    for ((id, detail), m) in ids.iter().zip(&got).zip(&media) {
        assert_eq!(banner_of(detail), m.banner(), "kitsu {id}");
        let row = meta_cache_get(&state.cache_pool, &anime_detail_key(id))
            .expect("cache read")
            .expect("the detail is kept");
        let kept: KitsuAnimeRef = serde_json::from_str(&row).expect("a detail");
        assert_eq!(banner_of(&kept), m.banner(), "kitsu {id}");
    }
    assert_eq!(
        asked_of(&anilist).await,
        vec![Asked::ByMal(1), Asked::PageByMal((2..=10).collect())]
    );
}

#[tokio::test]
async fn a_strip_refused_by_anilist_keeps_none_of_its_details() {
    // A refusal is no answer about any banner: each detail is served
    // without one and not kept, so the next read asks again.
    let shows: Vec<(String, Vec<(&'static str, u32)>)> = (1..=4)
        .map(|i| (format!("{}", 7100 + i), vec![("myanimelist/anime", i)]))
        .collect();
    let ids: Vec<String> = shows.iter().map(|(k, _)| k.clone()).collect();
    let kitsu = serve(Kitsu(shows), "GET").await;
    let anilist = serve(
        wiremock::ResponseTemplate::new(429).insert_header("retry-after", "60"),
        "POST",
    )
    .await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let got = read_all(&state, &ids, &anilist.uri()).await;
    for (id, detail) in ids.iter().zip(&got) {
        assert_eq!(banner_of(detail), None);
        assert_eq!(
            meta_cache_get(&state.cache_pool, &anime_detail_key(id)).expect("cache read"),
            None,
            "kitsu {id} was kept"
        );
    }
    let asked = asked_of(&anilist).await;
    assert!(matches!(asked.first(), Some(Asked::ByMal(1))), "{asked:?}");
    assert!(asked.len() <= 3, "{asked:?}");
}

#[tokio::test]
async fn a_lone_detail_keeps_its_single_banner_lookup() {
    let kitsu = serve(
        Kitsu(vec![("7201".into(), vec![("myanimelist/anime", 9)])]),
        "GET",
    )
    .await;
    let media = FakeMedia {
        id: 91,
        mal: Some(9),
        listed: 0,
    };
    let anilist = serve(
        FakeAniList {
            media: vec![media.clone()],
        },
        "POST",
    )
    .await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let detail = kitsu_anime_detail_with_anilist_base(&state, "7201", Some(&anilist.uri()))
        .await
        .expect("a detail");
    assert_eq!(banner_of(&detail), media.banner());
    assert_eq!(asked_of(&anilist).await, vec![Asked::ByMal(9)]);
}
