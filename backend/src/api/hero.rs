//! The home hero's routes (`crate::hero`): `POST /api/home/hero`
//! makes the pick from the trending shows the renderer kept after its
//! availability filter, under the launch's flags, and keeps the walk
//! it returned; `GET /api/home/hero/walk` hands that walk to the Debug
//! page, read-only, with the flag it ran under. Split from
//! `api/mod.rs` like `api/airing.rs`.

use std::sync::Arc;

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use serde::{Deserialize, Serialize};

use crate::app::AppState;
use crate::error::AniError;
use crate::flags::store::Origin;
use crate::hero::last::LastWalk;
use crate::hero::{HeroShow, Pick, Walk, HERO_AIRING_FIRST, MAX_SHOWS};

/// Mount the hero routes, with the store of the last walk they share.
/// Called from [`crate::api::build_api_router`], once per router.
pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/home/hero", post(post_hero))
        .route("/api/home/hero/walk", get(get_walk))
        .layer(Extension(LastWalk::default()))
}

/// The pick's body: the trending shows, in trending order.
#[derive(Debug, Deserialize)]
pub struct HeroPickBody {
    /// The shows, as much of each as the pick reads.
    pub shows: Vec<HeroShow>,
}

/// The airing preference as the Debug page shows it.
#[derive(Debug, Serialize)]
pub struct FlagView {
    /// The flag's name.
    pub name: &'static str,
    /// Whether it is on for this launch.
    pub on: bool,
    /// Where the launch's flags came from. On implies a file whose
    /// signature verified, since no other file may turn a flag on.
    pub origin: Origin,
}

/// The Debug page's view: the flag, and the last pick's walk.
#[derive(Debug, Serialize)]
pub struct WalkView {
    /// The airing preference.
    pub flag: FlagView,
    /// The last walk; `None` until Home has asked for a pick.
    pub walk: Option<Walk>,
}

/// Reads its own body, so a malformed one answers the app's keyed
/// envelope rather than the framework's plain-text rejection.
async fn post_hero(
    State(state): State<Arc<AppState>>,
    Extension(last): Extension<LastWalk>,
    body: axum::body::Bytes,
) -> Result<Json<Pick>, AniError> {
    let body: HeroPickBody =
        serde_json::from_slice(&body).map_err(|_| AniError::InvalidRequestBody)?;
    if body.shows.len() > MAX_SHOWS {
        return Err(AniError::InvalidRequestBody);
    }
    let flags = state.flags.settled().await;
    let pick = crate::hero::pick_for(&flags, &body.shows);
    last.record(pick.walk.clone());
    Ok(Json(pick))
}

async fn get_walk(
    State(state): State<Arc<AppState>>,
    Extension(last): Extension<LastWalk>,
) -> Json<WalkView> {
    let flags = state.flags.settled().await;
    Json(WalkView {
        flag: FlagView {
            name: HERO_AIRING_FIRST.name,
            on: flags.enabled(&HERO_AIRING_FIRST),
            origin: state.flags.origin(),
        },
        walk: last.get(),
    })
}
