//! The one request per launch for the flags file.
//!
//! The file lives on the repository's `flags` branch and is read from
//! `raw.githubusercontent.com`, GitHub's content CDN — not the REST
//! API, so it does not draw on the 60-requests-an-hour unauthenticated
//! budget the update check spends. GitHub publishes no figure for the
//! CDN's own limit; one request per launch stays far from any
//! plausible one.

use std::time::Duration;

use super::resolve::{parse_document, Document, MAX_BYTES};

/// Where the flags file is read from.
pub const FLAGS_URL: &str = "https://raw.githubusercontent.com/JoaoPucci/ani-gui/flags/flags.json";

/// The whole request — connect, status and body — must finish within
/// this, or the launch goes on without the file.
pub const FETCH_TIMEOUT: Duration = Duration::from_secs(5);

/// Fetch and parse the flags file. `None` on every failure: a
/// transport error, the timeout, a status other than 200, a body over
/// the cap, or a body that is not a flags document.
pub async fn fetch(client: &reqwest::Client, url: &str, timeout: Duration) -> Option<Document> {
    let body = tokio::time::timeout(timeout, fetch_body(client, url))
        .await
        .ok()??;
    parse_document(&body)
}

/// The body of a 200 response, read no further than one byte past the
/// cap, so an oversized body is refused without being held whole.
async fn fetch_body(client: &reqwest::Client, url: &str) -> Option<Vec<u8>> {
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
