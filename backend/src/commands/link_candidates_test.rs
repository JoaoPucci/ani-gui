//! The picker's searches for a guessed row's candidates.

use super::*;
use std::path::PathBuf;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn state_at(uri: &str) -> AppState {
    AppState {
        secret: crate::proxy::AppSecret::random(),
        sessions: crate::proxy::SessionTable::new(),
        proxy_http: reqwest::Client::new(),
        meta_http: reqwest::Client::new(),
        proxy_origin: crate::proxy::ProxyOrigin::new("127.0.0.1", 12_345),
        bundled_bin: None,
        legacy_sweep: crate::legacy_script::SweepReport::default(),
        history_path: PathBuf::from("/y/history"),
        providers: crate::provider_set::ProviderSet::for_tests(),
        image_cache_dir: PathBuf::from("/tmp/ani-gui-images"),
        cache_pool: crate::cache::open_in_memory().expect("in-mem pool"),
        kitsu: crate::meta::kitsu::KitsuClient::with_base(reqwest::Client::new(), uri),
        config_path: PathBuf::from("/tmp/ani-gui-config.toml"),
        state_dir: PathBuf::from("/tmp/ani-gui-state"),
        internal_secret: crate::account::InternalSecret::random(),
        mal_refresh: crate::meta::mal_user::MalRefreshState::new(),
        account_write_locks: crate::commands::account::AccountWriteLocks::new(),
        availability_refreshes: crate::commands::availability_refresh::AvailabilityRefreshes::new(),
        flags: crate::flags::FlagStore::fixed(crate::flags::DECLARED),
        host_budget: crate::proxy::host_budget::HostBudget::fresh(),
    }
}

/// A search answer naming `hits` as `(id, episode count)`.
fn hits(shows: &[(&str, Option<u32>)]) -> ResponseTemplate {
    let data: Vec<serde_json::Value> = shows
        .iter()
        .map(|(id, count)| {
            serde_json::json!({
                "id": id, "type": "anime",
                "attributes": { "canonicalTitle": format!("Show {id}"), "subtype": "TV", "episodeCount": count }
            })
        })
        .collect();
    ResponseTemplate::new(200)
        .insert_header("content-type", "application/vnd.api+json")
        .set_body_json(serde_json::json!({ "data": data }))
}

async fn answer(mock: &MockServer, text: &str, response: ResponseTemplate) {
    Mock::given(method("GET"))
        .and(path("/anime"))
        .and(query_param("filter[text]", text))
        .respond_with(response)
        .mount(mock)
        .await;
}

fn current(id: &str) -> KitsuAnimeRef {
    serde_json::from_value(serde_json::json!({
        "id": id, "canonical_title": format!("Show {id}"), "slug": null, "synopsis": null,
        "start_date": null, "end_date": null, "episode_count": 1, "average_rating": null,
        "subtype": "special", "status": "finished", "age_rating": null, "popularity_rank": null,
        "poster_image": null, "cover_image": null
    }))
    .expect("a ref as the renderer holds it, titles and all")
}

fn args(query: Option<&str>) -> CandidatesArgs {
    CandidatesArgs {
        row: "hianime:black-clover-949".into(),
        title: "Black Clover".into(),
        current: Some(current("12625")),
        query: query.map(str::to_string),
        episode: 2.0,
    }
}

fn shown(out: &[LinkCandidate]) -> Vec<(&str, bool, usize)> {
    out.iter()
        .map(|c| (c.show.id.as_str(), c.current, c.warnings.len()))
        .collect()
}

#[tokio::test]
async fn the_picker_searches_the_title_and_the_slug_words_the_card_show_first() {
    let mock = MockServer::start().await;
    answer(
        &mock,
        "Black Clover",
        hits(&[("13209", Some(170)), ("12625", Some(1))]),
    )
    .await;
    let state = state_at(&mock.uri());
    // The saved title and the key's words search alike here; another
    // row's words differ.
    let mut asked = args(None);
    asked.row = "hianime:hackers-77".into();
    answer(&mock, "hackers", hits(&[("44242", None)])).await;
    let out = link_candidates(&state, &asked).await.expect("candidates");
    assert_eq!(
        shown(&out),
        [("12625", true, 1), ("13209", false, 0), ("44242", false, 0)]
    );
}

#[tokio::test]
async fn one_search_failing_keeps_what_the_other_found() {
    let mock = MockServer::start().await;
    answer(&mock, "Black Clover", ResponseTemplate::new(503)).await;
    let mut asked = args(None);
    asked.row = "hianime:hackers-77".into();
    answer(&mock, "hackers", hits(&[("44242", None)])).await;
    let state = state_at(&mock.uri());
    let out = link_candidates(&state, &asked).await.expect("candidates");
    assert_eq!(shown(&out), [("12625", true, 1), ("44242", false, 0)]);
}

#[tokio::test]
async fn a_row_whose_key_names_no_words_searches_its_title_alone() {
    let mock = MockServer::start().await;
    answer(&mock, "Old", hits(&[("5", None)])).await;
    let state = state_at(&mock.uri());
    let asked = CandidatesArgs {
        row: "vDTSJHSpYnrkZnAvG".into(),
        title: "Old".into(),
        current: None,
        query: None,
        episode: 1.0,
    };
    let out = link_candidates(&state, &asked).await.expect("candidates");
    assert_eq!(shown(&out), [("5", false, 0)]);
    let requests = mock.received_requests().await.expect("recorded");
    assert_eq!(requests.len(), 1, "one search, the title's");
}

#[tokio::test]
async fn a_typed_search_answers_its_own_hits_marking_the_card_show() {
    let mock = MockServer::start().await;
    answer(
        &mock,
        "jojo 2000",
        hits(&[("7730", Some(13)), ("12625", Some(1))]),
    )
    .await;
    let state = state_at(&mock.uri());
    let out = link_candidates(&state, &args(Some("  jojo 2000 ")))
        .await
        .expect("candidates");
    assert_eq!(shown(&out), [("7730", false, 0), ("12625", true, 1)]);
}

#[tokio::test]
async fn a_typed_search_that_fails_says_so() {
    let mock = MockServer::start().await;
    answer(&mock, "jojo", ResponseTemplate::new(503)).await;
    let state = state_at(&mock.uri());
    assert!(link_candidates(&state, &args(Some("jojo"))).await.is_err());
}
