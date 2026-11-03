//! `GET /api/source-health` reports what each provider's gate saw.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tempfile::TempDir;
use tower::ServiceExt;

use super::build_api_router;
use super::tests::{body_string, test_app_state};
use crate::scraper::gate::{ScrapeOutcome, ScrapePriority};
use crate::scraper::provider::ProviderId;

async fn get(state: crate::app::AppState) -> (StatusCode, serde_json::Value) {
    let router = build_api_router(Arc::new(state));
    let response = router
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/source-health")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("oneshot");
    let status = response.status();
    let body = body_string(response).await;
    (status, serde_json::from_str(&body).unwrap_or_default())
}

#[tokio::test]
async fn a_quiet_provider_reads_as_answering() {
    let td = TempDir::new().expect("tempdir");
    let (status, body) = get(test_app_state(&td)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body,
        serde_json::json!({ "providers": [{ "provider": "testa", "state": "answering" }] })
    );
}

#[tokio::test]
async fn a_provider_a_user_saw_fail_again_and_again_reads_as_not_answering() {
    let td = TempDir::new().expect("tempdir");
    let state = test_app_state(&td);
    let gate = state.providers.gate(ProviderId::TestA);
    for _ in 0..4 {
        gate.record_from(
            ScrapePriority::Interactive,
            ScrapeOutcome::Failure,
            tokio::time::Instant::now(),
        );
    }
    let (_, body) = get(state).await;
    let entry = &body["providers"][0];
    assert_eq!(entry["provider"], "testa");
    assert_eq!(entry["state"], "not_answering");
    assert_eq!(entry["failures"], 4);
    assert!(entry["since_ms"].as_u64().is_some_and(|t| t > 0));
}
