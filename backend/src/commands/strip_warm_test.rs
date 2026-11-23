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
    // only it was waiting on. The gate keeps half a minute between
    // requests, so the warm's second request waits there when Home
    // leaves.
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
    let gate = crate::meta::anilist_gate::share_for_tests(
        &base,
        crate::meta::anilist_gate::Pace {
            min_gap: Duration::from_secs(30),
            background_wait: Duration::from_secs(60),
            ..crate::meta::anilist_gate::ANILIST
        },
    );
    let queued = crate::held::until("the warm's next request waits for its turn", || {
        gate.waiting() > 0
    });
    tokio::select! {
        () = warm_strip(&state, &ids, Some(&base)) => panic!("the warm ended inside the gap"),
        () = queued => {}
    }
    crate::held::until("the warm's requests leave the queue", || {
        gate.waiting() == 0
    })
    .await;
    assert_eq!(gate.let_go().len(), 1, "requests sent for nobody");
    let asked = asked_of(&anilist).await;
    assert!(asked.len() <= 1, "requests sent for nobody: {asked:?}");
}

/// A Kitsu answering one coverless show, `8001`, MAL id 1 — or, with
/// `cover`, the same show with a cover of its own.
struct OneShow {
    cover: bool,
}

impl wiremock::Respond for OneShow {
    fn respond(&self, request: &wiremock::Request) -> wiremock::ResponseTemplate {
        let mut answer = Kitsu.respond(request);
        if self.cover && !request.url.query().unwrap_or("").contains("mappings") {
            let mut detail: serde_json::Value =
                serde_json::from_slice(NULL_COVER_DETAIL).expect("fixture");
            detail["data"]["id"] = serde_json::json!("801");
            detail["data"]["attributes"]["coverImage"] =
                serde_json::json!({ "original": "https://media.kitsu.app/c/801.jpg" });
            answer = wiremock::ResponseTemplate::new(200)
                .insert_header("content-type", "application/vnd.api+json")
                .set_body_json(detail);
        }
        answer
    }
}

#[tokio::test]
async fn a_warmed_banner_answer_is_read_once_then_asked_again_as_the_single_path_would() {
    // The warm found no banner for the show. The detail read takes that
    // answer and keeps it in its own row; once that row has gone, as an
    // airing show's does after a day, the next read asks AniList again
    // rather than read the warm's answer for a month.
    let kitsu = serve(OneShow { cover: false }, "GET").await;
    let anilist = serve(
        FakeAniList {
            media: vec![FakeMedia {
                id: 101,
                mal: Some(1),
                listed: 0,
            }],
        },
        "POST",
    )
    .await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let base = anilist.uri();
    crate::commands::anilist_banner_memo::remember_warmed(&state.cache_pool, "801", None);
    let first =
        crate::commands::kitsu::kitsu_anime_detail_with_anilist_base(&state, "801", Some(&base))
            .await
            .expect("a detail");
    assert!(first.cover_image.is_none());
    assert!(
        asked_for_banners(&anilist).await.is_empty(),
        "the warm's answer was not read"
    );
    crate::cache::meta_cache_delete(
        &state.cache_pool,
        &crate::commands::kitsu::anime_detail_key("801"),
    )
    .expect("expire the detail");
    let again =
        crate::commands::kitsu::kitsu_anime_detail_with_anilist_base(&state, "801", Some(&base))
            .await
            .expect("a detail");
    assert_eq!(
        again.cover_image.and_then(|c| c.original).as_deref(),
        Some("https://al.cdn/banner/101.jpg")
    );
    assert_eq!(asked_for_banners(&anilist).await.len(), 1);
}

#[tokio::test]
async fn a_show_with_a_kitsu_cover_never_takes_the_warmed_banner() {
    let kitsu = serve(OneShow { cover: true }, "GET").await;
    let anilist = serve(FakeAniList::default(), "POST").await;
    let state = state_with_kitsu_at(&kitsu.uri());
    crate::commands::anilist_banner_memo::remember_warmed(
        &state.cache_pool,
        "801",
        Some("https://al.cdn/banner/warmed.jpg"),
    );
    let detail = crate::commands::kitsu::kitsu_anime_detail_with_anilist_base(
        &state,
        "801",
        Some(&anilist.uri()),
    )
    .await
    .expect("a detail");
    assert_eq!(
        detail.cover_image.and_then(|c| c.original).as_deref(),
        Some("https://media.kitsu.app/c/801.jpg")
    );
    assert_eq!(detail.anilist_banner, None);
}

#[tokio::test]
async fn a_warmed_banner_since_found_gone_is_forgotten_and_asked_again() {
    let kitsu = serve(OneShow { cover: false }, "GET").await;
    let anilist = serve(
        FakeAniList {
            media: vec![FakeMedia {
                id: 101,
                mal: Some(1),
                listed: 0,
            }],
        },
        "POST",
    )
    .await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let gone = "https://al.cdn/banner/gone.jpg";
    crate::commands::anilist_banner_memo::remember_warmed(&state.cache_pool, "801", Some(gone));
    crate::meta::images::mark_dead(&state.cache_pool, gone);
    assert_eq!(
        crate::commands::anilist_banner_memo::warmed(&state.cache_pool, "801"),
        None
    );
    let detail = crate::commands::kitsu::kitsu_anime_detail_with_anilist_base(
        &state,
        "801",
        Some(&anilist.uri()),
    )
    .await
    .expect("a detail");
    assert_eq!(
        detail.cover_image.and_then(|c| c.original).as_deref(),
        Some("https://al.cdn/banner/101.jpg")
    );
}

#[tokio::test]
async fn a_card_reading_its_detail_while_the_warm_asks_for_its_banner_shares_that_lookup() {
    // The warm's banner page is held. A card whose detail is read while
    // it is out takes the warm's answer instead of asking AniList for
    // its banner on its own.
    let (ids, fake) = strip();
    let kitsu = serve(Kitsu, "GET").await;
    let anilist = wiremock::MockServer::start().await;
    let (hold, held) = crate::held::gate();
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(HeldBanners(hold, fake.clone()))
        .mount(&anilist)
        .await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let base = anilist.uri();
    let warm = warm_strip(&state, &ids, Some(&base));
    let card = async {
        held.asked().await;
        let mut card = std::pin::pin!(
            crate::commands::kitsu::kitsu_anime_detail_with_anilist_base(
                &state,
                &ids[0],
                Some(&base)
            )
        );
        let joined = crate::held::until("the card waits on the warm's banner lookup", || {
            crate::commands::anilist_banner_fill::sharing(&state, &ids[0], Some(&base)) > 0
        });
        tokio::select! {
            _ = &mut card => panic!("the card read its detail before the warm's banner answered"),
            () = joined => {}
        }
        held.release();
        card.await.expect("a detail")
    };
    let ((), detail) = tokio::join!(warm, card);
    assert_eq!(
        detail.cover_image.and_then(|c| c.original),
        fake.media[0].banner()
    );
    assert_eq!(
        asked_for_banners(&anilist).await.len(),
        1,
        "{:?}",
        asked_for_banners(&anilist).await
    );
}

/// The stand-in answering banner queries once the test lets them
/// through.
struct HeldBanners(crate::held::Hold, FakeAniList);

impl wiremock::Respond for HeldBanners {
    fn respond(&self, request: &wiremock::Request) -> wiremock::ResponseTemplate {
        if String::from_utf8_lossy(&request.body).contains("bannerImage") {
            self.0.wait_blocking();
        }
        self.1.respond(request)
    }
}
