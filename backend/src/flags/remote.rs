//! The request for the flags file: one per launch, and in a session
//! where it fails, the occasional retry (see [`super::refresh`]).
//!
//! The file lives on the repository's `flags` branch and is read from
//! `raw.githubusercontent.com`, GitHub's content CDN — not the REST
//! API, so it does not draw on the 60-requests-an-hour unauthenticated
//! budget the update check spends. GitHub publishes no figure for the
//! CDN's own limit; a request per launch, and a retry at most hourly
//! after a failure, stay far from any plausible one.

use std::time::Duration;

use super::resolve::{parse_document, Document, MAX_BYTES};

/// Where the flags file is read from.
pub const FLAGS_URL: &str = "https://raw.githubusercontent.com/JoaoPucci/ani-gui/flags/flags.json";

/// The whole request — connect, status and body — must finish within
/// this. The launch does not wait for it (it waits
/// [`super::store::SETTLE_CAP`]); this bounds a background attempt,
/// whose only job is to cache the file for the next launch. The file
/// is a few hundred bytes, so the time goes on DNS, the TLS handshake
/// and round trips: 45 seconds lets a link with multi-second latency
/// and some packet loss — a congested mobile connection, a satellite
/// hop — finish all of them, where a tighter bound would fail the very
/// users the background attempt is for. A connection that cannot
/// finish in 45 seconds is retried later instead. The connect step
/// alone stays bound by the client's own connect timeout (10 seconds
/// on the metadata client).
pub const FETCH_TIMEOUT: Duration = Duration::from_secs(5);

/// Fetch and parse the flags file. `None` on every failure: a
/// transport error, the timeout, a status other than 200, a body over
/// the cap, or a body that is not a flags document.
pub async fn fetch(client: &reqwest::Client, url: &str, timeout: Duration) -> Option<Document> {
    let body = tokio::time::timeout(timeout, fetch_body(client, url, timeout))
        .await
        .ok()??;
    parse_document(&body)
}

/// The body of a 200 response, read no further than one byte past the
/// cap, so an oversized body is refused without being held whole.
async fn fetch_body(client: &reqwest::Client, url: &str, _timeout: Duration) -> Option<Vec<u8>> {
    // The client's own total timeout is shorter (the metadata client
    // allows 30 s); this request carries the longer one it is given.
    let mut resp = client
        .get(url)
        .header(reqwest::header::USER_AGENT, "ani-gui")
        .send()
        .await
        .ok()?;
    if resp.status() != reqwest::StatusCode::OK {
        return None;
    }
    let mut body = Vec::new();
    while let Some(chunk) = resp.chunk().await.ok()? {
        body.extend_from_slice(&chunk);
        if body.len() > MAX_BYTES {
            return None;
        }
    }
    Some(body)
}

#[cfg(test)]
#[path = "remote_test.rs"]
mod tests;
