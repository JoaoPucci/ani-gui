//! The route a Continue row's guess asks which of its hits the row
//! rules out by.

use super::tests::{body_string, test_app_state};
use super::*;
use axum::body::Body;
use axum::http::Request;
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;

async fn post(body: serde_json::Value) -> (StatusCode, String) {
    let td = TempDir::new().expect("tempdir");
    let router = build_api_router(Arc::new(test_app_state(&td)));
    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/history/guess/ruled-out")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .expect("req"),
        )
        .await
        .expect("oneshot");
    let status = response.status();
    (status, body_string(response).await)
}

#[tokio::test]
async fn the_route_answers_the_ids_the_row_rules_out() {
    // hianime's "Jujutsu Kaisen (TV)" at its last episode, against the
    // two hits Kitsu's search returned for it, written as the renderer
    // holds them: the special is out, the music video is never a
    // series either.
    let (status, body) = post(serde_json::json!({
        "title": "Jujutsu Kaisen (TV)",
        "episode": 24,
        "hits": [
            {
                "id": "43886",
                "canonical_title": "Jujutsu Kaisen New Year's Special",
                "titles": { "en": "Jujutsu Kaisen New Year's Special" },
                "abbreviated_titles": [],
                "slug": "jujutsu-kaisen-new-year-s-special",
                "episode_count": 1,
                "subtype": "special"
            },
            {
                "id": "42765",
                "canonical_title": "Jujutsu Kaisen",
                "titles": { "en": "Jujutsu Kaisen" },
                "abbreviated_titles": [],
                "slug": "jujutsu-kaisen",
                "episode_count": 24,
                "subtype": "TV"
            }
        ]
    }))
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let ids: Vec<String> = serde_json::from_str(&body).expect("ids");
    assert_eq!(ids, vec!["43886"]);
}

#[tokio::test]
async fn a_hit_missing_its_optional_fields_is_read_as_carrying_none() {
    // A hit from a cached answer written before Kitsu's titles were
    // kept carries no `titles` or `abbreviated_titles`; it is judged on
    // what it carries rather than refused as malformed.
    let (status, body) = post(serde_json::json!({
        "title": "Black Clover",
        "episode": 100,
        "hits": [
            { "id": "12625", "canonical_title": "Black Clover: Jump Festa 2016 Special", "episode_count": 1 },
            { "id": "13209", "canonical_title": "Black Clover" }
        ]
    }))
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let ids: Vec<String> = serde_json::from_str(&body).expect("ids");
    assert_eq!(ids, vec!["12625"]);
}
