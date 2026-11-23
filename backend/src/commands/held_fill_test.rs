//! A background fill the gate held back is retried once the pause ends,
//! lands in the cache without the page asking again, and tells the page
//! it landed. Each test shares one gate for its AniList mock, so a 429
//! puts that gate in a pause as it would AniList's own.

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

fn state(kitsu_uri: &str) -> crate::scratch::Scratch<AppState> {
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
        kitsu: crate::meta::kitsu::KitsuClient::with_base(reqwest::Client::new(), kitsu_uri),
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

/// Kitsu mapping each show to an AniList id, its detail with no cover.
async fn kitsu(shows: &[(&str, u32)]) -> MockServer {
    kitsu_after(shows, Duration::ZERO).await
}

/// [`kitsu`], answering each detail after `delay`, so reads overlap.
async fn kitsu_after(shows: &[(&str, u32)], delay: Duration) -> MockServer {
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
                    .set_body_json(detail)
                    .set_delay(delay),
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

/// A page reading a detail another read is already fetching shares
/// that read's request, and a banner held back on it is the page's to
/// wait on as well: the read that ran the request may be one no page
/// asked for (the franchise walk), which queues no retry of its own.
#[tokio::test]
async fn a_page_sharing_a_held_back_read_no_page_asked_for_gets_its_retry() {
    let kitsu = kitsu_after(&[("21", 39)], Duration::from_millis(300)).await;
    let anilist = anilist(Some(1), 1, BANNER).await;
    let _gate = share_for_tests(&anilist.uri(), ANILIST);
    let state = state(&kitsu.uri());
    let uri = anilist.uri();
    let read =
        || crate::commands::kitsu::kitsu_anime_detail_with_anilist_base(&state, "21", Some(&uri));

    let page = async {
        tokio::time::sleep(Duration::from_millis(50)).await;
        collect(read()).await
    };
    let (walk, (page, tickets)) = tokio::join!(read(), page);
    assert!(walk.expect("walk").cover_image.is_none());
    assert!(page.expect("page").cover_image.is_none());
    assert_eq!(requests(&anilist).await, 1, "one banner lookup for both");
    assert_eq!(tickets.len(), 1, "the page waits on the retry");

    assert_eq!(wait_any(&tickets, WAIT).await, vec![(tickets[0], true)]);
    let key = crate::commands::kitsu::anime_detail_key("21");
    let body = row(&state, &key).expect("the retry kept the row");
    assert!(body.contains("https://al/b.jpg"), "{body}");
}

/// Two pages reading one detail at once both wait on the retry of the
/// banner held back on the request they shared.
#[tokio::test]
async fn two_pages_sharing_a_held_back_read_both_get_its_ticket() {
    let kitsu = kitsu_after(&[("22", 40)], Duration::from_millis(300)).await;
    let anilist = anilist(Some(1), 1, BANNER).await;
    let _gate = share_for_tests(&anilist.uri(), ANILIST);
    let state = state(&kitsu.uri());
    let uri = anilist.uri();
    let read =
        || crate::commands::kitsu::kitsu_anime_detail_with_anilist_base(&state, "22", Some(&uri));

    let ((_, first), (_, second)) = tokio::join!(collect(read()), collect(read()));
    assert_eq!(first.len(), 1);
    assert_eq!(first, second, "one retry, one ticket, both pages");
    assert_eq!(wait_any(&first, WAIT).await, vec![(first[0], true)]);
}

/// Two reads at once that find a banner share the one lookup and both
/// carry it.
#[tokio::test]
async fn two_reads_at_once_share_one_banner_lookup() {
    let kitsu = kitsu_after(&[("23", 41)], Duration::from_millis(300)).await;
    let anilist = anilist(Some(0), 1, BANNER).await;
    let _gate = share_for_tests(&anilist.uri(), ANILIST);
    let state = state(&kitsu.uri());
    let uri = anilist.uri();
    let read =
        || crate::commands::kitsu::kitsu_anime_detail_with_anilist_base(&state, "23", Some(&uri));

    let (a, b) = tokio::join!(read(), read());
    for detail in [a, b] {
        assert_eq!(
            detail.expect("detail").anilist_banner.as_deref(),
            Some("https://al/b.jpg")
        );
    }
    assert_eq!(requests(&anilist).await, 1);
}

#[tokio::test]
async fn a_ticket_never_issued_has_ended_unlanded() {
    assert_eq!(
        wait_any(&[u64::MAX], Duration::from_millis(10)).await,
        vec![(u64::MAX, false)]
    );
}

/// The AniList ids each request to `server` asked for, in order: one
/// for a single lookup, the page's for a batched one.
async fn asked_ids(server: &MockServer) -> Vec<Vec<u64>> {
    server
        .received_requests()
        .await
        .unwrap_or_default()
        .iter()
        .filter_map(|r| {
            let body: serde_json::Value = serde_json::from_slice(&r.body).ok()?;
            let vars = &body["variables"];
            match vars["ids"].as_array() {
                Some(ids) => Some(ids.iter().filter_map(serde_json::Value::as_u64).collect()),
                None => vars["id"].as_u64().map(|id| vec![id]),
            }
        })
        .collect()
}

/// AniList's page answering a batched lookup of `ids`, each listed as
/// [`LISTING`] lists its one episode.
fn listing_page(ids: &[u32]) -> String {
    let media: Vec<serde_json::Value> = ids
        .iter()
        .map(|id| {
            serde_json::json!({
                "id": id,
                "episodes": null,
                "status": "RELEASING",
                "streamingEpisodes": [{"title": "Episode 1 - One", "thumbnail": "https://x.cdn/1.jpg"}],
            })
        })
        .collect();
    serde_json::json!({ "data": { "Page": { "pageInfo": { "hasNextPage": false }, "media": media } } })
        .to_string()
}

/// AniList answering a batched lookup with [`listing_page`] of the ids
/// it asked for.
struct PageOfIds;

impl wiremock::Respond for PageOfIds {
    fn respond(&self, request: &wiremock::Request) -> ResponseTemplate {
        let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap_or_default();
        let ids: Vec<u32> = body["variables"]["ids"]
            .as_array()
            .map(|ids| {
                ids.iter()
                    .filter_map(|v| v.as_u64().and_then(|v| u32::try_from(v).ok()))
                    .collect()
            })
            .unwrap_or_default();
        ResponseTemplate::new(200).set_body_string(listing_page(&ids))
    }
}

#[tokio::test]
async fn fills_held_on_a_page_left_go_only_after_the_next_pages_fresh_fills_and_are_cached() {
    // The user's page asks for shows 40 and 41 while AniList refuses
    // this client for two seconds; both are held back. The user leaves
    // and opens another page, which asks for 42, 43 and 44 inside the
    // pause. Every fresh fill of the page on screen goes before either
    // leftover retry, and the leftovers still land in the cache. The
    // three fresh fills miss together: the first goes alone, the other
    // two share one request.
    let kitsu = kitsu(&[("40", 40), ("41", 41), ("42", 42), ("43", 43), ("44", 44)]).await;
    let anilist = anilist(Some(1), 2, LISTING).await;
    Mock::given(method("POST"))
        .and(wiremock::matchers::body_string_contains("\"ids\""))
        .respond_with(PageOfIds)
        .with_priority(2)
        .mount(&anilist)
        .await;
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
    for s in ["42", "43", "44"] {
        assert!(
            row(&state, &format!("anilist:eps-thumbs:v2:k{s}")).is_some(),
            "fresh fill {s} was not kept"
        );
    }
    let ids = asked_ids(&anilist).await;
    let last_fresh = ids
        .iter()
        .rposition(|asked| asked.iter().any(|id| (42..=44).contains(id)))
        .expect("fresh");
    let first_retry = ids
        .iter()
        .enumerate()
        .skip(1)
        .find(|&(_, asked)| asked.iter().any(|id| (40..=41).contains(id)))
        .map(|(i, _)| i)
        .expect("a retry");
    assert!(last_fresh < first_retry, "fresh fills go first: {ids:?}");
    // The refusal, the first fresh fill alone, the other two in one
    // request, two retries.
    assert_eq!(ids.len(), 5, "{ids:?}");
    assert_eq!(ids[0], vec![40]);
    let mut fresh: Vec<u64> = ids[1].iter().chain(&ids[2]).copied().collect();
    fresh.sort_unstable();
    assert_eq!(fresh, vec![42, 43, 44], "{ids:?}");
    assert_eq!((ids[1].len(), ids[2].len()), (1, 2), "{ids:?}");
    assert_eq!(&ids[3..], &[vec![40], vec![41]], "{ids:?}");
}

/// A page that takes a copy of another read's held-back lookup is served
/// that lookup's ticket. After a release, that ticket is on screen again
/// from the moment it is served, as one served by its own hold is.
#[tokio::test]
async fn a_released_ticket_a_page_takes_a_copy_of_is_on_screen_until_it_waits() {
    let state = state("http://127.0.0.1:9");
    let base = Some("http://held-fill-copy.invalid");
    let key = super::key(&state, super::Fill::Thumbs, "copy", base);
    let now = std::time::Instant::now;
    let (ticket, _) = super::registry().hold(&key, now()).expect("room");
    super::release(&[ticket]);
    let ((), tickets) = collect(async {
        super::note_pending(&state, super::Fill::Thumbs, "copy", base);
    })
    .await;
    let on_screen = super::registry().on_screen(ticket, now());
    super::registry().end(&key, ticket, false);
    assert_eq!(tickets, vec![ticket]);
    assert!(on_screen, "served to the page on screen");
}

/// Kitsu as [`kitsu`] for one show, its mappings answered after
/// `delay`, so two thumbnail lookups of the show overlap.
async fn kitsu_slow_mappings(kitsu_id: &str, anilist_id: u32, delay: Duration) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/anime/{kitsu_id}")))
        .and(query_param("include", "mappings"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({
                    "data": { "id": kitsu_id, "type": "anime", "attributes": { "canonicalTitle": "t" } },
                    "included": [{
                        "id": "1",
                        "type": "mappings",
                        "attributes": { "externalSite": "anilist/anime", "externalId": anilist_id.to_string() }
                    }],
                }))
                .set_delay(delay),
        )
        .mount(&server)
        .await;
    server
}

/// A page filling a show's thumbnails while another lookup of the show
/// is already running shares that lookup, and a listing held back on it
/// is the page's to wait on as well: the lookup that ran may be one no
/// page asked for, which queues no retry of its own.
#[tokio::test]
async fn a_page_sharing_a_held_back_lookup_no_page_asked_for_gets_its_retry() {
    let kitsu = kitsu_slow_mappings("31", 50, Duration::from_millis(300)).await;
    let anilist = anilist(Some(1), 1, LISTING).await;
    let _gate = share_for_tests(&anilist.uri(), ANILIST);
    let state = state(&kitsu.uri());
    let uri = anilist.uri();
    let lookup = || {
        crate::commands::anilist_eps_thumbs::thumbs_for_show_with_anilist_base(
            &state,
            "31",
            Some(&uri),
        )
    };

    let page = async {
        tokio::time::sleep(Duration::from_millis(50)).await;
        collect(lookup()).await
    };
    let (unasked, (_, tickets)) = tokio::join!(lookup(), page);
    assert!(unasked.entries.is_empty());
    assert_eq!(requests(&anilist).await, 1, "one AniList request for both");
    assert_eq!(tickets.len(), 1, "the page waits on the retry");

    assert_eq!(wait_any(&tickets, WAIT).await, vec![(tickets[0], true)]);
    assert!(row(&state, "anilist:eps-thumbs:v2:k31").is_some());
    assert_eq!(requests(&anilist).await, 2, "the refusal, then one retry");
}

/// Two pages filling one show's thumbnails at once wait on the one
/// retry of the listing held back on the lookup they shared.
#[tokio::test]
async fn two_pages_sharing_a_held_back_lookup_both_get_its_ticket() {
    let kitsu = kitsu_slow_mappings("32", 51, Duration::from_millis(300)).await;
    let anilist = anilist(Some(1), 1, LISTING).await;
    let _gate = share_for_tests(&anilist.uri(), ANILIST);
    let state = state(&kitsu.uri());

    let (first, second) = tokio::join!(
        thumbs(&state, "32", &anilist),
        thumbs(&state, "32", &anilist)
    );
    assert_eq!(first.len(), 1);
    assert_eq!(first, second, "one retry, one ticket, both pages");
    assert_eq!(wait_any(&first, WAIT).await, vec![(first[0], true)]);
}
