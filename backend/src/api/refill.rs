//! The reads a background fill completes — a show's detail (its AniList
//! banner) and its episode pages (their AniList thumbnails) — and the
//! route a page waits on when one of those fills was held back.
//!
//! A read served without a fill the AniList gate held back carries the
//! tickets of the retries that will bring it, in [`REFILL_HEADER`]
//! (comma-separated). The page posts them to `/api/refills/wait`, which
//! answers once one of them has ended — `[{ "ticket": 3, "landed":
//! true }]` — or with `[]` after [`WAIT_LIMIT`], and fetches the read
//! once more when one landed (`commands::held_fill`). A page that
//! closes posts its tickets to `/api/refills/release`: their retries
//! then rank after every fill still on screen. Split from
//! `api/mod.rs` (like `api/airing.rs`) to keep that file under the CRAP
//! ceiling.

use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderName, HeaderValue};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::app::AppState;
use crate::commands::{held_fill, kitsu as kitsu_inner};
use crate::error::AniError;

/// The header a read carries its pending refills' tickets in.
pub const REFILL_HEADER: &str = "x-ani-gui-refill";

/// The longest `/api/refills/wait` holds a request open. The page asks
/// again while it still waits; shorter than any idle timeout between.
const WAIT_LIMIT: Duration = Duration::from_secs(20);

/// The most tickets one wait may name.
const MAX_TICKETS: usize = 256;

/// Mount the routes. Called from [`crate::api::build_api_router`].
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/kitsu/anime/:id", get(get_kitsu_anime_detail))
        .route("/api/kitsu/episodes/:anime_id", get(get_kitsu_episodes))
        .route("/api/refills/wait", post(post_refills_wait))
        .route("/api/refills/release", post(post_refills_release))
}

/// `body` as the response, with `tickets` in [`REFILL_HEADER`] when
/// there are any.
fn with_refills(body: impl IntoResponse, tickets: &[u64]) -> Response {
    let mut resp = body.into_response();
    if tickets.is_empty() {
        return resp;
    }
    let joined = tickets
        .iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(",");
    if let Ok(value) = HeaderValue::from_str(&joined) {
        resp.headers_mut()
            .insert(HeaderName::from_static(REFILL_HEADER), value);
    }
    resp
}

async fn get_kitsu_anime_detail(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Response, AniError> {
    let id = crate::kitsu_id::require(&id)?;
    let (detail, tickets) =
        held_fill::collect(kitsu_inner::kitsu_anime_detail_page(&state, id)).await;
    Ok(with_refills(Json(detail?), &tickets))
}

#[derive(Deserialize)]
struct EpisodesQuery {
    page: Option<u32>,
    /// Skip the cached page — sent only by a click on a tile the
    /// schedule calls unaired.
    #[serde(default)]
    refresh: bool,
}

async fn get_kitsu_episodes(
    State(state): State<Arc<AppState>>,
    Path(anime_id): Path<String>,
    Query(q): Query<EpisodesQuery>,
) -> Result<Response, AniError> {
    let anime_id = crate::kitsu_id::require(&anime_id)?;
    let page = q.page.unwrap_or(1);
    let (eps, tickets) = held_fill::collect(kitsu_inner::kitsu_episodes_with(
        &state, anime_id, page, q.refresh,
    ))
    .await;
    Ok(with_refills(Json(eps?), &tickets))
}

#[derive(Deserialize)]
struct WaitBody {
    tickets: Vec<u64>,
}

#[derive(Serialize)]
struct Ended {
    ticket: u64,
    landed: bool,
}

async fn post_refills_wait(Json(body): Json<WaitBody>) -> Json<Vec<Ended>> {
    let tickets: Vec<u64> = body.tickets.into_iter().take(MAX_TICKETS).collect();
    let ended = held_fill::wait_any(&tickets, WAIT_LIMIT).await;
    Json(
        ended
            .into_iter()
            .map(|(ticket, landed)| Ended { ticket, landed })
            .collect(),
    )
}

/// The page showing these tickets' fills has gone: their retries rank
/// after every fill still on screen.
async fn post_refills_release(Json(body): Json<WaitBody>) -> axum::http::StatusCode {
    let tickets: Vec<u64> = body.tickets.into_iter().take(MAX_TICKETS).collect();
    held_fill::release(&tickets);
    axum::http::StatusCode::NO_CONTENT
}

#[cfg(test)]
#[path = "refill_test.rs"]
mod tests;
