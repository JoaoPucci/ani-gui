//! The origins the app's own renderer runs at, and the CORS layer that
//! answers only them.
//!
//! The backend's listeners are reachable from every program on the
//! machine and from every page any browser on it opens, since all of
//! them can reach a loopback port. What keeps them out of the API is
//! the per-launch secret (`crate::api::gate`); CORS decides which
//! pages a browser lets read what the backend answers, and it names
//! only the renderer's origins:
//!
//! - `app://localhost`, the packaged app on every platform: the
//!   Electron shell serves the static bundle under its own `app:`
//!   scheme (`electron/main.js`), on Linux and Windows alike.
//! - `http://localhost`, `http://127.0.0.1` or `http://[::1]`, on any
//!   port: the Vite dev server a dev run loads, wherever
//!   `VITE_DEV_URL` puts it. These cost nothing in a packaged run —
//!   a page there still lacks the secret, and every API route refuses
//!   it — and the backend cannot tell which port a dev run picked
//!   without being told at launch.
//!
//! Everything else — `null` (a sandboxed frame, a `file:` page), any
//! `https:` origin, another host — gets no `Access-Control-Allow-Origin`,
//! so a browser neither sends it a preflighted request nor shows it an
//! answer.

use std::time::Duration;

use tower_http::cors::{AllowOrigin, Any, CorsLayer};

/// The packaged renderer's origin.
pub const PACKAGED: &str = "app://localhost";

/// How long a browser may reuse a preflight's answer. Chromium caps it
/// at two hours; the renderer's requests carry the secret as a header,
/// which makes even a `GET` preflighted, so the cache spares a second
/// round trip on every repeat of a URL.
pub const PREFLIGHT_MAX_AGE: Duration = Duration::from_secs(2 * 60 * 60);

/// Whether `origin` — an `Origin` request header's value — is one the
/// app's renderer runs at.
#[must_use]
pub fn is_renderer_origin(origin: &[u8]) -> bool {
    let Ok(origin) = std::str::from_utf8(origin) else {
        return false;
    };
    if origin == PACKAGED {
        return true;
    }
    let Some(rest) = origin.strip_prefix("http://") else {
        return false;
    };
    ["localhost", "127.0.0.1", "[::1]"]
        .iter()
        .any(|host| rest.strip_prefix(host).is_some_and(is_port_suffix))
}

/// `""` or `:<port>`, a port being one to five ASCII digits naming a
/// value up to 65535.
fn is_port_suffix(rest: &str) -> bool {
    if rest.is_empty() {
        return true;
    }
    let Some(port) = rest.strip_prefix(':') else {
        return false;
    };
    (1..=5).contains(&port.len())
        && port.bytes().all(|b| b.is_ascii_digit())
        && port.parse::<u32>().is_ok_and(|p| p <= 65_535)
}

/// The CORS layer every router the renderer reads from carries: the
/// renderer's origins only, any method and header (the origin is what
/// narrows it), every response header readable (the renderer reads the
/// refill header), preflights cached for [`PREFLIGHT_MAX_AGE`].
pub fn cors() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(|origin, _| {
            is_renderer_origin(origin.as_bytes())
        }))
        .allow_methods(Any)
        .allow_headers(Any)
        .expose_headers(Any)
        .max_age(PREFLIGHT_MAX_AGE)
}

#[cfg(test)]
#[path = "renderer_origin_test.rs"]
mod tests;
