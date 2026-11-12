//! Axum handler for `GET /api/flags`: the renderer's view of the
//! remote flags (see `crate::flags`). The backend fetched them; the
//! renderer never reaches GitHub for them itself.

use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::app::AppState;
use crate::flags::store::Origin;
use crate::flags::Resolved;

#[derive(Serialize)]
pub(super) struct FlagsResponse {
    /// Where the launch's flags came from: `file`, `cache`,
    /// `defaults`, or `test` in a test run.
    origin: Origin,
    /// Every declared flag and its value.
    flags: Resolved,
}

/// Answers with the launch's flags once they are decided — within
/// the settle cap, or the store's own limit past it. The values do not
/// change for the rest of the session, so the renderer asks once,
/// before its first page.
pub(super) async fn get_flags(State(state): State<Arc<AppState>>) -> Json<FlagsResponse> {
    let flags = state.flags.settled().await;
    Json(FlagsResponse {
        origin: state.flags.origin(),
        flags,
    })
}
