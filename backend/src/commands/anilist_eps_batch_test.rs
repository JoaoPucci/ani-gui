//! A Continue Watching strip filling its shows' thumbnails at once: the
//! lookups that miss the cache together share AniList requests — one per
//! id space — instead of one request per show, and each show still keeps
//! its own cached answer.

use super::*;
use crate::meta::anilist_streaming_eps_fake::{asked_of, Asked, FakeAniList, FakeMedia};

/// Kitsu answering `/anime/:id?include=mappings` for each show in
/// `shows`: (kitsu id, its sites).
struct KitsuMappings(Vec<(String, Vec<(&'static str, u32)>)>);

impl wiremock::Respond for KitsuMappings {
    fn respond(&self, request: &wiremock::Request) -> wiremock::ResponseTemplate {
        let id = request.url.path().trim_start_matches("/anime/");
        match self.0.iter().find(|(k, _)| k == id) {
            Some((k, sites)) => {
                wiremock::ResponseTemplate::new(200).set_body_string(mappings_body(k, sites))
            }
            None => wiremock::ResponseTemplate::new(404),
        }
    }
}

async fn kitsu_for(shows: Vec<(String, Vec<(&'static str, u32)>)>) -> wiremock::MockServer {
    let kitsu = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .respond_with(KitsuMappings(shows))
        .mount(&kitsu)
        .await;
    kitsu
}

async fn anilist_serving(fake: FakeAniList) -> wiremock::MockServer {
    let anilist = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(fake)
        .mount(&anilist)
        .await;
    anilist
}

/// Fill every show in `kitsu_ids` at once, as the strip's cards do.
async fn fill_all(state: &AppState, kitsu_ids: &[String], base: &str) -> Vec<StreamingListing> {
    futures_util::future::join_all(
        kitsu_ids
            .iter()
            .map(|id| thumbs_for_show_with_anilist_base(state, id, Some(base))),
    )
    .await
}

fn cached(state: &AppState, kitsu_id: &str) -> Option<StreamingListing> {
    meta_cache_get(&state.cache_pool, &anilist_eps_thumbs_key(kitsu_id))
        .expect("cache read")
        .map(|body| serde_json::from_str(&body).expect("a listing"))
}

#[tokio::test]
async fn a_strip_of_ten_shows_fills_with_one_anilist_request() {
    let media: Vec<FakeMedia> = (1..=10)
        .map(|i| FakeMedia {
            id: 100 + i,
            mal: Some(i),
            listed: 3,
        })
        .collect();
    let ids: Vec<String> = (1..=10).map(|i| format!("{}", 5000 + i)).collect();
    let kitsu = kitsu_for(
        (1..=10)
            .map(|i| {
                (
                    format!("{}", 5000 + i),
                    vec![("myanimelist/anime", i), ("anilist/anime", 100 + i)],
                )
            })
            .collect(),
    )
    .await;
    let anilist = anilist_serving(FakeAniList {
        media: media.clone(),
    })
    .await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let got = fill_all(&state, &ids, &anilist.uri()).await;
    for ((id, listing), m) in ids.iter().zip(&got).zip(&media) {
        assert_eq!(*listing, m.listing(), "kitsu {id}");
        assert_eq!(
            cached(&state, id).as_ref(),
            Some(&m.listing()),
            "kitsu {id}"
        );
    }
    assert_eq!(
        asked_of(&anilist).await,
        vec![Asked::PageByMal((1..=10).collect())]
    );
}

#[tokio::test]
async fn a_strip_of_mixed_mappings_fills_with_one_request_per_id_space() {
    // Shows by MAL id, shows Kitsu maps to AniList alone, and one whose
    // MAL id AniList does not record — read by its AniList id, as the
    // single lookup reads it — plus one AniList has nothing for, cached
    // as the empty listing AniList's answer is.
    let fake = FakeAniList {
        media: vec![
            FakeMedia {
                id: 11,
                mal: Some(1),
                listed: 2,
            },
            FakeMedia {
                id: 12,
                mal: Some(2),
                listed: 2,
            },
            FakeMedia {
                id: 13,
                mal: None,
                listed: 2,
            },
            FakeMedia {
                id: 14,
                mal: None,
                listed: 2,
            },
            FakeMedia {
                id: 15,
                mal: Some(77),
                listed: 2,
            },
        ],
    };
    let shows: Vec<(String, Vec<(&'static str, u32)>)> = vec![
        ("1".into(), vec![("myanimelist/anime", 1)]),
        (
            "2".into(),
            vec![("myanimelist/anime", 2), ("anilist/anime", 12)],
        ),
        ("3".into(), vec![("anilist/anime", 13)]),
        ("4".into(), vec![("anilist/anime", 14)]),
        (
            "5".into(),
            vec![("myanimelist/anime", 5), ("anilist/anime", 15)],
        ),
        ("6".into(), vec![("myanimelist/anime", 6)]),
    ];
    let ids: Vec<String> = shows.iter().map(|(k, _)| k.clone()).collect();
    let kitsu = kitsu_for(shows).await;
    let anilist = anilist_serving(fake.clone()).await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let got = fill_all(&state, &ids, &anilist.uri()).await;
    let want = [
        fake.media[0].listing(),
        fake.media[1].listing(),
        fake.media[2].listing(),
        fake.media[3].listing(),
        fake.media[4].listing(),
        StreamingListing::default(),
    ];
    assert_eq!(got, want);
    for (id, listing) in ids.iter().zip(&want) {
        assert_eq!(cached(&state, id).as_ref(), Some(listing), "kitsu {id}");
    }
    let mut asked = asked_of(&anilist).await;
    assert_eq!(asked.len(), 2, "{asked:?}");
    for a in &mut asked {
        if let Asked::PageByMal(v) | Asked::PageById(v) = a {
            v.sort_unstable();
        }
    }
    assert_eq!(
        asked,
        vec![
            Asked::PageByMal(vec![1, 2, 5, 6]),
            Asked::PageById(vec![13, 14, 15])
        ]
    );
}

#[tokio::test]
async fn a_strip_refused_by_anilist_caches_none_of_its_shows() {
    // A refusal says nothing about any show it covered: nothing is
    // cached, so each is asked again, and the next view reads no
    // "no stills" row.
    let shows: Vec<(String, Vec<(&'static str, u32)>)> = (1..=4)
        .map(|i| (format!("{i}"), vec![("myanimelist/anime", i)]))
        .collect();
    let ids: Vec<String> = shows.iter().map(|(k, _)| k.clone()).collect();
    let kitsu = kitsu_for(shows).await;
    let anilist = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(429).insert_header("retry-after", "60"))
        .mount(&anilist)
        .await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let got = fill_all(&state, &ids, &anilist.uri()).await;
    for (id, listing) in ids.iter().zip(&got) {
        assert_eq!(*listing, StreamingListing::default());
        assert_eq!(cached(&state, id), None, "kitsu {id} was cached");
    }
    let asked = asked_of(&anilist).await;
    assert!(
        asked.iter().all(|a| matches!(a, Asked::PageByMal(_))),
        "{asked:?}"
    );
    assert!(!asked.is_empty() && asked.len() <= 2, "{asked:?}");
}

#[tokio::test]
async fn a_strip_whose_page_fails_keeps_the_other_space_s_answers() {
    let shows: Vec<(String, Vec<(&'static str, u32)>)> = vec![
        ("1".into(), vec![("myanimelist/anime", 1)]),
        ("2".into(), vec![("myanimelist/anime", 2)]),
        ("3".into(), vec![("anilist/anime", 13)]),
    ];
    let ids: Vec<String> = shows.iter().map(|(k, _)| k.clone()).collect();
    let kitsu = kitsu_for(shows).await;
    let anilist = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::body_string_contains("malIds"))
        .respond_with(wiremock::ResponseTemplate::new(502))
        .mount(&anilist)
        .await;
    let media = FakeMedia {
        id: 13,
        mal: None,
        listed: 2,
    };
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(FakeAniList {
            media: vec![media.clone()],
        })
        .mount(&anilist)
        .await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let got = fill_all(&state, &ids, &anilist.uri()).await;
    assert_eq!(got[0], StreamingListing::default());
    assert_eq!(got[1], StreamingListing::default());
    assert_eq!(got[2], media.listing());
    assert_eq!(cached(&state, "1"), None);
    assert_eq!(cached(&state, "2"), None);
    assert_eq!(cached(&state, "3"), Some(media.listing()));
}

#[tokio::test]
async fn a_lone_show_keeps_its_single_lookup() {
    // The episode page filling one show: the query it always sent.
    let kitsu = kitsu_for(vec![("9".into(), vec![("myanimelist/anime", 9)])]).await;
    let media = FakeMedia {
        id: 90,
        mal: Some(9),
        listed: 1,
    };
    let anilist = anilist_serving(FakeAniList {
        media: vec![media.clone()],
    })
    .await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let got = thumbs_for_show_with_anilist_base(&state, "9", Some(&anilist.uri())).await;
    assert_eq!(got, media.listing());
    assert_eq!(asked_of(&anilist).await, vec![Asked::ByMal(9)]);
}

#[tokio::test]
async fn a_card_that_stopped_waiting_is_left_out_of_the_page() {
    // The episode route gives the fill a budget and drops it past that.
    // A lookup dropped before its page went out is not asked for.
    let kitsu = kitsu_for(vec![
        ("1".into(), vec![("myanimelist/anime", 1)]),
        ("2".into(), vec![("myanimelist/anime", 2)]),
        ("3".into(), vec![("myanimelist/anime", 3)]),
    ])
    .await;
    let fake = FakeAniList {
        media: (1..=3)
            .map(|i| FakeMedia {
                id: 10 + i,
                mal: Some(i),
                listed: 1,
            })
            .collect(),
    };
    let anilist = anilist_serving(fake.clone()).await;
    let state = state_with_kitsu_at(&kitsu.uri());
    let base = anilist.uri();
    // Show 3's mappings are read first, so its lookup is queued when it
    // is dropped.
    let _ = crate::commands::external_ids::external_ids_for(&state, "3").await;
    let dropped = tokio::time::timeout(
        std::time::Duration::from_millis(1),
        thumbs_for_show_with_anilist_base(&state, "3", Some(&base)),
    );
    let (_, a, b) = tokio::join!(
        dropped,
        thumbs_for_show_with_anilist_base(&state, "1", Some(&base)),
        thumbs_for_show_with_anilist_base(&state, "2", Some(&base)),
    );
    assert_eq!(a, fake.media[0].listing());
    assert_eq!(b, fake.media[1].listing());
    assert_eq!(asked_of(&anilist).await, vec![Asked::PageByMal(vec![1, 2])]);
}
