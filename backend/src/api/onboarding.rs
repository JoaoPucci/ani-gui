//! The feature tips' routes (`crate::onboarding`):
//!
//! - `GET /api/onboarding` → [`OnboardingView`]: the tips offered to
//!   this install at the running release, each saying whether it is
//!   still pending.
//! - `POST /api/onboarding/seen` `{id}` → 204: the tip was closed, by
//!   any route; it is never pending again.
//! - `GET /api/onboarding/debug` → [`DebugView`]: every declared tip
//!   with this install's state, for the Debug page.
//! - `PUT /api/onboarding/debug/seen` `{id, seen}` → 204: the Debug
//!   page's seen toggle.
//!
//! Each route first takes this launch into the record
//! ([`eligibility::sync`]) under the launch's flags, once they have
//! settled, and writes the record back when that changed it. One lock
//! per router serialises the reads and writes, so two closes in flight
//! cannot drop each other's mark.

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post, put};
use axum::{Extension, Json, Router};
use serde::Deserialize;

use crate::app::AppState;
use crate::error::AniError;
use crate::flags::FlagValue;
use crate::onboarding::decl::TIPS;
use crate::onboarding::eligibility;
use crate::onboarding::release::Release;
use crate::onboarding::state::{self, Record};
use crate::onboarding::view::{self, DebugView, OnboardingView};

/// What the routes share: the release counted at, and the lock.
#[derive(Clone)]
struct Ctx {
    version: Release,
    lock: Arc<tokio::sync::Mutex<()>>,
}

/// Mount the routes, counting at the running release. Called from
/// [`crate::api::build_api_router`].
pub fn router() -> Router<Arc<AppState>> {
    router_at(crate::onboarding::running_version())
}

/// The routes, counting at `version`.
pub(crate) fn router_at(version: Release) -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/onboarding", get(get_onboarding))
        .route("/api/onboarding/seen", post(post_seen))
        .route("/api/onboarding/debug", get(get_debug))
        .route("/api/onboarding/debug/seen", put(put_debug_seen))
        .layer(Extension(Ctx {
            version,
            lock: Arc::default(),
        }))
}

/// `POST /api/onboarding/seen`'s body.
#[derive(Debug, Deserialize)]
pub struct SeenBody {
    /// The tip's id.
    pub id: String,
}

/// `PUT /api/onboarding/debug/seen`'s body.
#[derive(Debug, Deserialize)]
pub struct DebugSeenBody {
    /// The tip's id.
    pub id: String,
    /// Seen, or not.
    pub seen: bool,
}

/// The record with this launch taken in, the flags it was taken under,
/// and where it lives. Written back when the launch changed it.
async fn synced(state: &AppState, version: Release) -> (Record, impl Fn(&str) -> bool, PathBuf) {
    let flags = state.flags.settled().await;
    let flag_on = move |name: &str| flags.get(name) == Some(FlagValue::Bool(true));
    let path = state::path_in(&state.state_dir);
    let loaded = state::load(&path).or_empty();
    let record = eligibility::sync(&TIPS, &loaded, version, &flag_on);
    if record != loaded {
        if let Err(e) = state::save(&path, &record) {
            tracing::warn!(target: "onboarding", error = %e, "could not write the onboarding record");
        }
    }
    (record, flag_on, path)
}

async fn get_onboarding(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<Ctx>,
) -> Json<OnboardingView> {
    let _held = ctx.lock.lock().await;
    let (record, flag_on, _) = synced(&state, ctx.version).await;
    Json(view::offered_view(&TIPS, &record, ctx.version, flag_on))
}

async fn get_debug(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<Ctx>,
) -> Json<DebugView> {
    let _held = ctx.lock.lock().await;
    let (record, flag_on, _) = synced(&state, ctx.version).await;
    Json(view::debug_view(&TIPS, &record, ctx.version, flag_on))
}

/// Reads its own body, so a malformed one answers the app's keyed
/// envelope rather than the framework's plain-text rejection.
async fn post_seen(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<Ctx>,
    body: axum::body::Bytes,
) -> Result<StatusCode, AniError> {
    let body: SeenBody = serde_json::from_slice(&body).map_err(|_| AniError::InvalidRequestBody)?;
    mark(&state, &ctx, &body.id, true).await
}

async fn put_debug_seen(
    State(state): State<Arc<AppState>>,
    Extension(ctx): Extension<Ctx>,
    body: axum::body::Bytes,
) -> Result<StatusCode, AniError> {
    let body: DebugSeenBody =
        serde_json::from_slice(&body).map_err(|_| AniError::InvalidRequestBody)?;
    mark(&state, &ctx, &body.id, body.seen).await
}

/// Mark tip `id` and write the record. An id no tip has, or an
/// on-demand tip's, is a bad request; a record that cannot be written
/// is an I/O failure.
async fn mark(state: &AppState, ctx: &Ctx, id: &str, seen: bool) -> Result<StatusCode, AniError> {
    let _held = ctx.lock.lock().await;
    let (record, _, path) = synced(state, ctx.version).await;
    let next = view::mark(&TIPS, &record, id, seen).map_err(|_| AniError::InvalidRequestBody)?;
    if next != record {
        state::save(&path, &next).map_err(|_| AniError::Io)?;
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
#[path = "onboarding_test.rs"]
mod tests;
