//! A guessed Continue card's play that lands on another listing writes
//! a row of its own there, and the history names the card it came from
//! beside that row, so Home's card stands for the line the user clicked.
//!
//! JoJo's 2000 OVA card (`…-2000-5182`), guessed as the 2012 series,
//! plays the 2012 series, whose resolve lands on the 2012 listing
//! (`…-2012-708`) and writes that row.

use super::tests::{body_string, test_app_state};
use super::*;
use axum::body::Body;
use axum::http::Request;
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;

const TITLE: &str = "JoJo no Kimyou na Bouken (TV)";
const CARD: &str = "hianime:jojos-bizarre-adventure-2000-5182";
const PLAYED: &str = "hianime:jojos-bizarre-adventure-2012-708";

fn router_with_resolution(td: &TempDir) -> (axum::Router, std::path::PathBuf) {
    use crate::commands::play_resolution_cache::{cache_key, put, CachedResolution};
    let state = test_app_state(td);
    put(
        &state.cache_pool,
        &cache_key(TITLE, "sub", "best", "2", None, None, None),
        &CachedResolution {
            upstream_url: "https://video.example/master.m3u8".into(),
            referer: String::new(),
            media_kind: crate::proxy::MediaKind::Hls,
            show_id: PLAYED.into(),
            show_title: "JoJo's Bizarre Adventure (2012)".into(),
            resolved_slot: Some(2),
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
            id: CARD.into(),
            title: "JoJo's Bizarre Adventure".into(),
            watched_at: None,
            kitsu_id: None,
            own_number: true,
        }],
    )
    .expect("history");
    (build_api_router(Arc::new(state)), history)
}

async fn mark_watched(router: &axum::Router, guess_row: Option<&str>, kitsu_id: Option<&str>) {
    let mut body = serde_json::json!({ "title": TITLE, "episode": "2", "mode": "sub" });
    if let Some(row) = guess_row {
        body["guess_row"] = serde_json::json!(row);
    }
    if let Some(id) = kitsu_id {
        body["kitsu_id"] = serde_json::json!(id);
    }
    let response = router
        .clone()
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
}

async fn rows(router: &axum::Router) -> Vec<serde_json::Value> {
    let response = router
        .clone()
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
    serde_json::from_str::<serde_json::Value>(&body_string(response).await)
        .expect("json")
        .as_array()
        .expect("array")
        .clone()
}

fn row<'a>(rows: &'a [serde_json::Value], id: &str) -> &'a serde_json::Value {
    rows.iter().find(|r| r["id"] == id).expect("row")
}

#[tokio::test]
async fn a_guessed_play_that_lands_on_another_listing_names_the_card_it_came_from() {
    let td = TempDir::new().expect("tempdir");
    let (router, _) = router_with_resolution(&td);
    mark_watched(&router, Some(CARD), None).await;
    let rows = rows(&router).await;
    assert_eq!(row(&rows, PLAYED)["guessed_from"], CARD);
    // The card's own row came from nowhere: it is the line clicked.
    assert!(row(&rows, CARD).get("guessed_from").is_none());
}

#[tokio::test]
async fn a_guessed_play_on_the_cards_own_listing_names_no_card() {
    let td = TempDir::new().expect("tempdir");
    let (router, _) = router_with_resolution(&td);
    mark_watched(&router, Some(PLAYED), None).await;
    let rows = rows(&router).await;
    assert!(row(&rows, PLAYED).get("guessed_from").is_none());
}

#[tokio::test]
async fn a_later_play_of_the_row_no_guess_asked_for_makes_it_its_own() {
    let td = TempDir::new().expect("tempdir");
    let (router, _) = router_with_resolution(&td);
    mark_watched(&router, Some(CARD), None).await;
    // Played again from the 2012 series' own page.
    mark_watched(&router, None, Some("7158")).await;
    let rows = rows(&router).await;
    assert!(row(&rows, PLAYED).get("guessed_from").is_none());
}

#[tokio::test]
async fn removing_the_row_forgets_the_card_it_came_from() {
    let td = TempDir::new().expect("tempdir");
    let (router, history) = router_with_resolution(&td);
    mark_watched(&router, Some(CARD), None).await;
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/history/{PLAYED}"))
                .body(Body::empty())
                .expect("req"),
        )
        .await
        .expect("oneshot");
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(crate::history::read_all(&history)
        .expect("rows")
        .iter()
        .all(|r| r.id != PLAYED));
    // A row written there again, by a play no guess asked for, is its
    // own: nothing the removed row left names a card for it.
    crate::history::upsert_and_write(
        &history,
        crate::history::HistoryEntry {
            ep_no: "3".into(),
            id: PLAYED.into(),
            title: "JoJo's Bizarre Adventure (2012)".into(),
            watched_at: None,
            kitsu_id: None,
            own_number: true,
        },
    )
    .expect("row");
    let rows = rows(&router).await;
    assert!(row(&rows, PLAYED).get("guessed_from").is_none());
}
