//! The evidence reader: which Kitsu pages it reads for a count, how
//! it reads Kitsu's and AniList's answers, and that it reads them from
//! the cache a detail page leaves or from Kitsu when the cache has
//! none.

use super::*;
use crate::commands::play_native_appended::TailEpisode;
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

/// The corpus records, for every counted show, exactly the pages the
/// reader asks for, so the regression run reads what production reads.
#[test]
fn the_corpus_holds_the_pages_the_reader_reads() {
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
    let tail = source.tail(12).await.expect("read");
    assert!(tail.finished && tail.count == Some(12) && tail.anilist_count == Some(12));
    assert_eq!(tail.episodes.len(), 12);
    // Cached: a second read does not ask Kitsu for the page again.
    let again = source.tail(12).await.expect("read again");
    assert_eq!(again, tail);
}

/// Kitsu failing to answer is no evidence.
#[tokio::test]
async fn an_entry_kitsu_cannot_serve_gives_no_tail() {
    let state = state_with_kitsu("http://127.0.0.1:1");
    let source = KitsuTail::for_entry(&state, Some("7881")).expect("an entry");
    assert_eq!(source.tail(12).await, None);
}

/// The tail carries the entries Kitsu relates the show to, read whole,
/// and a TV sequel's first page of titles: Noragami's OVA, two
/// episodes long, and Noragami Aragoto's thirteen titles.
#[tokio::test]
async fn noragami_s_tail_carries_its_relations_and_its_sequel_s_titles() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    let kitsu = MockServer::start().await;
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
        .tail(12)
        .await
        .expect("read");
    assert!(tail.related_complete);
    let ova = tail
        .related
        .iter()
        .find(|r| r.subtype.as_deref() == Some("OVA"))
        .expect("the OVA");
    assert_eq!((ova.anime, ova.episode_count), (true, Some(2)));
    assert_eq!(ova.episode_titles, None);
    let sequel = tail
        .related
        .iter()
        .find(|r| r.role == "sequel")
        .expect("the sequel");
    let titles = sequel.episode_titles.as_ref().expect("the sequel's titles");
    assert_eq!(titles.len(), 13);
    assert_eq!(titles[0], "Bearing a Posthumous Name");
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
        .tail(19)
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
        .tail(19)
        .await
        .expect("read");
    assert!(
        tail.episodes.iter().any(|e| e.number == 20),
        "{:?}",
        tail.episodes
    );
}
