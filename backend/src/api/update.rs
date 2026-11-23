//! Axum handler for the `/api/update-check` route.
//!
//! Lives in its own submodule (not inline in `api/mod.rs`) so the
//! parent module's aggregate ccn doesn't tick up every time a new
//! handler lands. The actual GitHub lookup lives in
//! `crate::meta::github` — this is the thin HTTP adapter.

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;

use crate::app::AppState;

#[derive(serde::Deserialize, Default)]
pub(super) struct UpdateCheckQuery {
    /// Whether pre-releases count as an offer. Defaults to true
    /// because every ani-gui release shipped so far is marked
    /// prerelease=true — with it off, nothing is offered today.
    #[serde(default = "default_include_prereleases")]
    include_prereleases: bool,
}

fn default_include_prereleases() -> bool {
    true
}

/// `GET /api/update-check?include_prereleases=true` — wraps the
/// upstream GitHub releases lookup behind the localhost API
/// boundary. The renderer used to call `api.github.com` directly,
/// breaking the documented "backend owns outbound HTTP" rule (see
/// `docs/architecture.md`).
///
/// The offer is the newest release with a package for the
/// platform this binary was built for (see `meta::github`).
///
/// Returns 200 + JSON `ReleaseInfo` when a release is found, or
/// 204 No Content when none is (no release qualifies, or the app
/// was built for a platform it does not package) and for every
/// soft failure (offline, rate-limited, malformed payload). The frontend
/// branches on the body's presence — no error path to plumb.
pub(super) async fn get_update_check(
    State(state): State<Arc<AppState>>,
    Query(q): Query<UpdateCheckQuery>,
) -> Response {
    let release = crate::meta::github::fetch_latest_release(
        &state.meta_http,
        q.include_prereleases,
        crate::meta::github::Platform::current(),
        None,
    )
    .await;
    match release {
        Some(r) => Json(r).into_response(),
        None => StatusCode::NO_CONTENT.into_response(),
    }
}
