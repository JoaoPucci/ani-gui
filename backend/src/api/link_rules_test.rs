//! The change-show flow's routes answer with the backend's rules.

use super::super::build_api_router;
use super::super::tests::{body_string, test_app_state};
use axum::body::Body;
use axum::http::{Request, StatusCode};
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;

async fn post(uri: &str, body: serde_json::Value) -> (StatusCode, serde_json::Value) {
    let td = TempDir::new().expect("tempdir");
    let router = build_api_router(Arc::new(test_app_state(&td)));
    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .expect("req"),
        )
        .await
        .expect("oneshot");
    let status = response.status();
    let text = body_string(response).await;
    (
        status,
        serde_json::from_str(&text).unwrap_or(serde_json::Value::Null),
    )
}

#[tokio::test]
async fn the_card_lines_route_answers_the_line_each_card_stands_for() {
    let (status, body) = post(
        "/api/history/card-lines",
        serde_json::json!({ "rows": [
            { "id": "hianime:jojos-bizarre-adventure-2012-708",
              "guessed_from": "hianime:jojos-bizarre-adventure-2000-5182",
              "show": "7158", "guess": true },
            { "id": "hianime:jojos-bizarre-adventure-2000-5182", "show": "7158", "guess": true }
        ] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["hianime:jojos-bizarre-adventure-2012-708"],
        serde_json::json!({ "line": "hianime:jojos-bizarre-adventure-2000-5182", "correctable": true })
    );
}

#[tokio::test]
async fn the_verdict_route_answers_whether_a_surface_asks() {
    let (status, body) = post(
        "/api/history/link-verdict",
        serde_json::json!({ "page": "7158", "show": "7158", "trusted": false, "asks_guess": true }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, serde_json::json!({ "offer": "guess", "kept": false }));
}

#[tokio::test]
async fn the_candidates_route_leads_with_the_card_show_and_its_warnings() {
    // Kitsu is unreachable from the test state: both of the picker's own
    // searches fail quietly, and the card's show still leads.
    let (status, body) = post(
        "/api/history/link-candidates",
        serde_json::json!({
            "row": "vDTSJHSpYnrkZnAvG", "title": "Gintama Season 4", "episode": 3,
            "current": { "id": "1", "canonical_title": "Gintama", "slug": "gintama",
                "synopsis": null, "start_date": null, "end_date": null, "episode_count": 1,
                "average_rating": null, "subtype": "TV", "status": "finished",
                "age_rating": null, "popularity_rank": null, "poster_image": null,
                "cover_image": null }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body[0]["ref"]["id"], "1");
    assert_eq!(body[0]["current"], true);
    assert_eq!(
        body[0]["warnings"],
        serde_json::json!(["season", "episode"])
    );
}
