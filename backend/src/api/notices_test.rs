//! `GET /api/notices` hands the window the published notices of a
//! verified flags file, while the notification center is on.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;
use tempfile::TempDir;
use tower::ServiceExt;

use super::build_api_router;
use super::tests::{body_string, test_app_state};
use crate::app::AppState;
use crate::flags::resolve::parse_document;
use crate::flags::store::Origin;
use crate::flags::{FlagStore, DECLARED};

async fn get(state: AppState) -> (StatusCode, serde_json::Value) {
    let router = build_api_router(Arc::new(state));
    let response = router
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/notices")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("oneshot");
    let status = response.status();
    let body = body_string(response).await;
    (status, serde_json::from_str(&body).unwrap_or_default())
}

fn store(center: bool) -> FlagStore {
    let doc = parse_document(
        json!({ "flags": {
            "notification_center": center,
            "notices": { "format": 1, "donation_ask": 2, "items": [
                { "id": "maint", "title": "Maintenance tonight", "body": "Back by 22:00.",
                  "link": "https://github.com/JoaoPucci/ani-gui", "priority": "warning" }
            ]}
        }})
        .to_string()
        .as_bytes(),
    )
    .expect("doc");
    let store = FlagStore::pending(DECLARED);
    store.settle(Some(&doc), Origin::Test);
    store
}

#[tokio::test]
async fn the_route_serves_the_notices_while_the_center_is_on() {
    let td = TempDir::new().expect("tempdir");
    let state = AppState {
        flags: store(true),
        ..test_app_state(&td)
    };
    let (status, body) = get(state).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        json!({
            "donation_ask": 2,
            "items": [{
                "id": "maint",
                "title": "Maintenance tonight",
                "body": "Back by 22:00.",
                "link": "https://github.com/JoaoPucci/ani-gui",
                "priority": "warning",
                "about_source": false
            }]
        })
    );
}

#[tokio::test]
async fn the_route_serves_nothing_while_the_center_is_off() {
    let td = TempDir::new().expect("tempdir");
    let state = AppState {
        flags: store(false),
        ..test_app_state(&td)
    };
    let (status, body) = get(state).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({ "donation_ask": null, "items": [] }));
}
