//! Two reads of one anime's detail at the same time share one request.
//!
//! On a first visit the show page reads the detail, and the franchise
//! walk reads the same detail for the entry it was asked about. Before
//! either answer was cached, each sent Kitsu its own request: the
//! listing reads were shared in flight (`franchise::franchise_listing`)
//! and the detail reads were not.

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::tests::state_with_kitsu_at;
use super::*;

const DETAIL: &[u8] = include_bytes!("../../../tests/fixtures/kitsu/anime_one_piece_detail.json");

/// Kitsu answering `/anime/12` slowly, so two reads overlap, and
/// expecting one request.
async fn slow_detail(answer: ResponseTemplate) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime/12"))
        .respond_with(answer.set_delay(std::time::Duration::from_millis(200)))
        .expect(1)
        .mount(&server)
        .await;
    server
}

#[tokio::test]
async fn the_show_page_and_the_walk_reading_one_detail_at_once_make_one_request() {
    let server = slow_detail(
        ResponseTemplate::new(200)
            .insert_header("content-type", "application/vnd.api+json")
            .set_body_bytes(DETAIL.to_vec()),
    )
    .await;
    let state = state_with_kitsu_at(&server.uri());
    let (page, walk) = tokio::join!(
        kitsu_anime_detail_page(&state, "12"),
        kitsu_anime_detail(&state, "12")
    );
    let (page, walk) = (page.expect("page read"), walk.expect("walk read"));
    assert_eq!(page.id, "12");
    assert_eq!(page.canonical_title, walk.canonical_title);
    server.verify().await;
}

/// A failure is shared as well: the second read does not ask again
/// while the first is still waiting on the same answer.
#[tokio::test]
async fn a_failed_read_is_shared_with_the_read_waiting_on_it() {
    let server = slow_detail(ResponseTemplate::new(503)).await;
    let state = state_with_kitsu_at(&server.uri());
    let (a, b) = tokio::join!(
        kitsu_anime_detail(&state, "12"),
        kitsu_anime_detail(&state, "12")
    );
    assert!(a.is_err() && b.is_err(), "both reads report the failure");
    server.verify().await;
}

/// Reads of two different details are not held to one request.
#[tokio::test]
async fn reads_of_two_details_each_ask_kitsu() {
    let server = MockServer::start().await;
    for id in ["12", "13"] {
        Mock::given(method("GET"))
            .and(path(format!("/anime/{id}")))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/vnd.api+json")
                    .set_body_bytes(
                        String::from_utf8_lossy(DETAIL)
                            .replace("\"12\"", &format!("\"{id}\""))
                            .into_bytes(),
                    ),
            )
            .expect(1)
            .mount(&server)
            .await;
    }
    let state = state_with_kitsu_at(&server.uri());
    let (a, b) = tokio::join!(
        kitsu_anime_detail(&state, "12"),
        kitsu_anime_detail(&state, "13")
    );
    assert_eq!(a.expect("12").id, "12");
    assert_eq!(b.expect("13").id, "13");
    server.verify().await;
}
