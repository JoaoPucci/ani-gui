//! The API answers only requests carrying the launch's secret.
//!
//! Every lane is a loopback listener any program on the machine can
//! connect to, and any page a browser on it opens can send requests
//! to. The backend prints a per-launch secret in its handshake
//! (`crate::lanes::handshake_lines`) to the Electron main process,
//! which hands it to the renderer through the preload bridge; every
//! route under `/api` answers only a request that carries it. A request
//! without it gets `403 Forbidden` with an empty body, whatever it
//! asked for, so a refusal tells the caller nothing about the route.
//!
//! The secret travels as the `x-ani-gui-internal-secret` header, on
//! every route. Three routes are loaded by elements that cannot set a
//! header — the play and download progress streams (an `EventSource`)
//! and `/api/image` (an `<img src>`) — and the Electron main process
//! adds it to those requests for the app's own page
//! (electron/lib/secret-header.cjs). No route reads it from the query:
//! a URL is kept where a header is not, in Chromium's disk cache and in
//! any log line that names it.
//!
//! The stream proxy (`/s/<session>/…`) is not behind this gate: the
//! player's media element and the download tools fetch it without
//! headers, and the session id in its path — a random UUID only the
//! play that made the session learns — is its guard.

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::account::InternalSecret;
use crate::app::AppState;

/// Whether a request carries the launch's secret, as the header.
#[must_use]
pub fn admits(secret: &InternalSecret, headers: &HeaderMap) -> bool {
    secret.validate_header(headers).is_ok()
}

/// The middleware: pass an admitted request on, refuse the rest.
pub async fn require_secret(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    if admits(&state.internal_secret, request.headers()) {
        next.run(request).await
    } else {
        StatusCode::FORBIDDEN.into_response()
    }
}

#[cfg(test)]
#[path = "gate_test.rs"]
mod tests;
