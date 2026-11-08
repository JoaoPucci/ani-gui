//! The history route names each row's key year beside the row.

use super::tests::{body_string, test_app_state};
use super::*;
use axum::body::Body;
use axum::http::Request;
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;

fn row(id: &str, title: &str) -> HistoryEntry {
    HistoryEntry {
        ep_no: "1".into(),
        id: id.into(),
        title: title.into(),
        watched_at: None,
        kitsu_id: None,
        own_number: true,
    }
}

#[tokio::test]
async fn the_history_names_the_year_a_rows_key_carries() {
    let td = TempDir::new().expect("tempdir");
    let state = test_app_state(&td);
    crate::history::write_atomic(
        &state.history_path,
        &[
            row(
                "hianime:jojos-bizarre-adventure-2000-5182",
                "JoJo's Bizarre Adventure",
            ),
            row("hianime:black-clover-949", "Black Clover"),
        ],
    )
    .expect("history");
    let router = build_api_router(Arc::new(state));
    let response = router
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/history")
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("oneshot");
    assert_eq!(response.status(), StatusCode::OK);
    let rows: serde_json::Value = serde_json::from_str(&body_string(response).await).expect("json");
    let by_id = |id: &str| {
        rows.as_array()
            .expect("array")
            .iter()
            .find(|r| r["id"] == id)
            .cloned()
            .expect("row")
    };
    let jojo = by_id("hianime:jojos-bizarre-adventure-2000-5182");
    assert_eq!(jojo["key_year"], 2000);
    assert_eq!(jojo["title"], "JoJo's Bizarre Adventure");
    let clover = by_id("hianime:black-clover-949");
    assert!(
        clover.get("key_year").is_none(),
        "a key with no year names none"
    );
}
