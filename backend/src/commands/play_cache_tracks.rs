//! The readers a cached row's liveness check is made of, beside
//! [`super::play_cache`], which composes them: the stream's HEAD
//! ping, the WebVTT prefix a track's first bytes must carry, and the
//! track's GET read the way the relay reads it. Split out so the
//! composing module stays under the CRAP ratchet's high-risk bar;
//! no behaviour change.

/// HEAD-validate that `url` is still alive, with the supplied
/// `referer` (empty string means "no Referer header"). 2xx and 3xx
/// (CDN edge redirects) both count as live; everything else,
/// including network errors, is dead.
pub(crate) async fn upstream_head_ok(
    client: &reqwest::Client,
    url: &url::Url,
    referer: &str,
) -> bool {
    let mut req = client.head(url.as_str());
    if !referer.is_empty() {
        req = req.header(reqwest::header::REFERER, referer);
    }
    let Ok(resp) = req.send().await else {
        return false;
    };
    resp.status().is_success() || resp.status().is_redirection()
}

/// What the first bytes of a body say about it being a WebVTT
/// track: `Some(true)` once they carry the signature — behind a
/// UTF-8 byte-order mark or not, as [`crate::proxy::is_webvtt`]
/// allows — `Some(false)` once they cannot, and `None` while too few
/// have arrived to tell either way. A decided prefix agrees with the
/// whole body's verdict.
#[must_use]
pub(crate) fn webvtt_prefix(bytes: &[u8]) -> Option<bool> {
    const BOM: &[u8] = b"\xEF\xBB\xBF";
    const SIGNATURE: &[u8] = b"WEBVTT";
    if BOM.starts_with(bytes) {
        // Empty, or still inside what may become a byte-order mark.
        return None;
    }
    let body = bytes.strip_prefix(BOM).unwrap_or(bytes);
    if body.len() >= SIGNATURE.len() {
        Some(body.starts_with(SIGNATURE))
    } else if SIGNATURE.starts_with(body) {
        None
    } else {
        Some(false)
    }
}

/// Whether a cached sidecar track is still a track: a GET with the
/// row's `referer` — what the relay sends when the player asks for
/// it — answers 2xx and its first bytes carry the WebVTT signature.
/// A HEAD is not enough: a CDN can answer one with 200 and serve a
/// challenge page to the GET, which the relay then refuses, and a
/// track that fails to load never reaches the player's recovery
/// path. The body is read only until its first bytes decide — a
/// chunk or two — and the response is dropped there, so the check
/// costs a request per track, not a track's worth of bytes.
pub(crate) async fn cached_track_ok(
    client: &reqwest::Client,
    url: &url::Url,
    referer: &str,
) -> bool {
    let mut req = client.get(url.as_str());
    if !referer.is_empty() {
        req = req.header(reqwest::header::REFERER, referer);
    }
    let Ok(mut resp) = req.send().await else {
        return false;
    };
    if !resp.status().is_success() {
        return false;
    }
    let mut head: Vec<u8> = Vec::new();
    loop {
        if let Some(verdict) = webvtt_prefix(&head) {
            return verdict;
        }
        match resp.chunk().await {
            Ok(Some(chunk)) => head.extend_from_slice(&chunk),
            // The body ended, or failed, before its first bytes
            // could say it is a track.
            Ok(None) | Err(_) => return false,
        }
    }
}
