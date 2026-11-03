//! The request for the flags file: one per launch, and in a session
//! where it fails, the occasional retry (see [`super::refresh`]). Each
//! attempt also asks for the file's detached signature beside it, which
//! decides whether the notices the file carries may be used (see
//! [`super::signature`]).
//!
//! The file lives on the repository's `flags` branch and is read from
//! `raw.githubusercontent.com`, GitHub's content CDN — not the REST
//! API, so it does not draw on the 60-requests-an-hour unauthenticated
//! budget the update check spends. GitHub publishes no figure for the
//! CDN's own limit; a request per launch, and a retry at most hourly
//! after a failure, stay far from any plausible one.

use std::time::Duration;

use super::resolve::{parse_document, Document, MAX_BYTES};
use super::signature::{admit, verify, MAX_SIGNATURE_BYTES, SIGNATURE_SUFFIX, TRUSTED_KEYS};

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
/// finish in 45 seconds is retried later instead. Of the 45, up to
/// [`CONNECT_TIMEOUT`] may go on connecting (see [`client`]).
pub const FETCH_TIMEOUT: Duration = Duration::from_secs(45);

/// The bound on DNS, the TCP connect and the TLS handshake together,
/// where the metadata client allows ten seconds. The connector splits
/// it across the addresses DNS returns (the CDN returns four, so each
/// TCP attempt gets about 7.5 s before the next address is tried),
/// which is how a lossy link's retransmitted SYNs get several seconds
/// per address rather than two and a half. Thirty leaves the rest of
/// [`FETCH_TIMEOUT`] for the request. No getter exposes a client's
/// timeouts, so tests pin the constant, not the builder call.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// The client the flags file is fetched with: its own, for the
/// connect timeout above. Falls back to the default client if the
/// builder fails, which it does not in practice.
#[must_use]
pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(FETCH_TIMEOUT)
        .build()
        .unwrap_or_default()
}

/// Fetch and parse the flags file. `None` on every failure: a
/// transport error, the timeout, a status other than 200, a body over
/// the cap, or a body that is not a flags document.
pub async fn fetch(client: &reqwest::Client, url: &str, timeout: Duration) -> Option<Document> {
    fetch_trusting(client, url, timeout, TRUSTED_KEYS).await
}

/// [`fetch`], trusting the signature keys `keys`: the file, then its
/// detached signature from the same address with
/// [`SIGNATURE_SUFFIX`] appended, both within `timeout`. The flags are
/// returned whatever the signature says; the notices block only when
/// the file verifies under one of `keys` (see [`super::signature`]). A
/// signature that cannot be fetched is no signature.
pub async fn fetch_trusting(
    client: &reqwest::Client,
    url: &str,
    timeout: Duration,
    keys: &[&str],
) -> Option<Document> {
    let both = async {
        let body = fetch_body(client, url, timeout).await?;
        let doc = parse_document(&body)?;
        let signature = fetch_body(client, &format!("{url}{SIGNATURE_SUFFIX}"), timeout)
            .await
            .filter(|s| s.len() <= MAX_SIGNATURE_BYTES)
            .and_then(|s| String::from_utf8(s).ok());
        let verdict = verify(&body, signature.as_deref(), keys);
        tracing::info!(?verdict, "flags file signature checked");
        Some(admit(doc, verdict))
    };
    tokio::time::timeout(timeout, both).await.ok()?
}

/// The body of a 200 response, read no further than one byte past the
/// cap, so an oversized body is refused without being held whole.
async fn fetch_body(client: &reqwest::Client, url: &str, timeout: Duration) -> Option<Vec<u8>> {
    // A client's own total timeout may be shorter; this request
    // carries the one it is given.
    let mut resp = client
        .get(url)
        .timeout(timeout)
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

#[cfg(test)]
#[path = "remote_signed_test.rs"]
mod signed_tests;
