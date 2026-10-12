//! `/api/kitsu/search/page`, driven through the router.

use super::tests::{body_string, test_app_state};
use super::*;
use crate::meta::kitsu::KitsuClient;
use axum::body::Body;
use axum::http::Request;
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn post_page(state: AppState, body: &str) -> axum::response::Response {
    build_api_router(Arc::new(state))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/kitsu/search/page")
                .header("content-type", "application/json")
                .body(Body::from(body.to_owned()))
                .expect("req"),
        )
        .await
        .expect("oneshot")
}

#[tokio::test]
async fn a_page_answers_its_hits_and_where_the_next_one_starts() {
    let mock = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/anime"))
        .and(query_param("filter[text]", "dragon ball"))
        .and(query_param("page[offset]", "60"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/vnd.api+json")
                .set_body_json(serde_json::json!({
                    "data": [{
                        "id": "68",
                        "type": "anime",
                        "attributes": { "canonicalTitle": "Dragon Ball Super: Beerus" }
                    }],
                    "links": { "next": "https://kitsu.io/api/edge/anime?page%5Boffset%5D=80" },
                    "meta": { "count": 120 }
                })),
        )
        .expect(1)
        .mount(&mock)
        .await;

    let td = TempDir::new().expect("tempdir");
    let mut state = test_app_state(&td);
    state.kitsu = KitsuClient::with_base(reqwest::Client::new(), mock.uri());
    let response = post_page(state, r#"{"query":"dragon ball","offset":60}"#).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value = serde_json::from_str(&body_string(response).await).expect("json");
    assert_eq!(body["hits"][0]["id"], "68");
    assert_eq!(body["next_offset"], 80);
}

#[tokio::test]
async fn an_offset_off_a_page_boundary_is_a_bad_request_with_its_key() {
    let td = TempDir::new().expect("tempdir");
    let response = post_page(test_app_state(&td), r#"{"query":"x","offset":7}"#).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_string(response).await;
    assert!(body.contains("invalid_search_offset"), "{body}");
}

#[tokio::test]
async fn a_negative_or_missing_offset_is_refused() {
    for body in [r#"{"query":"x","offset":-20}"#, r#"{"query":"x"}"#] {
        let td = TempDir::new().expect("tempdir");
        let response = post_page(test_app_state(&td), body).await;
        assert!(
            response.status().is_client_error(),
            "{body}: {}",
            response.status()
        );
    }
}
