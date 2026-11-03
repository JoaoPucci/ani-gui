//! `GET /api/flags` hands the renderer the resolved flags and whether
//! the launch refresh has settled them, without waiting for it.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tempfile::TempDir;
use tower::ServiceExt;

use super::build_api_router;
use super::tests::{body_string, test_app_state};
use crate::app::AppState;
use crate::flags::store::{Origin, SETTLE_LIMIT};
use crate::flags::test_decls::TEST_DECLS;
use crate::flags::FlagStore;

async fn get_flags(state: AppState) -> (StatusCode, serde_json::Value) {
    let router = build_api_router(Arc::new(state));
    let response = router
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/flags")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("oneshot");
    let status = response.status();
    let body = body_string(response).await;
    (
        status,
        serde_json::from_str(&body).unwrap_or(serde_json::Value::Null),
    )
}

#[tokio::test]
async fn the_route_returns_the_decided_flags_and_their_origin() {
    let td = TempDir::new().expect("tempdir");
    let state = AppState {
        flags: FlagStore::fixed(TEST_DECLS),
        ..test_app_state(&td)
    };
    let (status, body) = get_flags(state).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        serde_json::json!({
            "origin": "defaults",
            "flags": { "test_kill": true, "test_fallback": "new" }
        })
    );
}

#[tokio::test(start_paused = true)]
async fn the_route_waits_for_the_decision() {
    let td = TempDir::new().expect("tempdir");
    let store = FlagStore::pending(TEST_DECLS);
    let state = AppState {
        flags: store.clone(),
        ..test_app_state(&td)
    };
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;
        let doc = crate::flags::resolve::parse_document(br#"{"flags":{"test_kill":false}}"#);
        store.settle(doc.as_ref(), Origin::File);
    });
    let (status, body) = get_flags(state).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["origin"], serde_json::json!("file"));
    assert_eq!(body["flags"]["test_kill"], serde_json::json!(false));
}

#[tokio::test(start_paused = true)]
async fn the_route_never_waits_past_the_stores_limit() {
    let td = TempDir::new().expect("tempdir");
    let state = AppState {
        flags: FlagStore::pending(TEST_DECLS),
        ..test_app_state(&td)
    };
    let started = tokio::time::Instant::now();
    let (status, body) = get_flags(state).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["origin"], serde_json::json!("defaults"));
    assert!(started.elapsed() <= SETTLE_LIMIT + std::time::Duration::from_millis(100));
}

/// The shipped build declares the notification center, a rollout,
/// which the window reads as off until a signed file turns it on.
#[tokio::test]
async fn the_shipped_build_hands_the_window_the_center_off() {
    let td = TempDir::new().expect("tempdir");
    let (status, body) = get_flags(test_app_state(&td)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["flags"],
        serde_json::json!({ "notification_center": false })
    );
}
