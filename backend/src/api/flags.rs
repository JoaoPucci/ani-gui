//! Axum handler for `GET /api/flags`: the renderer's view of the
//! remote flags (see `crate::flags`). The backend fetched them; the
//! renderer never reaches GitHub for them itself.

use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::app::AppState;
use crate::flags::Resolved;

#[derive(Serialize)]
pub(super) struct FlagsResponse {
    /// Whether the launch refresh has finished. Until it has, `flags`
    /// may still be a fresh cache or the defaults.
    settled: bool,
    /// Every declared flag and its value.
    flags: Resolved,
}

/// Answers at once with the flags as they stand; never waits for the
/// launch refresh.
pub(super) async fn get_flags(State(state): State<Arc<AppState>>) -> Json<FlagsResponse> {
    Json(FlagsResponse {
        settled: state.flags.is_settled(),
        flags: state.flags.current(),
    })
}
