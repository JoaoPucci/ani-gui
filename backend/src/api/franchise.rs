//! Franchise route — `GET /api/kitsu/franchise/:kitsu_id` returns the
//! run an anime belongs to and what sits around it
//! ([`crate::commands::franchise_walk::Franchise`]). Split from
//! `api/mod.rs` like `api/airing.rs`.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};

use crate::app::AppState;
use crate::commands::franchise_walk::Franchise;
use crate::error::AniError;

/// Mount the franchise route. Called from
/// [`crate::api::build_api_router`].
pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/api/kitsu/franchise/:kitsu_id", get(get_franchise))
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
