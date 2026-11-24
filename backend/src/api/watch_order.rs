//! Watch-order routes: what each page draws from the watch order,
//! decided by `crate::watch_order` over the franchise walk and the
//! history (`commands::watch_order`). Every route but the rail walks the
//! franchise, so the renderer sends them on the franchise lane.
//!
//! - `POST /api/watch-order/:kitsu_id/show` `{today}` → [`ShowView`]
//! - `POST /api/watch-order/:kitsu_id/franchise` `{today}` → [`FranchiseView`]
//! - `POST /api/watch-order/:kitsu_id/up-next` `{today, episode}` → [`UpNextView`]
//! - `POST /api/watch-order/:kitsu_id/handoff` `{today}` → [`HandoffAnswer`]
//! - `POST /api/watch-order/rail` [`RailRequest`] → [`RailView`]
//!
//! `today` is the viewer's local day, `YYYY-MM-DD`: what "not aired
//! yet" is judged against.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::app::AppState;
use crate::error::AniError;
use crate::watch_order::handoff::Handoff;
use crate::watch_order::wire::{
    rail_view, FranchiseView, RailRequest, RailView, ShowView, UpNextView,
};

/// Mount the watch-order routes. Called from [`crate::api::build_api_router`].
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/watch-order/rail", post(post_rail))
        .route("/api/watch-order/:kitsu_id/show", post(post_show))
        .route("/api/watch-order/:kitsu_id/franchise", post(post_franchise))
        .route("/api/watch-order/:kitsu_id/up-next", post(post_up_next))
        .route("/api/watch-order/:kitsu_id/handoff", post(post_handoff))
}

/// The day a page is read on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DayArgs {
    /// The viewer's local day, `YYYY-MM-DD`.
    pub today: String,
}

/// The day and the episode the player is on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpNextArgs {
    /// The viewer's local day, `YYYY-MM-DD`.
    pub today: String,
    /// The episode playing (an extra's `12.5` included).
    pub episode: f64,
}

/// A row's hand-off: the entry it moves on to, or none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HandoffAnswer {
    /// The hand-off, where the row moves on.
    pub handoff: Option<Handoff>,
}

async fn post_show(
    State(state): State<Arc<AppState>>,
    Path(kitsu_id): Path<String>,
    Json(args): Json<DayArgs>,
) -> Result<Json<ShowView>, AniError> {
    let id = crate::kitsu_id::require(&kitsu_id)?;
    Ok(Json(
        crate::commands::watch_order::show(&state, id, &args.today).await?,
    ))
}

async fn post_franchise(
    State(state): State<Arc<AppState>>,
    Path(kitsu_id): Path<String>,
    Json(args): Json<DayArgs>,
) -> Result<Json<FranchiseView>, AniError> {
    let id = crate::kitsu_id::require(&kitsu_id)?;
    Ok(Json(
        crate::commands::watch_order::franchise_page(&state, id, &args.today).await?,
    ))
}

async fn post_up_next(
    State(state): State<Arc<AppState>>,
    Path(kitsu_id): Path<String>,
    Json(args): Json<UpNextArgs>,
) -> Result<Json<UpNextView>, AniError> {
    let id = crate::kitsu_id::require(&kitsu_id)?;
    Ok(Json(
        crate::commands::watch_order::up_next(&state, id, &args.today, args.episode).await?,
    ))
}

async fn post_handoff(
    State(state): State<Arc<AppState>>,
    Path(kitsu_id): Path<String>,
    Json(args): Json<DayArgs>,
) -> Result<Json<HandoffAnswer>, AniError> {
    let id = crate::kitsu_id::require(&kitsu_id)?;
    Ok(Json(HandoffAnswer {
        handoff: crate::commands::watch_order::handoff(&state, id, &args.today).await?,
    }))
}

async fn post_rail(Json(req): Json<RailRequest>) -> Json<RailView> {
    Json(rail_view(&req))
}

#[cfg(test)]
#[path = "watch_order_test.rs"]
mod tests;
