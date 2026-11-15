//! `POST /api/history/switch-episode`: where "Switch to it now" opens
//! the show picked for a card, from the history.

use super::tests::{body_string, test_app_state};
use super::*;
use axum::body::Body;
use axum::http::Request;
use std::sync::Arc;
use tempfile::TempDir;
use tower::ServiceExt;

const CARD: &str = "hianime:jojos-bizarre-adventure-diamond-is-unbreakable-1388";
const OWN: &str = "hianime:jojos-bizarre-adventure-stardust-crusaders-718";
const OTHER_LISTING: &str = "hianime:jojo-s-bizarre-adventure-part-2-stardust-crusaders-9760";

fn row(ep: &str, id: &str, at: i64, kitsu: Option<&str>) -> crate::history::HistoryEntry {
    crate::history::HistoryEntry {
        ep_no: ep.into(),
        id: id.into(),
        title: id.into(),
        watched_at: Some(at),
        kitsu_id: kitsu.map(str::to_string),
        own_number: true,
    }
}

async fn ask(rows: &[crate::history::HistoryEntry], body: serde_json::Value) -> serde_json::Value {
    let td = TempDir::new().expect("tempdir");
    let state = test_app_state(&td);
    crate::history::write_atomic(&state.history_path, rows).expect("history");
    let response = test_router(Arc::new(state))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/history/switch-episode")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .expect("req"),
        )
        .await
        .expect("oneshot");
    assert_eq!(response.status(), StatusCode::OK);
    serde_json::from_str(&body_string(response).await).expect("json")
}

#[tokio::test]
async fn the_card_corrected_to_a_show_watched_further_opens_where_it_was_left() {
    // The card's line was just named Stardust Crusaders and played
    // last, at episode 2: it is not the pick's own progress.
    let rows = [
        row("2", CARD, 3_000, Some("8063")),
        row("6", OWN, 1_000, Some("8063")),
    ];
    let body =
        serde_json::json!({ "pick": "8063", "card": CARD, "episode": 2, "episode_count": 48 });
    assert_eq!(
        ask(&rows, body).await,
        serde_json::json!({ "episode": 7.0 })
    );
}

#[tokio::test]
async fn the_picks_progress_on_another_listing_counts() {
    let rows = [
        row("2", CARD, 3_000, Some("8063")),
        row("3", OWN, 1_000, Some("8063")),
        row("6", OTHER_LISTING, 2_000, Some("8063")),
    ];
    let body = serde_json::json!({
        "pick": "8063", "card": CARD, "episode": 2, "episode_count": 48, "part_way": [6]
    });
    assert_eq!(
        ask(&rows, body).await,
        serde_json::json!({ "episode": 6.0 })
    );
}

#[tokio::test]
async fn a_pick_behind_or_without_progress_takes_the_cards_episode() {
    let behind = [
        row("2", CARD, 3_000, Some("8063")),
        row("1", OWN, 1_000, Some("8063")),
    ];
    let body = serde_json::json!({ "pick": "8063", "card": CARD, "episode": 2 });
    assert_eq!(
        ask(&behind, body.clone()).await,
        serde_json::json!({ "episode": null })
    );
    let none = [row("2", CARD, 3_000, Some("8063"))];
    assert_eq!(
        ask(&none, body).await,
        serde_json::json!({ "episode": null })
    );
}

#[tokio::test]
async fn a_bare_cards_walked_row_is_the_cards_listing_not_the_picks_progress() {
    // A retired provider's bare card and the walked row with its words
    // are one listing: neither is the pick's own progress.
    let bare = "jojos-bizarre-adventure-diamond-is-unbreakable-77";
    let rows = [
        row(
            "9",
            "hianime:jojos-bizarre-adventure-diamond-is-unbreakable-1388",
            3_000,
            Some("8063"),
        ),
        row("2", bare, 2_000, Some("8063")),
        row("6", OWN, 1_000, Some("8063")),
    ];
    let body =
        serde_json::json!({ "pick": "8063", "card": bare, "episode": 2, "episode_count": 48 });
    assert_eq!(
        ask(&rows, body).await,
        serde_json::json!({ "episode": 7.0 })
    );
}

#[tokio::test]
async fn a_pick_that_is_no_kitsu_id_is_refused() {
    let td = TempDir::new().expect("tempdir");
    let state = test_app_state(&td);
    let response = test_router(Arc::new(state))
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/history/switch-episode")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({ "pick": "not an id", "card": CARD, "episode": 2 })
                        .to_string(),
                ))
                .expect("req"),
        )
        .await
        .expect("oneshot");
    assert!(response.status().is_client_error());
}
