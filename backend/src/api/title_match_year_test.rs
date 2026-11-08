//! Title matches keyed by the year the row names, through the routes.

use super::tests::{body_string, test_app_state};
use super::*;
use axum::body::Body;
use axum::http::Request;
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;

const TITLE: &str = "JoJo's Bizarre Adventure";

fn row(id: &str) -> HistoryEntry {
    HistoryEntry {
        ep_no: "1".into(),
        id: id.into(),
        title: TITLE.into(),
        watched_at: None,
        kitsu_id: None,
        own_number: true,
    }
}

fn state(td: &TempDir) -> AppState {
    let state = test_app_state(td);
    crate::history::write_atomic(
        &state.history_path,
        &[
            row("hianime:jojos-bizarre-adventure-2000-5182"),
            row("hianime:jojos-bizarre-adventure-2012-9"),
        ],
    )
    .expect("history");
    state
}

async fn put(router: &Router, year: u32, kitsu_id: &str) {
    let body = serde_json::json!({
        "title": TITLE, "cour": 1, "kitsu_id": kitsu_id, "provider": "hianime", "year": year,
    });
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/title-match")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .expect("req"),
        )
        .await
        .expect("oneshot");
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

async fn get(router: &Router, year: Option<u32>) -> serde_json::Value {
    let mut uri =
        "/api/title-match?title=JoJo%27s+Bizarre+Adventure&cour=1&provider=hianime".to_string();
    if let Some(y) = year {
        uri.push_str(&format!("&year={y}"));
    }
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(uri)
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("oneshot");
    assert_eq!(response.status(), StatusCode::OK);
    serde_json::from_str(&body_string(response).await).expect("json")
}

#[tokio::test]
async fn rows_titled_alike_keep_the_guess_their_year_named() {
    let td = TempDir::new().expect("tempdir");
    let router = build_api_router(Arc::new(state(&td)));
    put(&router, 2000, "7730").await;
    put(&router, 2012, "7158").await;
    assert_eq!(get(&router, Some(2000)).await, "7730");
    assert_eq!(get(&router, Some(2012)).await, "7158");
    assert_eq!(
        get(&router, None).await,
        serde_json::Value::Null,
        "a row naming no year reads neither"
    );
}

#[tokio::test]
async fn removing_a_row_takes_the_guess_its_year_keyed() {
    let td = TempDir::new().expect("tempdir");
    let state = Arc::new(state(&td));
    let router = build_api_router(state.clone());
    put(&router, 2000, "7730").await;
    put(&router, 2012, "7158").await;
    assert!(crate::commands::history::history_delete(
        &state,
        "hianime:jojos-bizarre-adventure-2000-5182"
    )
    .expect("delete"));
    assert_eq!(get(&router, Some(2000)).await, serde_json::Value::Null);
    assert_eq!(
        get(&router, Some(2012)).await,
        "7158",
        "the other row's guess stays"
    );
}
