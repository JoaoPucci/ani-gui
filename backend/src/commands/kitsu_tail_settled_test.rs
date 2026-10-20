//! A refusal the evidence already read settles stands, whatever a
//! later read would have done: a read that fails after it must not
//! hand the listing back to a reading kept from before.
//!
//! Each case serves Noragami's recorded Kitsu answers with one of them
//! changed so that it refuses the reading, keeps the reading an
//! earlier play proved, and makes a read the refusal does not need
//! fail. The listing must come back untagged, and the kept reading
//! forgotten.

use super::*;
use crate::commands::play_native::PickedShow;
use crate::commands::play_native_appended::fit_appended;
use crate::scraper::provider::{BrowseHit, EpisodeRef};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn json(name: &str) -> serde_json::Value {
    serde_json::from_slice(&fixture(name)).expect("recorded json")
}

/// Noragami's Kitsu answers, as recorded.
struct Answers {
    detail: serde_json::Value,
    page: serde_json::Value,
    relations: serde_json::Value,
    /// AniList's answer, cached as a detail page visit leaves it;
    /// `None` leaves it to be read, through Kitsu's mappings.
    airing: Option<AiringStatus>,
}

impl Answers {
    fn recorded() -> Self {
        Self {
            detail: json("kitsu-anime-7881.json"),
            page: json("kitsu-episodes-7881-p1.json"),
            relations: json("kitsu-relations-7881.json"),
            airing: Some(
                crate::meta::anilist_airing::parse_airing_response(&fixture(
                    "anilist-airing-7881.json",
                ))
                .expect("airing")
                .expect("known"),
            ),
        }
    }

    /// The related entry `id`'s attributes.
    fn related_mut(&mut self, id: &str) -> &mut serde_json::Value {
        self.relations["included"]
            .as_array_mut()
            .expect("included")
            .iter_mut()
            .find(|e| e["id"] == id)
            .map(|e| &mut e["attributes"])
            .expect("the related entry")
    }
}

/// Every route a Kitsu read can take answers 500 unless `answers`
/// serves it; the reads a case leaves unserved are the ones that fail.
async fn serve(answers: &Answers, unserved: &[&str]) -> MockServer {
    let kitsu = MockServer::start().await;
    let served = |route: &str| !unserved.contains(&route);
    if served("mappings") {
        Mock::given(method("GET"))
            .and(path("/anime/7881"))
            .and(query_param("include", "mappings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": answers.detail["data"].clone(), "included": []
            })))
            .with_priority(1)
            .mount(&kitsu)
            .await;
    }
    if served("detail") {
        Mock::given(method("GET"))
            .and(path("/anime/7881"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&answers.detail))
            .mount(&kitsu)
            .await;
    }
    if served("page") {
        Mock::given(method("GET"))
            .and(path("/anime/7881/episodes"))
            .and(query_param("page[offset]", "20"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"data": []})))
            .with_priority(1)
            .mount(&kitsu)
            .await;
        Mock::given(method("GET"))
            .and(path("/anime/7881/episodes"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&answers.page))
            .mount(&kitsu)
            .await;
    }
    if served("relations") {
        Mock::given(method("GET"))
            .and(path("/anime/7881/media-relationships"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&answers.relations))
            .mount(&kitsu)
            .await;
    }
    if served("sequel page") {
        Mock::given(method("GET"))
            .and(path("/anime/10800/episodes"))
            .respond_with(
                ResponseTemplate::new(200).set_body_bytes(fixture("kitsu-episodes-10800-p1.json")),
            )
            .mount(&kitsu)
            .await;
    }
    Mock::given(wiremock::matchers::any())
        .respond_with(ResponseTemplate::new(500))
        .with_priority(10)
        .mount(&kitsu)
        .await;
    kitsu
}

/// Noragami's listing as hianime recorded it: twelve episodes and the
/// two OADs after them.
fn noragami() -> PickedShow {
    let body = String::from_utf8(fixture("episode-list-916.json")).expect("utf-8");
    let rows = crate::scraper::hianime::parse_episode_list(&body).expect("recorded listing");
    PickedShow::listed(
        BrowseHit {
            slug: "noragami-916".into(),
            title: "Noragami".into(),
            kind: None,
            jname: None,
        },
        rows,
    )
}

/// Fits Noragami's listing against `answers`, with the reading the
/// recorded evidence proves kept from an earlier play.
async fn fitted(answers: &Answers, unserved: &[&str]) -> (Vec<EpisodeRef>, Option<Remembered>) {
    let kitsu = serve(answers, unserved).await;
    let state = state_with_kitsu(&kitsu.uri());
    if let Some(airing) = &answers.airing {
        crate::cache::meta_cache_put(
            &state.cache_pool,
            "airing:v2:7881",
            &serde_json::to_string(airing).expect("json"),
            60,
        )
        .expect("airing row");
    }
    let source = KitsuTail::for_entry(&state, Some("7881")).expect("an entry");
    let show = noragami();
    source.remember(Some(&Remembered {
        slug: show.hit.slug.clone(),
        count: 12,
        ids: show.episodes[12..].iter().map(|e| e.id).collect(),
    }));
    let fitted = fit_appended(show, Some(12), Some(&source)).await;
    (fitted.episodes, source.remembered())
}

fn untagged(rows: &[EpisodeRef]) -> bool {
    rows.len() == 14 && rows.iter().all(|e| e.number2.is_none())
}

/// The recorded answers prove the reading, so the cases below each
/// refuse it for the one thing they change.
#[tokio::test]
async fn the_recorded_answers_prove_the_reading() {
    let (rows, kept) = fitted(&Answers::recorded(), &[]).await;
    assert!(!untagged(&rows), "{rows:?}");
    assert!(kept.is_some());
}

/// Kitsu now calls the TV sequel airing: whatever its first page says,
/// the rows past the count could be its start, and its page is not read.
#[tokio::test]
async fn a_tv_relation_airing_now_refuses_without_its_page() {
    let mut answers = Answers::recorded();
    answers.related_mut("10800")["status"] = "current".into();
    let (rows, kept) = fitted(&answers, &["sequel page"]).await;
    assert!(untagged(&rows), "{rows:?}");
    assert_eq!(kept, None);
}

/// Kitsu does not call the show finished: AniList is not asked.
#[tokio::test]
async fn a_show_kitsu_does_not_call_finished_refuses_without_more_reads() {
    let mut answers = Answers::recorded();
    answers.detail["data"]["attributes"]["status"] = "current".into();
    answers.airing = None;
    let (rows, kept) = fitted(&answers, &["mappings", "page", "relations"]).await;
    assert!(untagged(&rows), "{rows:?}");
    assert_eq!(kept, None);
}

/// AniList has another episode scheduled: Kitsu's pages are not read.
#[tokio::test]
async fn a_show_anilist_has_not_ended_refuses_without_kitsu_s_pages() {
    let mut answers = Answers::recorded();
    if let Some(airing) = answers.airing.as_mut() {
        airing.next_episode = Some(13);
    }
    let (rows, kept) = fitted(&answers, &["page", "relations"]).await;
    assert!(untagged(&rows), "{rows:?}");
    assert_eq!(kept, None);
}

/// Kitsu's page lists an episode past the count: the relations are not
/// read.
#[tokio::test]
async fn an_episode_kitsu_lists_past_the_count_refuses_without_the_relations() {
    let mut answers = Answers::recorded();
    let mut thirteenth = answers.page["data"][11].clone();
    thirteenth["id"] = "999999".into();
    thirteenth["attributes"]["number"] = 13.into();
    answers.page["data"]
        .as_array_mut()
        .expect("rows")
        .push(thirteenth);
    let (rows, kept) = fitted(&answers, &["relations"]).await;
    assert!(untagged(&rows), "{rows:?}");
    assert_eq!(kept, None);
}

/// No related entry is as long as the rows past the count: nothing
/// explains them, and the TV sequel's page is not read.
#[tokio::test]
async fn rows_no_relation_explains_refuse_without_the_sequel_s_page() {
    let mut answers = Answers::recorded();
    answers.related_mut("8152")["episodeCount"] = 3.into();
    let (rows, kept) = fitted(&answers, &["sequel page"]).await;
    assert!(untagged(&rows), "{rows:?}");
    assert_eq!(kept, None);
}

/// Kitsu says it relates more entries than it listed: the relations
/// are not all known, and the TV sequel's page is not read.
#[tokio::test]
async fn relations_kitsu_did_not_list_whole_refuse_without_the_sequel_s_page() {
    let mut answers = Answers::recorded();
    answers.relations["meta"]["count"] = 5.into();
    let (rows, kept) = fitted(&answers, &["sequel page"]).await;
    assert!(untagged(&rows), "{rows:?}");
    assert_eq!(kept, None);
}

/// A read the reading needs, failing, is no evidence: the kept reading
/// stands, as it did before.
#[tokio::test]
async fn a_read_the_reading_needs_failing_keeps_the_kept_reading() {
    let (rows, kept) = fitted(&Answers::recorded(), &["sequel page"]).await;
    assert!(!untagged(&rows), "{rows:?}");
    assert!(kept.is_some());
}
