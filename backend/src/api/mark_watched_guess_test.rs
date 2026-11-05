//! A watch a guessed Continue card's play asks to record carries the
//! guessed Kitsu id onto its row only when the row it records is the
//! card's own listing.

use super::tests::test_app_state;
use super::*;
use axum::body::Body;
use axum::http::Request;
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;

const TITLE: &str = "Seitokai ni mo Ana wa Aru!";
const SHOW: &str = "hianime:seitokai-ni-mo-ana-wa-aru-10497";

/// Record a watch of the cached resolution for `TITLE`, which played
/// `SHOW`, under Kitsu id 49877 and `guess_row`, and answer the id the
/// row then records.
async fn recorded_id(guess_row: Option<&str>) -> Option<String> {
    use crate::commands::play_resolution_cache::{cache_key, put, CachedResolution};
    let td = TempDir::new().expect("tempdir");
    let state = test_app_state(&td);
    put(
        &state.cache_pool,
        &cache_key(TITLE, "sub", "best", "1", None, None, None),
        &CachedResolution {
            upstream_url: "https://video.example/master.m3u8".into(),
            referer: String::new(),
            media_kind: crate::proxy::MediaKind::Hls,
            show_id: SHOW.into(),
            show_title: TITLE.into(),
            resolved_slot: Some(1),
            subtitles: Vec::new(),
            kitsu_id: None,
            versions: Vec::new(),
            version: None,
            played: None,
            former_episode: None,
            episode_unverified: false,
        },
    );
    let history = state.history_path.clone();
    let router = build_api_router(Arc::new(state));
    let mut body = serde_json::json!({
        "title": TITLE, "episode": "1", "mode": "sub", "kitsu_id": "49877",
    });
    if let Some(row) = guess_row {
        body["guess_row"] = serde_json::json!(row);
    }
    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/play/mark-watched")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .expect("req"),
        )
        .await
        .expect("oneshot");
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    crate::history::read_all(&history)
        .expect("rows")
        .into_iter()
        .find(|r| r.id == SHOW)
        .and_then(|r| r.kitsu_id)
}

#[tokio::test]
async fn a_guessed_watch_on_the_cards_own_listing_records_the_id() {
    assert_eq!(recorded_id(Some(SHOW)).await.as_deref(), Some("49877"));
    // A bare row from a retired provider, by its slug's words.
    assert_eq!(
        recorded_id(Some("seitokai-ni-mo-ana-wa-aru-77"))
            .await
            .as_deref(),
        Some("49877")
    );
}

#[tokio::test]
async fn a_guessed_watch_on_another_listing_records_no_id() {
    // The resolution the watch reads landed on another listing than the
    // one the session was judged by: the guess is not the row's id.
    assert_eq!(recorded_id(Some("hianime:another-show-1")).await, None);
}

#[tokio::test]
async fn a_watch_no_guess_asked_for_records_its_id() {
    assert_eq!(recorded_id(None).await.as_deref(), Some("49877"));
}

/// Record a guessed watch of the cached resolution for `TITLE`, which
/// played `SHOW`, naming `kitsu_id`, on a row that records `recorded`,
/// and answer the id the row then records.
async fn recorded_over(recorded: &str, kitsu_id: &str) -> Option<String> {
    use crate::commands::play_resolution_cache::{cache_key, put, CachedResolution};
    let td = TempDir::new().expect("tempdir");
    let state = test_app_state(&td);
    put(
        &state.cache_pool,
        &cache_key(TITLE, "sub", "best", "1", None, None, None),
        &CachedResolution {
            upstream_url: "https://video.example/master.m3u8".into(),
            referer: String::new(),
            media_kind: crate::proxy::MediaKind::Hls,
            show_id: SHOW.into(),
            show_title: TITLE.into(),
            resolved_slot: Some(1),
            subtitles: Vec::new(),
            kitsu_id: None,
            versions: Vec::new(),
            version: None,
            played: None,
            former_episode: None,
            episode_unverified: false,
        },
    );
    let history = state.history_path.clone();
    crate::history::write_atomic(
        &history,
        &[crate::history::HistoryEntry {
            ep_no: "1".into(),
            id: SHOW.into(),
            title: TITLE.into(),
            watched_at: None,
            kitsu_id: Some(recorded.into()),
            own_number: true,
        }],
    )
    .expect("history");
    let router = build_api_router(Arc::new(state));
    let body = serde_json::json!({
        "title": TITLE, "episode": "1", "mode": "sub",
        "kitsu_id": kitsu_id, "guess_row": SHOW,
    });
    let response = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/play/mark-watched")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .expect("req"),
        )
        .await
        .expect("oneshot");
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    crate::history::read_all(&history)
        .expect("rows")
        .into_iter()
        .find(|r| r.id == SHOW)
        .and_then(|r| r.kitsu_id)
}

#[tokio::test]
async fn a_guessed_watch_never_records_its_guess_over_a_linked_row() {
    // The user linked the line to 4013 mid-session; a later episode of
    // the guessed session lands on the line's own listing.
    assert_eq!(
        recorded_over("4013", "49877").await.as_deref(),
        Some("4013")
    );
}

#[tokio::test]
async fn a_guessed_watch_naming_the_link_keeps_it() {
    assert_eq!(recorded_over("4013", "4013").await.as_deref(), Some("4013"));
}
