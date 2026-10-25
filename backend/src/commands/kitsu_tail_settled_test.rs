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
    // Kitsu's mappings, which AniList's answer is read through, share
    // the entry's route; unserved, they fail rather than fall through
    // to the entry's answer.
    let mappings = if served("mappings") {
        ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": answers.detail["data"].clone(), "included": []
        }))
    } else {
        ResponseTemplate::new(500)
    };
    Mock::given(method("GET"))
        .and(path("/anime/7881"))
        .and(query_param("include", "mappings"))
        .respond_with(mappings)
        .with_priority(1)
        .mount(&kitsu)
        .await;
    if served("detail") {
        Mock::given(method("GET"))
            .and(path("/anime/7881"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&answers.detail))
            .mount(&kitsu)
            .await;
    }
    // The page after the count's, which the reader looks ahead to: it
    // fails whenever the pages are unserved, or it alone is.
    let ahead = if served("page") && served("look-ahead") {
        ResponseTemplate::new(200).set_body_json(serde_json::json!({"data": []}))
    } else {
        ResponseTemplate::new(500)
    };
    Mock::given(method("GET"))
        .and(path("/anime/7881/episodes"))
        .and(query_param("page[offset]", "20"))
        .respond_with(ahead)
        .with_priority(1)
        .mount(&kitsu)
        .await;
    if served("page") {
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
    fitted_listing(answers, unserved, noragami()).await
}

/// [`fitted`] for `show`, a listing of Noragami's as hianime might
/// list it.
async fn fitted_listing(
    answers: &Answers,
    unserved: &[&str],
    show: PickedShow,
) -> (Vec<EpisodeRef>, Option<Remembered>) {
    let kitsu = serve(answers, unserved).await;
    fit_on(&kitsu, answers, show).await
}

/// Fits `show` against the Kitsu `kitsu` serves, with AniList's answer
/// cached as `answers` gives it and the reading the recorded evidence
/// proves kept from an earlier play.
async fn fit_on(
    kitsu: &MockServer,
    answers: &Answers,
    show: PickedShow,
) -> (Vec<EpisodeRef>, Option<Remembered>) {
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
    // Every listing here runs two rows past its count.
    let at = show.episodes.len() - 2;
    let count = u32::try_from(at).expect("a count");
    source.remember(Some(&Remembered {
        slug: show.hit.slug.clone(),
        count,
        ids: show.episodes[at..].iter().map(|e| e.id).collect(),
    }));
    let fitted = fit_appended(show, Some(count), Some(&source)).await;
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

/// Every read Kitsu's evidence takes.
const EVERY_READ: &[&str] = &["detail", "mappings", "page", "relations", "sequel page"];

/// A row past the count with no name of its own refuses the reading on
/// the listing alone: nothing need be read, and Kitsu being down
/// changes nothing.
#[tokio::test]
async fn a_row_past_the_count_without_a_name_refuses_without_any_read() {
    let mut show = noragami();
    show.episodes[12].title = Some("Episode 13".into());
    let (rows, kept) = fitted_listing(&Answers::recorded(), EVERY_READ, show).await;
    assert!(untagged(&rows), "{rows:?}");
    assert_eq!(kept, None);
}

/// The listing does not name Kitsu's last episodes as Kitsu does: its
/// numbering is not shown to be Kitsu's, and the relations are not
/// read.
#[tokio::test]
async fn a_listing_kitsu_s_last_episodes_do_not_line_up_with_refuses_without_the_relations() {
    let mut show = noragami();
    show.episodes[11].title = Some("Something Else Entirely".into());
    let (rows, kept) = fitted_listing(&Answers::recorded(), &["relations"], show).await;
    assert!(untagged(&rows), "{rows:?}");
    assert_eq!(kept, None);
}

/// A row past the count carries the name of one of the show's own
/// episodes on Kitsu: the relations are not read.
#[tokio::test]
async fn a_row_named_as_one_of_the_show_s_episodes_refuses_without_the_relations() {
    let answers = Answers::recorded();
    let first = answers.page["data"][0]["attributes"]["canonicalTitle"]
        .as_str()
        .expect("Kitsu titles episode 1")
        .to_string();
    let mut show = noragami();
    show.episodes[12].title = Some(first);
    let (rows, kept) = fitted_listing(&answers, &["relations"], show).await;
    assert!(untagged(&rows), "{rows:?}");
    assert_eq!(kept, None);
}

/// One TV relation's page already refuses the reading, here by not
/// titling the episodes the rows would be: the next TV relation's page
/// is not read.
#[tokio::test]
async fn one_tv_relation_s_page_refusing_settles_it_without_the_next() {
    let mut answers = Answers::recorded();
    // A second TV entry Kitsu relates the show to, listed after the
    // sequel, whose page fails.
    let mut link = answers.relations["data"][0].clone();
    link["id"] = "999001".into();
    link["attributes"]["role"] = "other".into();
    link["relationships"]["destination"]["data"]["id"] = "999002".into();
    answers.relations["data"]
        .as_array_mut()
        .expect("data")
        .push(link);
    let mut entry = answers.relations["included"][0].clone();
    entry["id"] = "999002".into();
    answers.relations["included"]
        .as_array_mut()
        .expect("included")
        .push(entry);
    let count = answers.relations["meta"]["count"].as_u64().expect("count");
    answers.relations["meta"]["count"] = (count + 1).into();
    let kitsu_untitled = serde_json::json!({"data": [
        {"id": "1", "type": "episodes", "attributes": {"number": 1, "airdate": "2015-10-02"}},
        {"id": "2", "type": "episodes", "attributes": {"number": 2, "airdate": "2015-10-09"}}
    ]});
    let (rows, kept) = fitted_with_sequel_page(&answers, kitsu_untitled, noragami()).await;
    assert!(untagged(&rows), "{rows:?}");
    assert_eq!(kept, None);
}

/// [`fitted_listing`] with the sequel's page answered by `page`, and
/// every other relation's page failing.
async fn fitted_with_sequel_page(
    answers: &Answers,
    page: serde_json::Value,
    show: PickedShow,
) -> (Vec<EpisodeRef>, Option<Remembered>) {
    let kitsu = serve(answers, &["sequel page"]).await;
    Mock::given(method("GET"))
        .and(path("/anime/10800/episodes"))
        .respond_with(ResponseTemplate::new(200).set_body_json(page))
        .with_priority(1)
        .mount(&kitsu)
        .await;
    fit_on(&kitsu, answers, show).await
}

/// A row past the count named as one of the show's own episodes on the
/// page that holds the count's last ones: the page after it, which can
/// only add names, is not read.
#[tokio::test]
async fn a_row_named_as_one_of_the_show_s_episodes_refuses_without_the_page_after() {
    let answers = Answers::recorded();
    let first = answers.page["data"][0]["attributes"]["canonicalTitle"]
        .as_str()
        .expect("Kitsu titles episode 1")
        .to_string();
    let mut show = noragami();
    show.episodes[12].title = Some(first);
    let (rows, kept) = fitted_listing(&answers, &["look-ahead", "relations"], show).await;
    assert!(untagged(&rows), "{rows:?}");
    assert_eq!(kept, None);
}

/// Every one of Kitsu's last episodes is on the page read, and the
/// listing names one of them otherwise: the page after cannot change
/// that, and is not read.
#[tokio::test]
async fn a_listing_whose_last_episodes_kitsu_names_otherwise_refuses_without_the_page_after() {
    let mut show = noragami();
    show.episodes[11].title = Some("Something Else Entirely".into());
    let (rows, kept) =
        fitted_listing(&Answers::recorded(), &["look-ahead", "relations"], show).await;
    assert!(untagged(&rows), "{rows:?}");
    assert_eq!(kept, None);
}

/// A show Kitsu counts `count` episodes of, every one titled "Story n"
/// and dated, Kitsu's first page holding the first twenty and its
/// second the rest; the listing names them as Kitsu does and runs two
/// rows past the count, the first named as Kitsu's first episode.
fn a_longer_show(count: u32) -> (Answers, PickedShow) {
    let mut answers = Answers::recorded();
    answers.detail["data"]["attributes"]["episodeCount"] = count.into();
    if let Some(airing) = answers.airing.as_mut() {
        airing.aired = Some(count);
        airing.next_episode = None;
    }
    let row = |n: u32| {
        serde_json::json!({"id": n.to_string(), "type": "episodes", "attributes": {
            "number": n, "canonicalTitle": format!("Story {n}"), "airdate": "2020-01-01"}})
    };
    answers.page = serde_json::json!({"data": (1..=count.min(20)).map(row).collect::<Vec<_>>()});
    let mut rows: Vec<EpisodeRef> = (1..=count + 2)
        .map(|n| EpisodeRef {
            id: u64::from(n),
            number: n,
            number2: None,
            title: Some(format!("Story {n}")),
            former_tag: None,
        })
        .collect();
    let at = usize::try_from(count).expect("a count");
    rows[at].title = Some("Story 1".into());
    rows[at + 1].title = Some("A Second Extra".into());
    let show = PickedShow::listed(
        BrowseHit {
            slug: "a-longer-show-1".into(),
            title: "A Longer Show".into(),
            kind: None,
            jname: None,
        },
        rows,
    );
    (answers, show)
}

/// Twenty episodes, the count ending Kitsu's first page: the page after
/// is the second, and a row past the count named as the show's first
/// episode, read on the first, settles the refusal without it.
#[tokio::test]
async fn a_count_ending_a_page_settles_on_that_page_without_the_next() {
    let (answers, show) = a_longer_show(20);
    let (rows, kept) = fitted_listing(&answers, &["look-ahead", "relations"], show).await;
    assert!(rows.iter().all(|e| e.number2.is_none()), "{rows:?}");
    assert_eq!(kept, None);
}

/// Twenty-one episodes, the count's last three straddling Kitsu's first
/// two pages: the refusal the first page settles needs neither the
/// second nor the one after it.
#[tokio::test]
async fn a_refusal_the_first_of_the_count_s_pages_settles_needs_no_second() {
    let (answers, show) = a_longer_show(21);
    let (rows, kept) = fitted_listing(&answers, &["look-ahead", "relations"], show).await;
    assert!(rows.iter().all(|e| e.number2.is_none()), "{rows:?}");
    assert_eq!(kept, None);
}

/// One of the count's last episodes, read, that Kitsu does not date:
/// the listing's numbering is not shown to be Kitsu's, whatever the
/// episodes still to read say, so the pages after are not read.
#[tokio::test]
async fn an_undated_last_episode_read_refuses_without_the_pages_still_to_read() {
    let (mut answers, mut show) = a_longer_show(21);
    answers.page["data"][18]["attributes"]["airdate"] = serde_json::Value::Null;
    show.episodes[21].title = Some("An Extra of Its Own".into());
    let (rows, kept) = fitted_listing(&answers, &["look-ahead", "relations"], show).await;
    assert!(rows.iter().all(|e| e.number2.is_none()), "{rows:?}");
    assert_eq!(kept, None);
}

/// A listing row for one of the count's last episodes with no name
/// cannot line up with Kitsu's: nothing need be read, and Kitsu being
/// down changes nothing.
#[tokio::test]
async fn a_last_episode_the_listing_does_not_name_refuses_without_any_read() {
    let mut show = noragami();
    show.episodes[10].title = None;
    let (rows, kept) = fitted_listing(&Answers::recorded(), EVERY_READ, show).await;
    assert!(untagged(&rows), "{rows:?}");
    assert_eq!(kept, None);
}

/// The listing names each of the count's last episodes only by its
/// number: whatever Kitsu says, nothing shows its numbering to be
/// Kitsu's, so nothing need be read.
#[tokio::test]
async fn last_episodes_named_only_by_number_refuse_without_any_read() {
    let mut show = noragami();
    for row in &mut show.episodes[9..12] {
        row.title = Some(format!("Episode {}", row.number));
    }
    let (rows, kept) = fitted_listing(&Answers::recorded(), EVERY_READ, show).await;
    assert!(untagged(&rows), "{rows:?}");
    assert_eq!(kept, None);
}

/// A listing row for one of the count's last episodes named by
/// punctuation alone names nothing Kitsu's could match.
#[tokio::test]
async fn a_last_episode_named_by_punctuation_alone_refuses_without_any_read() {
    let mut show = noragami();
    show.episodes[10].title = Some("—".into());
    let (rows, kept) = fitted_listing(&Answers::recorded(), EVERY_READ, show).await;
    assert!(untagged(&rows), "{rows:?}");
    assert_eq!(kept, None);
}

/// A row past the count named as the listing names one of the count's
/// last episodes: for the listing to line up, Kitsu must title that
/// episode the same, and then the row carries one of the show's own
/// names. Either way it refuses, so nothing need be read.
#[tokio::test]
async fn a_row_past_the_count_named_as_a_last_episode_refuses_without_any_read() {
    let mut show = noragami();
    show.episodes[12].title = show.episodes[11].title.clone();
    let (rows, kept) = fitted_listing(&Answers::recorded(), EVERY_READ, show).await;
    assert!(untagged(&rows), "{rows:?}");
    assert_eq!(kept, None);
}
