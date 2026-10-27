//! The evidence reader: which Kitsu pages it reads for a count, how
//! it reads Kitsu's and AniList's answers, and that it reads them from
//! the cache a detail page leaves or from Kitsu when the cache has
//! none.

use super::*;
use crate::commands::play_native_appended::{SequelEpisode, TailEpisode};
use crate::meta::anilist_airing::AiringStatus;

fn fixture(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root")
        .join("tests/fixtures/hianime/appended")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

#[test]
fn the_pages_read_hold_the_count_s_last_episodes_and_any_past_it() {
    assert_eq!(tail_pages(1), vec![1]);
    assert_eq!(tail_pages(12), vec![1]);
    assert_eq!(tail_pages(19), vec![1]);
    assert_eq!(tail_pages(20), vec![1, 2], "a count ending a page");
    assert_eq!(tail_pages(21), vec![1, 2], "the last three straddle pages");
    assert_eq!(tail_pages(22), vec![1, 2]);
    assert_eq!(tail_pages(23), vec![2]);
    assert_eq!(tail_pages(40), vec![2, 3]);
    assert_eq!(tail_pages(201), vec![10, 11]);
}

/// The corpus records, for every counted show, the pages holding the
/// count's last episodes (with the page after when the count ends a
/// page), which are the pages the rule's verdict on the recorded data
/// rests on. The reader also asks for the page after a count that does
/// not end a page, which the corpus does not hold; the reader's own
/// tests serve that page.
#[test]
fn the_corpus_holds_the_pages_its_verdicts_rest_on() {
    let corpus: serde_json::Value =
        serde_json::from_slice(&fixture("corpus.json")).expect("corpus");
    for show in corpus["shows"].as_array().expect("shows") {
        let Some(count) = show["episode_count"].as_u64() else {
            assert!(show["kitsu_pages"]
                .as_object()
                .is_some_and(|p| p.is_empty()));
            continue;
        };
        let pages: Vec<u32> = show["kitsu_pages"]
            .as_object()
            .expect("pages")
            .keys()
            .map(|k| k.parse().expect("a page"))
            .collect();
        let mut want = tail_pages(u32::try_from(count).expect("a count"));
        want.sort_unstable();
        let mut pages = pages;
        pages.sort_unstable();
        assert_eq!(pages, want, "{}", show["title"]);
    }
}

#[test]
fn the_recorded_answers_read_as_a_show_that_ended_at_twelve() {
    let detail =
        crate::meta::kitsu::parse_anime_response(&fixture("kitsu-anime-7881.json")).expect("entry");
    let page = crate::meta::kitsu::parse_episodes_response(&fixture("kitsu-episodes-7881-p1.json"))
        .expect("page");
    let airing =
        crate::meta::anilist_airing::parse_airing_response(&fixture("anilist-airing-7881.json"))
            .expect("airing")
            .expect("known");
    let tail = entry_tail(
        detail.status.as_deref(),
        detail.episode_count,
        &airing,
        page,
    );
    assert!(tail.finished);
    assert_eq!(tail.count, Some(12));
    assert_eq!(tail.anilist_count, Some(12));
    assert_eq!(tail.episodes.len(), 12);
    assert_eq!(
        tail.episodes.last(),
        Some(&TailEpisode {
            number: 12,
            title: Some("Scrap of a Memory".into()),
            airdate: Some("2014-03-23".into()),
        })
    );
}

/// AniList's count stands only for a show it has finished: one still
/// scheduling episodes, one not yet aired, and one it knows nothing
/// about give none.
#[test]
fn anilist_s_count_is_the_finished_one() {
    let read = |airing: AiringStatus| entry_tail(None, None, &airing, Vec::new()).anilist_count;
    assert_eq!(
        read(AiringStatus {
            aired: Some(12),
            ..AiringStatus::default()
        }),
        Some(12)
    );
    assert_eq!(
        read(AiringStatus {
            aired: Some(11),
            next_episode: Some(12),
            ..AiringStatus::default()
        }),
        None
    );
    assert_eq!(
        read(AiringStatus {
            aired: Some(0),
            ..AiringStatus::default()
        }),
        None
    );
    assert_eq!(read(AiringStatus::default()), None);
    assert!(!entry_tail(Some("current"), None, &AiringStatus::default(), Vec::new()).finished);
}

/// A provider's listing as hianime recorded it.
fn hianime_listing(id: u32) -> Vec<crate::scraper::provider::EpisodeRef> {
    let body = String::from_utf8(fixture(&format!("episode-list-{id}.json"))).expect("utf-8");
    crate::scraper::hianime::parse_episode_list(&body).expect("recorded listing")
}

/// A listing of `count` episodes, episode `n` named `name(n)`, and
/// one row past the count named as no episode is: the shape the reader
/// is asked about.
fn listing_named(
    count: u32,
    name: impl Fn(u32) -> String,
) -> Vec<crate::scraper::provider::EpisodeRef> {
    (1..=count + 1)
        .map(|n| crate::scraper::provider::EpisodeRef {
            id: u64::from(n),
            number: n,
            number2: None,
            title: Some(if n > count {
                "An Extra of Its Own".to_string()
            } else {
                name(n)
            }),
            former_tag: None,
        })
        .collect()
}

fn state_with_kitsu(base: &str) -> crate::app::AppState {
    use crate::meta::kitsu::KitsuClient;
    use crate::proxy::{AppSecret, ProxyOrigin, SessionTable};
    crate::app::AppState {
        secret: AppSecret::random(),
        sessions: SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        host_budget: crate::proxy::host_budget::HostBudget::fresh(),
        meta_http: reqwest::Client::new(),
        proxy_origin: ProxyOrigin::new("127.0.0.1", 12_345),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path: std::path::PathBuf::from("/tmp/ani-gui-kitsu-tail/history"),
        providers: crate::provider_set::ProviderSet::for_tests(),
        image_cache_dir: std::path::PathBuf::from("/tmp/ani-gui-kitsu-tail/images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: KitsuClient::with_base(reqwest::Client::new(), base),
        config_path: std::path::PathBuf::from("/tmp/ani-gui-kitsu-tail/config.toml"),
        state_dir: std::path::PathBuf::from("/tmp/ani-gui-kitsu-tail/state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
    }
}

#[test]
fn an_entry_needs_an_id() {
    let state = state_with_kitsu("http://127.0.0.1:1");
    assert!(KitsuTail::for_entry(&state, None).is_none());
    assert!(KitsuTail::for_entry(&state, Some("")).is_none());
    let tail = KitsuTail::for_entry(&state, Some("7881")).expect("an entry");
    assert!(format!("{tail:?}").contains("7881"));
}

/// With nothing cached but AniList's answer, the reader asks Kitsu
/// for the entry and the page, and caches the page for the next read.
#[tokio::test]
async fn an_uncached_entry_and_page_are_read_from_kitsu() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    let kitsu = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime/7881"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(fixture("kitsu-anime-7881.json")))
        .mount(&kitsu)
        .await;
    Mock::given(method("GET"))
        .and(path("/anime/7881/episodes"))
        .respond_with(
            ResponseTemplate::new(200).set_body_bytes(fixture("kitsu-episodes-7881-p1.json")),
        )
        .expect(1)
        .mount(&kitsu)
        .await;
    // The page after, empty for a twelve-episode show.
    Mock::given(method("GET"))
        .and(path("/anime/7881/episodes"))
        .and(wiremock::matchers::query_param("page[offset]", "20"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"data": []})))
        .with_priority(1)
        .mount(&kitsu)
        .await;
    // The relations and the sequel's page, which the reader asks for
    // once the entry's own rows are read.
    for (route, file) in [
        (
            "/anime/7881/media-relationships",
            "kitsu-relations-7881.json",
        ),
        ("/anime/10800/episodes", "kitsu-episodes-10800-p1.json"),
    ] {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(fixture(file)))
            .mount(&kitsu)
            .await;
    }
    let state = state_with_kitsu(&kitsu.uri());
    let airing =
        crate::meta::anilist_airing::parse_airing_response(&fixture("anilist-airing-7881.json"))
            .expect("airing")
            .expect("known");
    crate::cache::meta_cache_put(
        &state.cache_pool,
        "airing:v2:7881",
        &serde_json::to_string(&airing).expect("json"),
        60,
    )
    .expect("airing row");
    let source = KitsuTail::for_entry(&state, Some("7881")).expect("an entry");
    let tail = source.tail(&hianime_listing(916), 12).await.expect("read");
    assert!(tail.finished && tail.count == Some(12) && tail.anilist_count == Some(12));
    assert_eq!(tail.episodes.len(), 12);
    // Cached: a second read does not ask Kitsu for the page again.
    let again = source
        .tail(&hianime_listing(916), 12)
        .await
        .expect("read again");
    assert_eq!(again, tail);
}

/// Kitsu failing to answer is no evidence.
#[tokio::test]
async fn an_entry_kitsu_cannot_serve_gives_no_tail() {
    let state = state_with_kitsu("http://127.0.0.1:1");
    let source = KitsuTail::for_entry(&state, Some("7881")).expect("an entry");
    assert_eq!(source.tail(&hianime_listing(916), 12).await, None);
}

/// The tail carries the entries Kitsu relates the show to, read whole,
/// and a TV sequel's first page of titles: Noragami's OVA, two
/// episodes long, and Noragami Aragoto's thirteen titles.
#[tokio::test]
async fn noragami_s_tail_carries_its_relations_and_its_sequel_s_titles() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    let kitsu = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime/7881/episodes"))
        .and(wiremock::matchers::query_param("page[offset]", "20"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"data": []})))
        .with_priority(1)
        .mount(&kitsu)
        .await;
    for (route, file) in [
        ("/anime/7881", "kitsu-anime-7881.json"),
        ("/anime/7881/episodes", "kitsu-episodes-7881-p1.json"),
        (
            "/anime/7881/media-relationships",
            "kitsu-relations-7881.json",
        ),
        ("/anime/10800/episodes", "kitsu-episodes-10800-p1.json"),
    ] {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(fixture(file)))
            .mount(&kitsu)
            .await;
    }
    let state = state_with_kitsu(&kitsu.uri());
    let airing =
        crate::meta::anilist_airing::parse_airing_response(&fixture("anilist-airing-7881.json"))
            .expect("airing")
            .expect("known");
    crate::cache::meta_cache_put(
        &state.cache_pool,
        "airing:v2:7881",
        &serde_json::to_string(&airing).expect("json"),
        60,
    )
    .expect("airing row");
    let tail = KitsuTail::for_entry(&state, Some("7881"))
        .expect("an entry")
        .tail(&hianime_listing(916), 12)
        .await
        .expect("read");
    assert!(tail.related_complete);
    let ova = tail
        .related
        .iter()
        .find(|r| r.subtype.as_deref() == Some("OVA"))
        .expect("the OVA");
    assert_eq!((ova.anime, ova.episode_count), (true, Some(2)));
    assert_eq!(ova.first_episodes, None);
    let sequel = tail
        .related
        .iter()
        .find(|r| r.role == "sequel")
        .expect("the sequel");
    let episodes = sequel
        .first_episodes
        .as_ref()
        .expect("the sequel's episodes");
    assert_eq!(episodes.len(), 13);
    assert_eq!(
        episodes[0],
        SequelEpisode {
            number: 1,
            title: Some("Bearing a Posthumous Name".into()),
        }
    );
    assert!(
        tail.related.iter().any(|r| !r.anime),
        "the manga is related too"
    );
}

/// Kitsu's pages are not always dense: a duplicated row pushes the
/// count's last episode to the end of its page and the next episode
/// onto the page after. A page read full to the end, with nothing past
/// the count on it, is followed by the next one, so an episode Kitsu
/// lists past the count is seen.
#[tokio::test]
async fn a_full_page_ending_at_the_count_is_followed_by_the_next() {
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    let row = |i: usize, n: u32| {
        serde_json::json!({"id": i.to_string(), "type": "episodes", "attributes": {
            "number": n, "canonicalTitle": format!("Story {n}"), "airdate": "2020-01-01"}})
    };
    // Page one: 1..=18, 18 again, 19. Page two: 20.
    let first: Vec<serde_json::Value> = (1..=18u32)
        .chain([18, 19])
        .enumerate()
        .map(|(i, n)| row(i, n))
        .collect();
    let kitsu = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime/9/episodes"))
        .and(query_param("page[offset]", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"data": first})))
        .mount(&kitsu)
        .await;
    Mock::given(method("GET"))
        .and(path("/anime/9/episodes"))
        .and(query_param("page[offset]", "20"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({"data": [row(20, 20)]})),
        )
        .mount(&kitsu)
        .await;
    Mock::given(method("GET"))
        .and(path("/anime/9/media-relationships"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"data": [], "meta": {"count": 0}})),
        )
        .mount(&kitsu)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let mut detail =
        crate::meta::kitsu::parse_anime_response(&fixture("kitsu-anime-7881.json")).expect("entry");
    detail.id = "9".into();
    detail.episode_count = Some(19);
    crate::cache::meta_cache_put(
        &state.cache_pool,
        &crate::commands::kitsu::anime_detail_key("9"),
        &serde_json::to_string(&detail).expect("json"),
        60,
    )
    .expect("detail row");
    crate::cache::meta_cache_put(
        &state.cache_pool,
        "airing:v2:9",
        &serde_json::to_string(&AiringStatus {
            aired: Some(19),
            ..AiringStatus::default()
        })
        .expect("json"),
        60,
    )
    .expect("airing row");
    let tail = KitsuTail::for_entry(&state, Some("9"))
        .expect("an entry")
        .tail(&listing_named(19, |n| format!("Story {n}")), 19)
        .await
        .expect("read");
    assert!(tail.read_past_count);
    assert!(
        tail.episodes.iter().any(|e| e.number == 20),
        "{:?}",
        tail.episodes
    );
}

/// Kitsu keeps placeholder rows mid-list, which the reader drops: a
/// page of twenty rows can read as seventeen. The page after the last
/// one read is read whatever its length, so an episode Kitsu lists
/// past the count is seen however the page before it was filled.
#[tokio::test]
async fn the_page_after_is_read_however_full_the_last_one_looked() {
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    let row = |i: usize, n: u32, real: bool| {
        let (title, date) = if real {
            (
                serde_json::json!(format!("Story {n}")),
                serde_json::json!("2020-01-01"),
            )
        } else {
            (serde_json::Value::Null, serde_json::Value::Null)
        };
        serde_json::json!({"id": i.to_string(), "type": "episodes", "attributes": {
            "number": n, "canonicalTitle": title, "airdate": date}})
    };
    // Page one: 1..=19 with 3, 5 and 6 placeholders, then 19 again —
    // twenty rows, seventeen real. Page two: 20.
    let first: Vec<serde_json::Value> = (1..=19u32)
        .chain([19])
        .enumerate()
        .map(|(i, n)| row(i, n, ![3, 5, 6].contains(&n)))
        .collect();
    let kitsu = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime/9/episodes"))
        .and(query_param("page[offset]", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"data": first})))
        .mount(&kitsu)
        .await;
    Mock::given(method("GET"))
        .and(path("/anime/9/episodes"))
        .and(query_param("page[offset]", "20"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"data": [row(20, 20, true)]})),
        )
        .mount(&kitsu)
        .await;
    Mock::given(method("GET"))
        .and(path("/anime/9/media-relationships"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"data": [], "meta": {"count": 0}})),
        )
        .mount(&kitsu)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let mut detail =
        crate::meta::kitsu::parse_anime_response(&fixture("kitsu-anime-7881.json")).expect("entry");
    detail.id = "9".into();
    detail.episode_count = Some(19);
    crate::cache::meta_cache_put(
        &state.cache_pool,
        &crate::commands::kitsu::anime_detail_key("9"),
        &serde_json::to_string(&detail).expect("json"),
        60,
    )
    .expect("detail row");
    crate::cache::meta_cache_put(
        &state.cache_pool,
        "airing:v2:9",
        &serde_json::to_string(&AiringStatus {
            aired: Some(19),
            ..AiringStatus::default()
        })
        .expect("json"),
        60,
    )
    .expect("airing row");
    let tail = KitsuTail::for_entry(&state, Some("9"))
        .expect("an entry")
        .tail(&listing_named(19, |n| format!("Story {n}")), 19)
        .await
        .expect("read");
    assert!(
        tail.episodes.iter().any(|e| e.number == 20),
        "{:?}",
        tail.episodes
    );
}

/// A count that ends a page already has the page after it among the
/// pages read; that page coming back empty is the read reaching past
/// the count, and no further page is asked for — a failure on one
/// would cost the whole read.
#[tokio::test]
async fn a_count_that_ends_a_page_reads_no_page_past_the_look_ahead() {
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    let row = |n: u32| {
        serde_json::json!({"id": n.to_string(), "type": "episodes", "attributes": {
            "number": n, "canonicalTitle": format!("Story {n}"), "airdate": "2020-01-01"}})
    };
    let first: Vec<serde_json::Value> = (1..=20u32).map(row).collect();
    let kitsu = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime/9/episodes"))
        .and(query_param("page[offset]", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"data": first})))
        .mount(&kitsu)
        .await;
    Mock::given(method("GET"))
        .and(path("/anime/9/episodes"))
        .and(query_param("page[offset]", "20"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"data": []})))
        .mount(&kitsu)
        .await;
    Mock::given(method("GET"))
        .and(path("/anime/9/episodes"))
        .and(query_param("page[offset]", "40"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&kitsu)
        .await;
    Mock::given(method("GET"))
        .and(path("/anime/9/media-relationships"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"data": [], "meta": {"count": 0}})),
        )
        .mount(&kitsu)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let mut detail =
        crate::meta::kitsu::parse_anime_response(&fixture("kitsu-anime-7881.json")).expect("entry");
    detail.id = "9".into();
    detail.episode_count = Some(20);
    crate::cache::meta_cache_put(
        &state.cache_pool,
        &crate::commands::kitsu::anime_detail_key("9"),
        &serde_json::to_string(&detail).expect("json"),
        60,
    )
    .expect("detail row");
    crate::cache::meta_cache_put(
        &state.cache_pool,
        "airing:v2:9",
        &serde_json::to_string(&AiringStatus {
            aired: Some(20),
            ..AiringStatus::default()
        })
        .expect("json"),
        60,
    )
    .expect("airing row");
    let tail = KitsuTail::for_entry(&state, Some("9"))
        .expect("an entry")
        .tail(&listing_named(20, |n| format!("Story {n}")), 20)
        .await
        .expect("read without a third page");
    assert_eq!(tail.episodes.len(), 20);
}

/// A sequel's page is read by episode number, holes and all: an episode
/// Kitsu dated but did not title is there without a title, one it
/// listed with neither — a placeholder the parser drops — is not there
/// at all, and one it titled but did not date is there with its title.
/// Compacting the titles would read episode 2's title as the first.
#[tokio::test]
async fn a_sequel_page_keeps_which_episodes_lack_titles() {
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    let kitsu = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime/7881/episodes"))
        .and(query_param("page[offset]", "20"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"data": []})))
        .with_priority(1)
        .mount(&kitsu)
        .await;
    for (route, file) in [
        ("/anime/7881", "kitsu-anime-7881.json"),
        ("/anime/7881/episodes", "kitsu-episodes-7881-p1.json"),
        (
            "/anime/7881/media-relationships",
            "kitsu-relations-7881.json",
        ),
    ] {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(fixture(file)))
            .mount(&kitsu)
            .await;
    }
    // Aragoto's page in every shape a row of Kitsu's takes: episode 1
    // dated but untitled, episode 2 titled and dated, episode 3 a
    // placeholder with neither, episode 4 titled but undated.
    Mock::given(method("GET"))
        .and(path("/anime/10800/episodes"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"data": [
            {"id": "1", "type": "episodes", "attributes": {"number": 1, "canonicalTitle": null, "airdate": "2015-10-03"}},
            {"id": "2", "type": "episodes", "attributes": {"number": 2, "canonicalTitle": "One of Her Memories", "airdate": "2015-10-10"}},
            {"id": "3", "type": "episodes", "attributes": {"number": 3, "canonicalTitle": null, "airdate": null}},
            {"id": "4", "type": "episodes", "attributes": {"number": 4, "canonicalTitle": "Wish", "airdate": null}},
        ]})))
        .mount(&kitsu)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let airing =
        crate::meta::anilist_airing::parse_airing_response(&fixture("anilist-airing-7881.json"))
            .expect("airing")
            .expect("known");
    crate::cache::meta_cache_put(
        &state.cache_pool,
        "airing:v2:7881",
        &serde_json::to_string(&airing).expect("json"),
        60,
    )
    .expect("airing row");
    let tail = KitsuTail::for_entry(&state, Some("7881"))
        .expect("an entry")
        .tail(&hianime_listing(916), 12)
        .await
        .expect("read");
    let sequel = tail
        .related
        .iter()
        .find(|r| r.role == "sequel")
        .expect("the sequel");
    assert_eq!(
        sequel.first_episodes,
        Some(vec![
            SequelEpisode {
                number: 1,
                title: None,
            },
            SequelEpisode {
                number: 2,
                title: Some("One of Her Memories".into()),
            },
            SequelEpisode {
                number: 4,
                title: Some("Wish".into()),
            },
        ])
    );
}

/// Kitsu relates Shangri-La Frontier Season 2 to its upcoming third
/// season, whose first page is twenty placeholder rows with neither
/// title nor date. The rule passes an announced sequel over, so its
/// page is not asked for: asked for, a rate limit or an error on that
/// request would void a read that needs nothing from it.
#[tokio::test]
async fn an_announced_sequel_s_page_is_not_asked_for() {
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    let announced =
        crate::meta::kitsu::parse_episodes_response(&fixture("kitsu-episodes-49778-p1.json"))
            .expect("the announced season's page");
    assert!(announced.is_empty(), "nothing real on it: {announced:?}");
    let kitsu = MockServer::start().await;
    for (route, offset, file) in [
        ("/anime/48677", None, "kitsu-anime-48677.json"),
        (
            "/anime/48677/episodes",
            Some("40"),
            "kitsu-episodes-48677-p3.json",
        ),
        // The prequel Kitsu relates it to, an aired TV entry whose
        // page the reader asks for.
        (
            "/anime/46241/episodes",
            None,
            "kitsu-episodes-46241-p1.json",
        ),
    ] {
        let mock = Mock::given(method("GET")).and(path(route));
        let mock = match offset {
            Some(o) => mock.and(query_param("page[offset]", o)),
            None => mock,
        };
        mock.respond_with(ResponseTemplate::new(200).set_body_bytes(fixture(file)))
            .mount(&kitsu)
            .await;
    }
    // Kitsu's second page as recorded lists 21 to 25 with neither title
    // nor date, which no listing lines up with: titled and dated here,
    // so the season's own episodes leave the reading open and the
    // relations are read.
    let mut second: serde_json::Value =
        serde_json::from_slice(&fixture("kitsu-episodes-48677-p2.json")).expect("page two");
    for row in second["data"].as_array_mut().expect("rows") {
        let n = row["attributes"]["number"].as_u64().expect("numbered");
        row["attributes"]["canonicalTitle"] = format!("Season Two Story {n}").into();
        row["attributes"]["airdate"] = "2025-03-01".into();
    }
    Mock::given(method("GET"))
        .and(path("/anime/48677/episodes"))
        .and(query_param("page[offset]", "20"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&second))
        .mount(&kitsu)
        .await;
    // The relations as recorded, with a finished one-episode OVA added
    // to explain the row past the count: without one the reader stops
    // before any relation's page, and the announced season's page not
    // being asked for would say nothing.
    let mut relations: serde_json::Value =
        serde_json::from_slice(&fixture("kitsu-relations-48677.json")).expect("relations");
    // The announced sequel first: Kitsu lists the aired prequel ahead
    // of it, and the prequel's page titles none of its episodes, so a
    // reader that stops at the first page refusing the rows would pass
    // the sequel over for that and not for being announced.
    relations["data"]
        .as_array_mut()
        .expect("data")
        .sort_by_key(|link| link["attributes"]["role"] != "sequel");
    let mut link = relations["data"][0].clone();
    link["id"] = "999001".into();
    link["attributes"]["role"] = "side_story".into();
    link["relationships"]["destination"]["data"]["id"] = "999002".into();
    relations["data"].as_array_mut().expect("data").push(link);
    let mut ova = relations["included"]
        .as_array()
        .expect("included")
        .iter()
        .find(|e| e["type"] == "anime")
        .expect("an anime entry")
        .clone();
    ova["id"] = "999002".into();
    ova["attributes"]["subtype"] = "OVA".into();
    ova["attributes"]["status"] = "finished".into();
    ova["attributes"]["episodeCount"] = 1.into();
    relations["included"]
        .as_array_mut()
        .expect("included")
        .push(ova);
    let count = relations["meta"]["count"].as_u64().expect("count");
    relations["meta"]["count"] = (count + 1).into();
    Mock::given(method("GET"))
        .and(path("/anime/48677/media-relationships"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&relations))
        .mount(&kitsu)
        .await;
    Mock::given(method("GET"))
        .and(path("/anime/49778/episodes"))
        .respond_with(ResponseTemplate::new(503))
        .expect(0)
        .mount(&kitsu)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    crate::cache::meta_cache_put(
        &state.cache_pool,
        "airing:v2:48677",
        &serde_json::to_string(&AiringStatus {
            aired: Some(25),
            ..AiringStatus::default()
        })
        .expect("json"),
        60,
    )
    .expect("airing row");
    let tail = KitsuTail::for_entry(&state, Some("48677"))
        .expect("an entry")
        .tail(&listing_named(25, |n| format!("Season Two Story {n}")), 25)
        .await
        .expect("read without the announced season's page");
    assert!(tail.finished && tail.count == Some(25));
    let sequel = tail
        .related
        .iter()
        .find(|r| r.role == "sequel")
        .expect("the announced sequel");
    assert_eq!(sequel.status.as_deref(), Some("upcoming"));
    assert_eq!(sequel.first_episodes, None);
    let prequel = tail
        .related
        .iter()
        .find(|r| r.role == "prequel")
        .expect("the aired prequel");
    assert!(
        prequel.first_episodes.is_some(),
        "the aired prequel's page is read beside it"
    );
}

/// Every aired TV entry Kitsu relates the show to has its first page
/// read, whatever the role: Noragami Aragoto's one TV relation is its
/// prequel, and its page comes back with Noragami's twelve titled
/// episodes; the OVA Kitsu relates as `other` gets none.
#[tokio::test]
async fn every_aired_tv_relation_s_page_is_read() {
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    let kitsu = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime/10800/episodes"))
        .and(query_param("page[offset]", "20"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"data": []})))
        .with_priority(1)
        .mount(&kitsu)
        .await;
    for (route, file) in [
        ("/anime/10800", "kitsu-anime-10800.json"),
        ("/anime/10800/episodes", "kitsu-episodes-10800-p1.json"),
        (
            "/anime/10800/media-relationships",
            "kitsu-relations-10800.json",
        ),
        ("/anime/7881/episodes", "kitsu-episodes-7881-p1.json"),
    ] {
        Mock::given(method("GET"))
            .and(path(route))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(fixture(file)))
            .mount(&kitsu)
            .await;
    }
    let state = state_with_kitsu(&kitsu.uri());
    let airing =
        crate::meta::anilist_airing::parse_airing_response(&fixture("anilist-airing-10800.json"))
            .expect("airing")
            .expect("known");
    crate::cache::meta_cache_put(
        &state.cache_pool,
        "airing:v2:10800",
        &serde_json::to_string(&airing).expect("json"),
        60,
    )
    .expect("airing row");
    let tail = KitsuTail::for_entry(&state, Some("10800"))
        .expect("an entry")
        .tail(&hianime_listing(913), 13)
        .await
        .expect("read");
    let prequel = tail
        .related
        .iter()
        .find(|r| r.role == "prequel")
        .expect("the prequel");
    let page = prequel.first_episodes.as_ref().expect("the prequel's page");
    assert_eq!(page.len(), 12);
    assert_eq!(
        page[0],
        SequelEpisode {
            number: 1,
            title: Some("A Housecat, a Stray God, and a Tail".into()),
        }
    );
    let ova = tail
        .related
        .iter()
        .find(|r| r.subtype.as_deref() == Some("OVA"))
        .expect("the OVA");
    assert_eq!(ova.first_episodes, None);
}

/// A reading master kept (`appended:v1:`) was proved by a rule that
/// checked the rows past the count against a TV sequel's names alone,
/// and turned away only a sequel still airing. The rule now reads
/// every aired TV entry Kitsu relates the show to, whatever the role,
/// and refuses an entry of any role airing now, so a reading it proved
/// may be one the rule refuses: with Kitsu titling Slime's TV sequel,
/// The Slime Diaries' rows appended to Slime's first season would read
/// as extras then, and as the Diaries' episodes now. Kept, it would
/// stand for thirty days whenever the evidence cannot be read. It is
/// not remembered, and the key it is kept under has moved past
/// master's.
#[test]
fn a_reading_kept_under_the_sequel_only_rule_is_not_remembered() {
    assert!(
        !kept_key("41024").starts_with("appended:v1:"),
        "the key must have moved past master's v1: {}",
        kept_key("41024")
    );
    let state = state_with_kitsu("http://127.0.0.1:1");
    let before = Remembered {
        slug: "tensei-shitara-slime-datta-ken-77".into(),
        count: 24,
        ids: vec![1, 2, 3],
    };
    crate::cache::meta_cache_put(
        &state.cache_pool,
        "appended:v1:41024",
        &serde_json::to_string(&before).expect("serialize"),
        60,
    )
    .expect("put");
    let tail = KitsuTail::for_entry(&state, Some("41024")).expect("an entry");
    assert_eq!(tail.remembered(), None);
    tail.remember(Some(&before));
    assert_eq!(tail.remembered(), Some(before));
}

#[path = "kitsu_tail_settled_test.rs"]
mod settled;

/// Kitsu relations an earlier build kept under `kitsu:relations:v1`
/// may hold an entry whose destination the page did not include, read
/// then as an entry of no format or status; the parser now refuses such
/// a page, and nothing in a kept row tells which it was. The key moved
/// past v1: a v1 row is not read, and the relations are asked again.
#[tokio::test]
async fn relations_an_earlier_build_kept_are_asked_again() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    let kitsu = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime/7881/media-relationships"))
        .respond_with(
            ResponseTemplate::new(200).set_body_bytes(fixture("kitsu-relations-7881.json")),
        )
        .expect(1)
        .mount(&kitsu)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let unexplained = crate::meta::kitsu::KitsuRelations {
        entries: Vec::new(),
        complete: true,
    };
    crate::cache::meta_cache_put(
        &state.cache_pool,
        "kitsu:relations:v1:7881",
        &serde_json::to_string(&unexplained).expect("json"),
        60,
    )
    .expect("seed");
    let read = crate::commands::kitsu::kitsu_relationships(&state, "7881")
        .await
        .expect("read");
    assert!(!read.entries.is_empty(), "asked Kitsu, not the v1 row");
}

/// Kitsu relations an earlier build kept under `kitsu:relations:v2`
/// carry no destination's start date, and nothing in a kept row says
/// whether one was asked for: read now, every part would be undated,
/// and a part that aired before the one it follows would be read as a
/// later part for the row's whole week. The key moved past v2.
#[tokio::test]
async fn relations_kept_without_their_start_dates_are_asked_again() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    let kitsu = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime/7881/media-relationships"))
        .respond_with(
            ResponseTemplate::new(200).set_body_bytes(fixture("kitsu-relations-7881.json")),
        )
        .expect(1)
        .mount(&kitsu)
        .await;
    let state = state_with_kitsu(&kitsu.uri());
    let undated = crate::meta::kitsu::KitsuRelations {
        entries: Vec::new(),
        complete: true,
    };
    crate::cache::meta_cache_put(
        &state.cache_pool,
        "kitsu:relations:v2:7881",
        &serde_json::to_string(&undated).expect("json"),
        60,
    )
    .expect("seed");
    let read = crate::commands::kitsu::kitsu_relationships(&state, "7881")
        .await
        .expect("read");
    assert!(!read.entries.is_empty(), "asked Kitsu, not the v2 row");
}
