//! Franchise routes — `GET /api/kitsu/franchise/:kitsu_id` returns the
//! run an anime belongs to and what sits around it
//! ([`crate::commands::franchise_walk::Franchise`]), and
//! `GET /api/kitsu/franchise/:kitsu_id/first-hop` what the show page
//! will draw from it, as the anime's own relation listing foretells
//! ([`crate::commands::franchise_walk::first_hop`]), so the page can
//! hold room for it while the walk runs. Split from `api/mod.rs` like
//! `api/airing.rs`.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};

use crate::app::AppState;
use serde::Serialize;

use crate::commands::franchise_walk::first_hop::{first_hop, FirstHop};
use crate::commands::franchise_walk::Franchise;
use crate::error::AniError;

/// Mount the franchise route. Called from
/// [`crate::api::build_api_router`].
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/kitsu/franchise/:kitsu_id", get(get_franchise))
        .route(
            "/api/kitsu/franchise/:kitsu_id/first-hop",
            get(get_first_hop),
        )
}

/// The first hop's answer: `{"signal": "strip" | "link" | "none"}`.
#[derive(Serialize)]
struct FirstHopBody {
    signal: FirstHop,
}

async fn get_first_hop(
    State(state): State<Arc<AppState>>,
    Path(kitsu_id): Path<String>,
) -> Result<Json<FirstHopBody>, AniError> {
    // The walk's own read and cache, so the walk that follows finds the
    // listing cached and the two never read different data.
    let kitsu_id = crate::kitsu_id::require(&kitsu_id)?;
    let listing = crate::commands::franchise::franchise_listing(&state, kitsu_id).await?;
    Ok(Json(FirstHopBody {
        signal: first_hop(kitsu_id, &listing),
    }))
}

async fn get_franchise(
    State(state): State<Arc<AppState>>,
    Path(kitsu_id): Path<String>,
) -> Result<Json<Franchise>, AniError> {
    let kitsu_id = crate::kitsu_id::require(&kitsu_id)?;
    Ok(Json(
        crate::commands::franchise::franchise(&state, kitsu_id).await?,
    ))
}

#[cfg(test)]
#[path = "franchise_test.rs"]
mod tests;
