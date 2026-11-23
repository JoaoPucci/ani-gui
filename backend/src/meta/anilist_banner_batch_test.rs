//! The batched banner lookup against a stand-in AniList: a strip's
//! banners cost one request per id space, and each show reads the
//! banner its own single lookup would have.

use super::*;
use crate::error::AniError;
use crate::meta::anilist_batch::Space;
use crate::meta::anilist_pages::Page;
use crate::meta::anilist_streaming_eps_fake_test::{asked_of, Asked, FakeAniList, FakeMedia};

fn media(id: u32, mal: Option<u32>) -> FakeMedia {
    FakeMedia { id, mal, listed: 0 }
}

async fn serve(fake: FakeAniList) -> wiremock::MockServer {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(fake)
        .mount(&server)
        .await;
    server
}

#[tokio::test]
async fn ten_shows_banners_cost_one_request() {
    let fake = FakeAniList {
        media: (1..=10).map(|i| media(100 + i, Some(i))).collect(),
    };
    let server = serve(fake.clone()).await;
    let wants: Vec<Want> = (1..=10)
        .map(|i| Want {
            mal: Some(i),
            anilist: Some(100 + i),
        })
        .collect();
    let got = banners_batch(&reqwest::Client::new(), &wants, Some(&server.uri())).await;
    for (w, b) in wants.iter().zip(got) {
        assert_eq!(b.expect("answered"), fake.single_banner(w.mal, w.anilist));
    }
    assert_eq!(
        asked_of(&server).await,
        vec![Asked::PageByMal((1..=10).collect())]
    );
}

#[tokio::test]
async fn mixed_mappings_banners_cost_one_request_per_id_space() {
    let fake = FakeAniList {
        media: vec![media(11, Some(1)), media(13, None), media(15, Some(99))],
    };
    let server = serve(fake.clone()).await;
    let wants = vec![
        Want {
            mal: Some(1),
            anilist: None,
        },
        Want {
            mal: None,
            anilist: Some(13),
        },
        Want {
            mal: Some(5),
            anilist: Some(15),
        },
        Want {
            mal: Some(6),
            anilist: None,
        },
    ];
    let got = banners_batch(&reqwest::Client::new(), &wants, Some(&server.uri())).await;
    for (w, b) in wants.iter().zip(got) {
        assert_eq!(
            b.expect("answered"),
            fake.single_banner(w.mal, w.anilist),
            "{w:?}"
        );
    }
    assert_eq!(
        asked_of(&server).await,
        vec![
            Asked::PageByMal(vec![1, 5, 6]),
            Asked::PageById(vec![13, 15])
        ]
    );
}

#[test]
fn a_banner_page_reads_each_node_s_banner() {
    let body = r#"{"data":{"Page":{"pageInfo":{"hasNextPage":false},"media":[
        {"idMal":1,"bannerImage":"https://al.cdn/1.jpg"},
        {"idMal":2,"bannerImage":null},
        {"idMal":3,"bannerImage":7}
    ]}}}"#;
    let Page::Whole((once, several)) =
        parse_banner_page(body.as_bytes(), Space::Mal).expect("parses")
    else {
        panic!("a whole page");
    };
    assert_eq!(once[&1].as_deref(), Some("https://al.cdn/1.jpg"));
    assert_eq!(once[&2], None, "a media without a banner");
    assert_eq!(
        several,
        vec![3],
        "a banner that is not a string is asked singly"
    );
}

#[test]
fn a_banner_page_without_the_banner_field_is_no_answer() {
    let body = r#"{"data":{"Page":{"pageInfo":{"hasNextPage":false},"media":[{"idMal":1}]}}}"#;
    assert!(matches!(
        parse_banner_page(body.as_bytes(), Space::Mal),
        Err(AniError::ParseFailed { .. })
    ));
}
