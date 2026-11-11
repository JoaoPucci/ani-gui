//! Axum handler for `GET /api/notices`: the published notices for the
//! notification center (see `crate::notices::serve`), once the
//! launch's flags are decided. The backend fetched them with the flags
//! file and kept them only if its signature verified; the renderer
//! never reaches GitHub itself.

use std::sync::Arc;

use axum::extract::State;
use axum::Json;

use crate::app::AppState;
use crate::notices::{served, RemoteNotices};

/// Answers once the launch's flags are decided, like `/api/flags`.
/// The answer does not change for the rest of the session.
pub(super) async fn get_notices(State(state): State<Arc<AppState>>) -> Json<RemoteNotices> {
    let flags = state.flags.settled().await;
    let raw = state.flags.notices();
    Json(served(&flags, raw.as_ref(), env!("CARGO_PKG_VERSION")))
}
