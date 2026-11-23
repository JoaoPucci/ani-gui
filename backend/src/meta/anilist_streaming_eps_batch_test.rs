//! The batched streaming-listing lookup against a stand-in AniList
//! ([`FakeAniList`]): how many requests a set of shows costs, and that
//! each show reads the listing its own single lookup would have.

use super::*;
use crate::meta::anilist_streaming_eps_fake_test::{asked_of, Asked, FakeAniList, FakeMedia};

fn media(id: u32, mal: Option<u32>) -> FakeMedia {
    FakeMedia { id, mal, listed: 2 }
}

async fn serve(fake: FakeAniList) -> wiremock::MockServer {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(fake)
        .mount(&server)
        .await;
    server
}

fn want(mal: Option<u32>, anilist: Option<u32>) -> Want {
    Want { mal, anilist }
}

/// Each answer, unwrapped; a failure fails the test.
fn listings(got: Vec<crate::error::Result<StreamingListing>>) -> Vec<StreamingListing> {
    got.into_iter().map(|r| r.expect("an answer")).collect()
}

#[tokio::test]
async fn ten_shows_by_mal_id_cost_one_request() {
    let fake = FakeAniList {
        media: (1..=10).map(|i| media(100 + i, Some(i))).collect(),
    };
    let server = serve(fake.clone()).await;
    let wants: Vec<Want> = (1..=10).map(|i| want(Some(i), Some(100 + i))).collect();
    let got = listings(
        streaming_listings_batch(&reqwest::Client::new(), &wants, Some(&server.uri())).await,
    );
    for (w, l) in wants.iter().zip(&got) {
        assert_eq!(*l, fake.single_path(w.mal, w.anilist));
    }
    assert_eq!(
        asked_of(&server).await,
        vec![Asked::PageByMal((1..=10).collect())]
    );
}

#[tokio::test]
async fn mixed_mappings_cost_one_request_per_id_space() {
    // Two shows by MAL id, one mapped to AniList alone, and one whose
    // MAL id AniList does not record: the last is asked by AniList id
    // in the same request as the AniList-only show, as the single
    // lookup falls back for it.
    let fake = FakeAniList {
        media: vec![
            media(11, Some(1)),
            media(12, Some(2)),
            media(13, None),
            media(14, Some(99)),
        ],
    };
    let server = serve(fake.clone()).await;
    let wants = vec![
        want(Some(1), Some(11)),
        want(Some(2), None),
        want(None, Some(13)),
        want(Some(4), Some(14)),
    ];
    let got = listings(
        streaming_listings_batch(&reqwest::Client::new(), &wants, Some(&server.uri())).await,
    );
    for (w, l) in wants.iter().zip(&got) {
        assert_eq!(*l, fake.single_path(w.mal, w.anilist));
    }
    assert_eq!(got[3], media(14, Some(99)).listing());
    assert_eq!(
        asked_of(&server).await,
        vec![
            Asked::PageByMal(vec![1, 2, 4]),
            Asked::PageById(vec![13, 14])
        ]
    );
}

#[tokio::test]
async fn a_show_anilist_does_not_list_reads_an_empty_listing() {
    // AniList answered and left the show out: that is its answer, the
    // empty listing the single lookup reads from `Media: null`.
    let fake = FakeAniList {
        media: vec![media(11, Some(1))],
    };
    let server = serve(fake).await;
    let wants = vec![
        want(Some(1), None),
        want(Some(2), None),
        want(None, Some(30)),
    ];
    let got = listings(
        streaming_listings_batch(&reqwest::Client::new(), &wants, Some(&server.uri())).await,
    );
    assert_eq!(got[0], media(11, Some(1)).listing());
    assert_eq!(got[1], StreamingListing::default());
    assert_eq!(got[2], StreamingListing::default());
}

#[tokio::test]
async fn two_shows_with_one_mal_id_share_its_node() {
    // Kitsu maps two cours of one show to the same MAL entry: both read
    // the one node AniList lists for it.
    let fake = FakeAniList {
        media: vec![media(11, Some(1)), media(12, Some(2))],
    };
    let server = serve(fake.clone()).await;
    let wants = vec![
        want(Some(1), None),
        want(Some(1), Some(50)),
        want(Some(2), None),
    ];
    let got = listings(
        streaming_listings_batch(&reqwest::Client::new(), &wants, Some(&server.uri())).await,
    );
    assert_eq!(got[0], media(11, Some(1)).listing());
    assert_eq!(got[1], media(11, Some(1)).listing());
    assert_eq!(asked_of(&server).await, vec![Asked::PageByMal(vec![1, 2])]);
}

#[tokio::test]
async fn a_mal_id_several_media_record_is_settled_by_its_single_lookup() {
    // Two AniList media record MAL id 1: the page cannot say which one
    // the show is, so it is asked as the single lookup asks it, and
    // reads the media that answers.
    let fake = FakeAniList {
        media: vec![media(11, Some(1)), media(12, Some(1)), media(13, Some(2))],
    };
    let server = serve(fake.clone()).await;
    let wants = vec![want(Some(1), None), want(Some(2), None)];
    let got = listings(
        streaming_listings_batch(&reqwest::Client::new(), &wants, Some(&server.uri())).await,
    );
    assert_eq!(got[0], fake.single_path(Some(1), None));
    assert_eq!(got[1], media(13, Some(2)).listing());
    assert_eq!(
        asked_of(&server).await,
        vec![Asked::PageByMal(vec![1, 2]), Asked::ByMal(1)]
    );
}

#[tokio::test]
async fn a_mal_id_several_media_record_is_asked_once_for_every_show_naming_it() {
    // Two shows name MAL id 1, one with an AniList id beside it: the
    // single lookup by MAL id goes once, since AniList records media
    // for the id and no show falls back past it.
    let fake = FakeAniList {
        media: vec![media(11, Some(1)), media(12, Some(1))],
    };
    let server = serve(fake.clone()).await;
    let wants = vec![want(Some(1), None), want(Some(1), Some(50))];
    let got = listings(
        streaming_listings_batch(&reqwest::Client::new(), &wants, Some(&server.uri())).await,
    );
    assert_eq!(got[0], fake.single_path(Some(1), None));
    assert_eq!(got[1], fake.single_path(Some(1), Some(50)));
    assert_eq!(
        asked_of(&server).await,
        vec![Asked::PageByMal(vec![1]), Asked::ByMal(1)]
    );
}

#[tokio::test]
async fn more_shows_than_a_page_holds_go_in_pages_of_fifty() {
    let fake = FakeAniList {
        media: (1..=60).map(|i| media(1000 + i, Some(i))).collect(),
    };
    let server = serve(fake.clone()).await;
    let wants: Vec<Want> = (1..=60).map(|i| want(Some(i), None)).collect();
    let got = listings(
        streaming_listings_batch(&reqwest::Client::new(), &wants, Some(&server.uri())).await,
    );
    for (w, l) in wants.iter().zip(&got) {
        assert_eq!(*l, fake.single_path(w.mal, w.anilist));
    }
    assert_eq!(
        asked_of(&server).await,
        vec![
            Asked::PageByMal((1..=50).collect()),
            Asked::PageByMal((51..=60).collect())
        ]
    );
}

#[tokio::test]
async fn a_failed_page_leaves_its_shows_unanswered_and_the_rest_answered() {
    // The MAL page fails: its shows get the failure — not the empty
    // listing, and not a fallback by AniList id, since AniList said
    // nothing about them. The AniList-only show's page still goes out.
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::body_partial_json(
            serde_json::json!({ "variables": { "malIds": [1, 2] } }),
        ))
        .respond_with(wiremock::ResponseTemplate::new(500))
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(FakeAniList {
            media: vec![media(13, None)],
        })
        .mount(&server)
        .await;
    let wants = vec![
        want(Some(1), Some(11)),
        want(Some(2), None),
        want(None, Some(13)),
    ];
    let got = streaming_listings_batch(&reqwest::Client::new(), &wants, Some(&server.uri())).await;
    assert!(matches!(got[0], Err(AniError::Upstream { status: 500 })));
    assert!(matches!(got[1], Err(AniError::Upstream { status: 500 })));
    assert_eq!(
        got[2].as_ref().expect("answered"),
        &media(13, None).listing()
    );
    assert_eq!(
        asked_of(&server).await,
        vec![Asked::PageByMal(vec![1, 2]), Asked::PageById(vec![13])]
    );
}

#[tokio::test]
async fn a_rate_limited_page_answers_every_show_with_the_refusal() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(429).insert_header("retry-after", "60"))
        .mount(&server)
        .await;
    let wants = vec![want(Some(1), None), want(None, Some(13))];
    let got = streaming_listings_batch(&reqwest::Client::new(), &wants, Some(&server.uri())).await;
    for answer in got {
        assert!(
            matches!(
                answer,
                Err(AniError::RateLimited {
                    refused_by: Some("anilist"),
                    ..
                })
            ),
            "{answer:?}"
        );
    }
}

// --- The page parser ----------------------------------------------

#[test]
fn a_page_maps_each_node_by_the_key_it_was_asked_by() {
    let body = r#"{"data":{"Page":{"pageInfo":{"hasNextPage":false},"media":[
        {"idMal":1,"episodes":12,"status":"FINISHED","streamingEpisodes":[
            {"title":"Episode 1 - A","thumbnail":"https://cr.cdn/1.jpg"}]},
        {"idMal":2,"episodes":null,"status":"RELEASING","streamingEpisodes":[]},
        {"idMal":null,"episodes":1,"status":"FINISHED","streamingEpisodes":[]}
    ]}}}"#;
    let Page::Whole((once, several)) =
        parse_streaming_page(body.as_bytes(), Space::Mal).expect("parses")
    else {
        panic!("a whole page");
    };
    assert!(several.is_empty());
    assert_eq!(once.len(), 2, "a node without its key names no show");
    let first = &once[&1];
    assert_eq!(first.episodes, Some(12));
    assert!(first.finished);
    assert_eq!(
        first.entries[&1].thumbnail.as_deref(),
        Some("https://cr.cdn/1.jpg")
    );
    assert_eq!(
        once[&2],
        StreamingListing {
            episodes: None,
            finished: false,
            entries: std::collections::BTreeMap::new(),
        }
    );
}

#[test]
fn a_page_keyed_by_anilist_id_reads_the_id() {
    let body = r#"{"data":{"Page":{"pageInfo":{"hasNextPage":false},"media":[
        {"id":7,"episodes":1,"status":"FINISHED","streamingEpisodes":null}
    ]}}}"#;
    let Page::Whole((once, _)) =
        parse_streaming_page(body.as_bytes(), Space::AniList).expect("parses")
    else {
        panic!("a whole page");
    };
    assert_eq!(
        once[&7],
        StreamingListing {
            episodes: Some(1),
            finished: true,
            entries: std::collections::BTreeMap::new(),
        }
    );
}

#[test]
fn a_cut_page_says_so() {
    let body = r#"{"data":{"Page":{"pageInfo":{"hasNextPage":true},"media":[]}}}"#;
    assert_eq!(
        parse_streaming_page(body.as_bytes(), Space::Mal).expect("parses"),
        Page::Cut
    );
}

#[test]
fn a_key_two_nodes_answer_is_handed_over() {
    let node = r#"{"idMal":1,"episodes":1,"status":"FINISHED","streamingEpisodes":[]}"#;
    let body = format!(
        r#"{{"data":{{"Page":{{"pageInfo":{{"hasNextPage":false}},"media":[{node},{node}]}}}}}}"#
    );
    let Page::Whole((once, several)) =
        parse_streaming_page(body.as_bytes(), Space::Mal).expect("parses")
    else {
        panic!("a whole page");
    };
    assert!(once.is_empty());
    assert_eq!(several, vec![1]);
}

#[test]
fn a_node_that_cannot_be_read_is_handed_to_its_single_lookup() {
    // AniList may null an entry of one media's list: that show is asked
    // by its single lookup, which reads the failure for it alone, and
    // the rest of the page stands.
    let body = r#"{"data":{"Page":{"pageInfo":{"hasNextPage":false},"media":[
        {"idMal":1,"episodes":1,"status":"FINISHED","streamingEpisodes":[null]},
        {"idMal":2,"episodes":1,"status":"FINISHED","streamingEpisodes":[]},
        {"idMal":3,"episodes":"x","status":"FINISHED","streamingEpisodes":[]}
    ]}}}"#;
    let Page::Whole((once, several)) =
        parse_streaming_page(body.as_bytes(), Space::Mal).expect("parses")
    else {
        panic!("a whole page");
    };
    assert_eq!(once.keys().copied().collect::<Vec<_>>(), vec![2]);
    assert_eq!(several, vec![1, 3]);
}

#[test]
fn a_page_that_leaves_out_what_it_was_asked_for_is_no_answer() {
    for node in [
        r#"{"episodes":1,"status":"FINISHED","streamingEpisodes":[]}"#,
        r#"{"idMal":1,"status":"FINISHED","streamingEpisodes":[]}"#,
        r#"{"idMal":1,"episodes":1,"streamingEpisodes":[]}"#,
        r#"{"idMal":1,"episodes":1,"status":"FINISHED"}"#,
        r#"7"#,
    ] {
        let body = format!(
            r#"{{"data":{{"Page":{{"pageInfo":{{"hasNextPage":false}},"media":[{node}]}}}}}}"#
        );
        assert!(
            matches!(
                parse_streaming_page(body.as_bytes(), Space::Mal),
                Err(AniError::ParseFailed { .. })
            ),
            "{node}"
        );
    }
    for body in [
        r#"{"data":{"Page":{"media":[]}}}"#,
        r#"{"data":{"Page":{"pageInfo":{"hasNextPage":false}}}}"#,
        r#"{"data":{"Page":null}}"#,
        "not json",
    ] {
        assert!(
            matches!(
                parse_streaming_page(body.as_bytes(), Space::Mal),
                Err(AniError::ParseFailed { .. })
            ),
            "{body}"
        );
    }
}
