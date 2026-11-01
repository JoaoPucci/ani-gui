//! A background fill the gate held back is retried once the pause ends,
//! lands in the cache without the page asking again, and tells the page
//! it landed. Each test shares one gate for its AniList mock, so a 429
//! puts that gate in a pause as it would AniList's own.

use std::path::PathBuf;
use std::time::Duration;

use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::{collect, wait_any};
use crate::app::AppState;
use crate::cache::meta_cache_get;
use crate::meta::anilist_gate::{share_for_tests, ANILIST};

const DETAIL_FIXTURE: &[u8] =
    include_bytes!("../../../tests/fixtures/kitsu/anime_one_piece_detail.json");

const LISTING: &str = r#"{"data":{"Media":{"streamingEpisodes":[{"title":"Episode 1 - One","thumbnail":"https://x.cdn/1.jpg"}]}}}"#;
const BANNER: &str = r#"{"data":{"Media":{"bannerImage":"https://al/b.jpg"}}}"#;

/// Long enough for every retry a test starts to end.
const WAIT: Duration = Duration::from_secs(20);

fn state(kitsu_uri: &str) -> AppState {
    AppState {
        secret: crate::proxy::AppSecret::random(),
        sessions: crate::proxy::SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        host_budget: crate::proxy::host_budget::HostBudget::fresh(),
        meta_http: reqwest::Client::new(),
        proxy_origin: crate::proxy::ProxyOrigin::new("127.0.0.1", 12_345),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path: PathBuf::from("/tmp/ani-gui-held-fill/history"),
        providers: crate::provider_set::ProviderSet::for_tests(),
        image_cache_dir: PathBuf::from("/tmp/ani-gui-held-fill/images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: crate::meta::kitsu::KitsuClient::with_base(reqwest::Client::new(), kitsu_uri),
        config_path: PathBuf::from("/tmp/ani-gui-held-fill/config.toml"),
        state_dir: PathBuf::from("/tmp/ani-gui-held-fill/state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    }
}

/// Kitsu mapping each show to an AniList id, its detail with no cover.
async fn kitsu(shows: &[(&str, u32)]) -> MockServer {
    let server = MockServer::start().await;
    for (kitsu_id, anilist_id) in shows {
        Mock::given(method("GET"))
            .and(path(format!("/anime/{kitsu_id}")))
            .and(query_param("include", "mappings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": { "id": kitsu_id, "type": "anime", "attributes": { "canonicalTitle": "t" } },
                "included": [{
                    "id": "1",
                    "type": "mappings",
                    "attributes": { "externalSite": "anilist/anime", "externalId": anilist_id.to_string() }
                }],
            })))
            .with_priority(1)
            .mount(&server)
            .await;
        let mut detail: serde_json::Value =
            serde_json::from_slice(DETAIL_FIXTURE).expect("fixture");
        detail["data"]["id"] = serde_json::json!(kitsu_id);
        detail["data"]["attributes"]["coverImage"] = serde_json::Value::Null;
        Mock::given(method("GET"))
            .and(path(format!("/anime/{kitsu_id}")))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/vnd.api+json")
                    .set_body_json(detail),
            )
            .mount(&server)
            .await;
    }
    server
}

/// AniList refusing this client `refusals` times (all of them when
/// `None`) with a `retry_after`-second wait, then answering `answer`.
async fn anilist(refusals: Option<u64>, retry_after: u64, answer: &str) -> MockServer {
    let server = MockServer::start().await;
    let refuse = Mock::given(method("POST")).respond_with(
        ResponseTemplate::new(429).insert_header("retry-after", retry_after.to_string().as_str()),
    );
    match refusals {
        Some(0) => {}
        Some(n) => {
            refuse
                .up_to_n_times(n)
                .with_priority(1)
                .mount(&server)
                .await
        }
        None => refuse.with_priority(1).mount(&server).await,
    }
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_string(answer))
        .mount(&server)
        .await;
    server
}

async fn requests(server: &MockServer) -> usize {
    server.received_requests().await.map_or(0, |r| r.len())
}

fn row(state: &AppState, key: &str) -> Option<String> {
    meta_cache_get(&state.cache_pool, key).expect("cache read")
}

async fn thumbs(state: &AppState, kitsu_id: &str, anilist: &MockServer) -> Vec<u64> {
    let uri = anilist.uri();
    collect(
        crate::commands::anilist_eps_thumbs::thumbs_for_show_with_anilist_base(
            state,
            kitsu_id,
            Some(&uri),
        ),
    )
    .await
    .1
}

#[tokio::test]
async fn held_back_thumbnails_land_once_the_pause_ends_without_another_view() {
    let kitsu = kitsu(&[("12", 30)]).await;
    let anilist = anilist(Some(1), 1, LISTING).await;
    let _gate = share_for_tests(&anilist.uri(), ANILIST);
    let state = state(&kitsu.uri());

    let tickets = thumbs(&state, "12", &anilist).await;
    assert_eq!(tickets.len(), 1, "the page is told a retry will come");
    assert_eq!(row(&state, "anilist:eps-thumbs:v2:k12"), None);

    let ended = wait_any(&tickets, WAIT).await;
    assert_eq!(ended, vec![(tickets[0], true)]);
    assert!(row(&state, "anilist:eps-thumbs:v2:k12").is_some());
    assert_eq!(requests(&anilist).await, 2, "the refusal, then one retry");
}

#[tokio::test]
async fn the_same_show_held_twice_is_retried_once() {
    let kitsu = kitsu(&[("13", 31)]).await;
    // A two-second pause: the second view is refused by the gate
    // without a request.
    let anilist = anilist(Some(1), 2, LISTING).await;
    let _gate = share_for_tests(&anilist.uri(), ANILIST);
    let state = state(&kitsu.uri());

    let first = thumbs(&state, "13", &anilist).await;
    let second = thumbs(&state, "13", &anilist).await;
    assert_eq!(first.len(), 1);
    assert_eq!(first, second, "both views wait on one retry");

    assert_eq!(wait_any(&first, WAIT).await, vec![(first[0], true)]);
    assert_eq!(requests(&anilist).await, 2, "the refusal, then one retry");
}

#[tokio::test]
async fn many_held_shows_each_land() {
    let kitsu = kitsu(&[("14", 32), ("15", 33), ("16", 34)]).await;
    // A two-second pause: the next two shows are refused by the gate.
    let anilist = anilist(Some(1), 2, LISTING).await;
    let _gate = share_for_tests(&anilist.uri(), ANILIST);
    let state = state(&kitsu.uri());

    let mut tickets = Vec::new();
    for show in ["14", "15", "16"] {
        tickets.extend(thumbs(&state, show, &anilist).await);
    }
    assert_eq!(tickets.len(), 3, "{tickets:?}");
    let mut landed = Vec::new();
    while landed.len() < tickets.len() {
        let waiting: Vec<u64> = tickets
            .iter()
            .copied()
            .filter(|t| !landed.contains(t))
            .collect();
        for (t, ok) in wait_any(&waiting, WAIT).await {
            assert!(ok, "ticket {t}");
            landed.push(t);
        }
    }
    for show in ["14", "15", "16"] {
        assert!(row(&state, &format!("anilist:eps-thumbs:v2:k{show}")).is_some());
    }
}

#[tokio::test]
async fn a_retry_held_back_every_time_gives_up_after_its_bound_and_caches_nothing() {
    let kitsu = kitsu(&[("17", 35)]).await;
    let anilist = anilist(None, 1, LISTING).await;
    let _gate = share_for_tests(&anilist.uri(), ANILIST);
    let state = state(&kitsu.uri());

    let tickets = thumbs(&state, "17", &anilist).await;
    assert_eq!(tickets.len(), 1);
    assert_eq!(
        wait_any(&tickets, Duration::from_secs(30)).await,
        vec![(tickets[0], false)]
    );
    assert_eq!(row(&state, "anilist:eps-thumbs:v2:k17"), None);
    assert_eq!(
        requests(&anilist).await,
        1 + super::registry::MAX_RETRIES as usize,
        "one retry per pause, up to the bound"
    );
}

#[tokio::test]
async fn a_retry_whose_run_fails_for_another_reason_is_not_run_again() {
    // Held back by a one-second pause, then AniList fails outright: the
    // retry runs once, gives up, and caches nothing.
    let kitsu = kitsu(&[("21", 39)]).await;
    let anilist = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "1"))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&anilist)
        .await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&anilist)
        .await;
    let _gate = share_for_tests(&anilist.uri(), ANILIST);
    let state = state(&kitsu.uri());

    let tickets = thumbs(&state, "21", &anilist).await;
    assert_eq!(tickets.len(), 1);
    assert_eq!(wait_any(&tickets, WAIT).await, vec![(tickets[0], false)]);
    assert_eq!(requests(&anilist).await, 2, "the refusal, then one retry");
    assert_eq!(row(&state, "anilist:eps-thumbs:v2:k21"), None);
}

#[tokio::test]
async fn a_fill_no_page_was_served_is_not_retried() {
    // A read nobody is waiting on — an import, a guard reading a show's
    // detail — keeps the old rule: the next view asks again.
    let kitsu = kitsu(&[("22", 45)]).await;
    let anilist = anilist(Some(1), 1, LISTING).await;
    let _gate = share_for_tests(&anilist.uri(), ANILIST);
    let state = state(&kitsu.uri());
    let uri = anilist.uri();

    let _ = crate::commands::anilist_eps_thumbs::thumbs_for_show_with_anilist_base(
        &state,
        "22",
        Some(&uri),
    )
    .await;
    tokio::time::sleep(Duration::from_millis(2500)).await;
    assert_eq!(requests(&anilist).await, 1, "no retry went out");
    assert_eq!(row(&state, "anilist:eps-thumbs:v2:k22"), None);
}

#[tokio::test]
async fn a_failure_that_is_not_a_pause_is_not_retried() {
    let kitsu = kitsu(&[("18", 36)]).await;
    let anilist = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&anilist)
        .await;
    let _gate = share_for_tests(&anilist.uri(), ANILIST);
    let state = state(&kitsu.uri());

    assert!(thumbs(&state, "18", &anilist).await.is_empty());
}

#[tokio::test]
async fn an_answered_fill_carries_no_ticket() {
    let kitsu = kitsu(&[("19", 37)]).await;
    let anilist = anilist(Some(0), 1, LISTING).await;
    let _gate = share_for_tests(&anilist.uri(), ANILIST);
    let state = state(&kitsu.uri());

    assert!(thumbs(&state, "19", &anilist).await.is_empty());
    assert!(row(&state, "anilist:eps-thumbs:v2:k19").is_some());
}

#[tokio::test]
async fn a_held_back_banner_lands_with_its_detail_row() {
    let kitsu = kitsu(&[("20", 38)]).await;
    let anilist = anilist(Some(1), 1, BANNER).await;
    let _gate = share_for_tests(&anilist.uri(), ANILIST);
    let state = state(&kitsu.uri());
    let uri = anilist.uri();

    let (detail, tickets) = collect(
        crate::commands::kitsu::kitsu_anime_detail_with_anilist_base(&state, "20", Some(&uri)),
    )
    .await;
    assert!(detail.expect("detail").cover_image.is_none());
    assert_eq!(tickets.len(), 1);
    let key = crate::commands::kitsu::anime_detail_key("20");
    assert_eq!(row(&state, &key), None, "no answer, no row");

    assert_eq!(wait_any(&tickets, WAIT).await, vec![(tickets[0], true)]);
    let body = row(&state, &key).expect("the retry kept the row");
    assert!(body.contains("https://al/b.jpg"), "{body}");
}

#[tokio::test]
async fn a_ticket_never_issued_has_ended_unlanded() {
    assert_eq!(
        wait_any(&[u64::MAX], Duration::from_millis(10)).await,
        vec![(u64::MAX, false)]
    );
}

/// The AniList id each request to `server` asked for, in order.
async fn asked_ids(server: &MockServer) -> Vec<u64> {
    server
        .received_requests()
        .await
        .unwrap_or_default()
        .iter()
        .filter_map(|r| {
            let body: serde_json::Value = serde_json::from_slice(&r.body).ok()?;
            body["variables"]["id"].as_u64()
        })
        .collect()
}

#[tokio::test]
async fn fills_held_on_a_page_left_go_only_after_the_next_pages_fresh_fills_and_are_cached() {
    // The user's page asks for shows 40 and 41 while AniList refuses
    // this client for two seconds; both are held back. The user leaves
    // and opens another page, which asks for 42, 43 and 44 inside the
    // pause. Every fresh fill of the page on screen goes before either
    // leftover retry, and the leftovers still land in the cache.
    let kitsu = kitsu(&[("40", 40), ("41", 41), ("42", 42), ("43", 43), ("44", 44)]).await;
    let anilist = anilist(Some(1), 2, LISTING).await;
    let _gate = share_for_tests(&anilist.uri(), ANILIST);
    let state = state(&kitsu.uri());

    let mut left = thumbs(&state, "40", &anilist).await;
    left.extend(thumbs(&state, "41", &anilist).await);
    assert_eq!(left.len(), 2, "{left:?}");
    super::release(&left);

    // Inside the pause, and late enough that the three, a quarter
    // second apart, all fit in a fill's one-second wait.
    tokio::time::sleep(Duration::from_millis(1700)).await;
    let (c, d, e) = tokio::join!(
        thumbs(&state, "42", &anilist),
        thumbs(&state, "43", &anilist),
        thumbs(&state, "44", &anilist),
    );
    assert!(
        c.is_empty() && d.is_empty() && e.is_empty(),
        "fresh fills answered"
    );

    let deadline = tokio::time::Instant::now() + WAIT;
    while ["40", "41"]
        .iter()
        .any(|s| row(&state, &format!("anilist:eps-thumbs:v2:k{s}")).is_none())
    {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the leftovers never landed"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let ids = asked_ids(&anilist).await;
    let last_fresh = ids
        .iter()
        .rposition(|id| (42..=44).contains(id))
        .expect("fresh");
    let first_retry = ids
        .iter()
        .enumerate()
        .skip(1)
        .find(|&(_, id)| (40..=41).contains(id))
        .map(|(i, _)| i)
        .expect("a retry");
    assert!(last_fresh < first_retry, "fresh fills go first: {ids:?}");
    assert_eq!(
        ids.len(),
        6,
        "the refusal, three fresh fills, two retries: {ids:?}"
    );
}
