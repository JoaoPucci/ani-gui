//! `GET /api/external-player/standard?kind=` tells Settings what a
//! blank External player command runs for a type, and where that
//! program is installed on this computer.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tempfile::TempDir;
use tower::ServiceExt;

use super::build_api_router;
use super::tests::{body_string, test_app_state};

async fn ask(kind: &str) -> (StatusCode, String) {
    let td = TempDir::new().expect("tempdir");
    let router = build_api_router(Arc::new(test_app_state(&td)));
    let response = router
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/external-player/standard?kind={kind}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("oneshot");
    let status = response.status();
    (status, body_string(response).await)
}

#[tokio::test]
async fn custom_has_no_standard_program() {
    let (status, body) = ask("custom").await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(v, serde_json::json!({ "name": null, "path": null }));
}

#[tokio::test]
async fn a_standard_type_names_its_program_and_where_it_was_found() {
    let (status, body) = ask("vlc").await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_str(&body).expect("json");
    assert_eq!(v["name"], "vlc");
    // Found or not depends on the host; either way it is what the
    // search finds here.
    let here = crate::commands::player_locate::locate_installed("vlc");
    assert_eq!(v["path"], serde_json::json!(here));
}

#[tokio::test]
async fn an_unknown_type_is_refused() {
    let (status, _) = ask("winamp").await;
    assert!(status.is_client_error(), "{status}");
}
