//! The reverse-mapping routes, driven through the router: what the
//! played read answers, and what an eviction may remove.

use super::tests::{body_string, test_app_state};
use super::*;
use axum::body::Body;
use axum::http::Request;
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;

async fn played_body(state: AppState, show_id: &str) -> String {
    let router = build_api_router(Arc::new(state));
    let response = router
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/allmanga-kitsu-map/{show_id}/played"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("oneshot");
    assert_eq!(response.status(), StatusCode::OK);
    body_string(response).await.trim().to_owned()
}

fn now_ms() -> i64 {
    i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_millis(),
    )
    .expect("ms")
}

/// A mapping a play stored reads as the Kitsu id the play stored.
/// Continue Watching keeps such a mapping when only the provider's
/// title doubts it, and only when the id is the one it read: the
/// mapping can change between its read and this one.
#[tokio::test]
async fn a_mapping_a_play_stored_reads_as_the_id_it_played() {
    let td = TempDir::new().expect("tempdir");
    let state = test_app_state(&td);
    crate::commands::kitsu::allmanga_kitsu_put_played(&state, "hianime:x-1", "49877").expect("put");
    assert_eq!(played_body(state, "hianime:x-1").await, "\"49877\"");
}

/// The id names the mapping standing now. A later play's mapping
/// reads as that play's id; a guess written over a played mapping
/// takes the mark, and reads as no play.
#[tokio::test]
async fn the_played_id_is_the_mapping_standing_now() {
    let td = TempDir::new().expect("tempdir");
    let replayed = test_app_state(&td);
    crate::commands::kitsu::allmanga_kitsu_put_played(&replayed, "hianime:x-1", "49877")
        .expect("put");
    crate::commands::kitsu::allmanga_kitsu_put_played(&replayed, "hianime:x-1", "1623")
        .expect("put");
    assert_eq!(
        played_body(replayed, "hianime:x-1").await,
        "\"1623\"",
        "played again"
    );

    let td = TempDir::new().expect("tempdir");
    let guessed = test_app_state(&td);
    crate::commands::kitsu::allmanga_kitsu_put_played(&guessed, "hianime:x-1", "49877")
        .expect("put");
    crate::commands::kitsu::allmanga_kitsu_put(&guessed, "hianime:x-1", "1623").expect("put");
    assert_eq!(
        played_body(guessed, "hianime:x-1").await,
        "null",
        "guessed over"
    );
}

/// A mapping no play stored is not played, however near the show's
/// watch stamp it was written.
#[tokio::test]
async fn a_mapping_no_watch_stored_reads_as_not_played() {
    let td = TempDir::new().expect("tempdir");
    let unstamped = test_app_state(&td);
    crate::commands::kitsu::allmanga_kitsu_put(&unstamped, "hianime:x-1", "1623").expect("put");
    assert_eq!(
        played_body(unstamped, "hianime:x-1").await,
        "null",
        "no stamp"
    );

    let td = TempDir::new().expect("tempdir");
    let apart = test_app_state(&td);
    crate::commands::kitsu::watched_at_put(&apart, "hianime:x-1", now_ms() - 3_600_000)
        .expect("stamp");
    crate::commands::kitsu::allmanga_kitsu_put(&apart, "hianime:x-1", "1623").expect("put");
    assert_eq!(
        played_body(apart, "hianime:x-1").await,
        "null",
        "an hour apart"
    );

    // A guess a Continue load stored half a minute after a watch
    // that stored no mapping: builds that stamped and mapped wrote
    // the two a Kitsu read apart, within seconds.
    let td = TempDir::new().expect("tempdir");
    let later = test_app_state(&td);
    crate::commands::kitsu::watched_at_put(&later, "hianime:x-1", now_ms() - 30_000)
        .expect("stamp");
    crate::commands::kitsu::allmanga_kitsu_put(&later, "hianime:x-1", "1623").expect("put");
    assert_eq!(
        played_body(later, "hianime:x-1").await,
        "null",
        "half a minute after"
    );

    // A guess stored in the same second as a watch that recorded
    // no id: written beside the stamp, and still a guess.
    let td = TempDir::new().expect("tempdir");
    let beside = test_app_state(&td);
    crate::commands::kitsu::watched_at_put(&beside, "hianime:x-1", now_ms()).expect("stamp");
    crate::commands::kitsu::allmanga_kitsu_put(&beside, "hianime:x-1", "1623").expect("put");
    assert_eq!(
        played_body(beside, "hianime:x-1").await,
        "null",
        "beside the stamp"
    );

    let td = TempDir::new().expect("tempdir");
    let unmapped = test_app_state(&td);
    crate::commands::kitsu::watched_at_put(&unmapped, "hianime:x-1", now_ms()).expect("stamp");
    assert_eq!(
        played_body(unmapped, "hianime:x-1").await,
        "null",
        "no mapping"
    );
}

async fn delete_status(state: &Arc<AppState>, uri: &str) -> StatusCode {
    build_api_router(Arc::clone(state))
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(uri)
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("oneshot")
        .status()
}

/// An eviction names the id it judged. Continue Watching judges the
/// mapping it read, and a play can store another while it waits on
/// Kitsu; that one, and its mark, are not the one judged and stay.
#[tokio::test]
async fn an_eviction_removes_only_the_mapping_it_names() {
    let td = TempDir::new().expect("tempdir");
    let state = Arc::new(test_app_state(&td));
    crate::commands::kitsu::allmanga_kitsu_put_played(&state, "hianime:x-1", "1623").expect("put");

    let status = delete_status(&state, "/api/allmanga-kitsu-map/hianime:x-1?kitsu_id=49877").await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        crate::commands::kitsu::allmanga_kitsu_get(&state, "hianime:x-1")
            .expect("get")
            .as_deref(),
        Some("1623"),
        "another id's eviction leaves the mapping"
    );
    assert!(
        crate::commands::kitsu_played::mapping_played(&state, "hianime:x-1").expect("played"),
        "and its mark"
    );

    let status = delete_status(&state, "/api/allmanga-kitsu-map/hianime:x-1?kitsu_id=1623").await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        crate::commands::kitsu::allmanga_kitsu_get(&state, "hianime:x-1").expect("get"),
        None,
        "its own eviction removes it"
    );
}

/// An eviction that names no id removes whatever mapping stands.
#[tokio::test]
async fn an_eviction_naming_no_id_removes_the_mapping() {
    let td = TempDir::new().expect("tempdir");
    let state = Arc::new(test_app_state(&td));
    crate::commands::kitsu::allmanga_kitsu_put(&state, "hianime:x-1", "1623").expect("put");

    let status = delete_status(&state, "/api/allmanga-kitsu-map/hianime:x-1").await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(
        crate::commands::kitsu::allmanga_kitsu_get(&state, "hianime:x-1").expect("get"),
        None
    );
}
