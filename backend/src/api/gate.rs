//! The API answers the app's own callers only.
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
//! The secret travels as the `x-ani-gui-internal-secret` header. Three
//! routes are loaded by elements that cannot set a header — the play
//! and download progress streams (an `EventSource`) and `/api/image`
//! (an `<img src>`) — and those also take it as the `secret` query
//! parameter. No other route reads the query for it.
//!
//! The stream proxy (`/s/<session>/…`) is not behind this gate: the
//! player's media element and the download tools fetch it without
//! headers, and the session id in its path — a random UUID only the
//! play that made the session learns — is its guard.

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{HeaderMap, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::account::InternalSecret;
use crate::app::AppState;

/// The query parameter that carries the secret on the routes that
/// cannot be sent a header.
pub const SECRET_QUERY: &str = "secret";

/// The routes whose callers cannot set a header, and so may carry the
/// secret in the query instead. `GET` only.
pub const QUERY_SECRET_PATHS: [&str; 3] =
    ["/api/image", "/api/play/stream", "/api/download/stream"];

/// Whether a request carries the launch's secret: as the header on
/// any route, or as the [`SECRET_QUERY`] parameter on a `GET` of one of
/// [`QUERY_SECRET_PATHS`].
#[must_use]
pub fn admits(
    secret: &InternalSecret,
    method: &Method,
    path: &str,
    query: Option<&str>,
    headers: &HeaderMap,
) -> bool {
    if secret.validate_header(headers).is_ok() {
        return true;
    }
    if method != Method::GET || !QUERY_SECRET_PATHS.contains(&path) {
        return false;
    }
    query.is_some_and(|q| {
        url::form_urlencoded::parse(q.as_bytes())
            .any(|(k, v)| k == SECRET_QUERY && secret.matches(&v))
    })
}

/// The middleware: pass an admitted request on, refuse the rest.
pub async fn require_secret(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Response {
    let uri = request.uri();
    if admits(
        &state.internal_secret,
        request.method(),
        uri.path(),
        uri.query(),
        request.headers(),
    ) {
        next.run(request).await
    } else {
        StatusCode::FORBIDDEN.into_response()
    }
}

#[cfg(test)]
#[path = "gate_test.rs"]
mod tests;
