//! Outbound `reqwest` client for the streaming proxy.
//!
//! Separate from the metadata client (Kitsu/AniList) so connection-pool
//! and timeout policy can differ — segments are large and latency-sensitive,
//! metadata calls are small and cacheable.
//!
//! The proxy never trusts the frontend's view of upstream URLs; the
//! [`StreamSession`](crate::proxy::token::StreamSession) it pulls from
//! the [`SessionTable`](crate::proxy::token::SessionTable) is the source
//! of truth for both the URL and the `Referer:` header.

use std::time::Duration;

use bytes::Bytes;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, RANGE, REFERER, USER_AGENT};
use url::Url;

use crate::error::{AniError, Result};

/// User-Agent used by every upstream fetch. Matches what `ani-cli`
/// presents so the stream CDNs see consistent traffic for one user.
pub const UA: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:109.0) Gecko/20100101 Firefox/121.0";

/// Build the proxy's outbound HTTP client with the right defaults.
///
/// # Errors
/// Returns [`AniError::Network`] if the underlying TLS stack cannot be
/// initialized (extremely rare).
pub fn build_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(UA)
        .pool_idle_timeout(Duration::from_secs(30))
        .tcp_keepalive(Duration::from_secs(60))
        .timeout(Duration::from_secs(120))
        .gzip(true)
        .build()
        .map_err(|_| AniError::Network)
}

/// Build the metadata HTTP client (Kitsu, AniList, provider search,
/// images, GitHub polls). A stalled metadata connection must fail a
/// probe in seconds, not ride [`build_client`]'s streaming-sized
/// 120s ceiling. Same UA so CDN HEAD probes keep their accepted
/// fingerprint. Falls back to the default client if the builder
/// fails (never observed in practice).
#[must_use]
pub fn build_meta_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(UA)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .gzip(true)
        .build()
        .unwrap_or_default()
}

/// Fetch a manifest (HTTP body) from upstream with the right `Referer:`.
/// Used for master.m3u8 + media .m3u8 + .vtt.
///
/// The most a subtitle body may weigh, in bytes. A WebVTT file for
/// a feature-length film runs to a few hundred kilobytes; four
/// megabytes leaves room for one carrying styling and positioning
/// on every cue, and refuses the video a malformed track URL can
/// point at before it is read into memory.
pub const SUBTITLE_BODY_CAP: usize = 4 * 1024 * 1024;

/// The most tracks a listing may carry into the app. An episode's
/// subtitles are a handful of languages — the listings seen so far
/// carry one — so sixteen leaves room for every language a site is
/// likely to offer and refuses the listing a malformed or hostile
/// payload could pad to hundreds, each track a request and a body
/// of its own. The tracks past the cap are dropped in listing order,
/// so a listing that is merely long keeps its first ones.
pub const SUBTITLE_TRACK_CAP: usize = 16;

/// The tracks of a listing the app accepts: the first
/// [`SUBTITLE_TRACK_CAP`] of them, and how many were left behind.
#[must_use]
pub fn within_track_cap<T>(tracks: &[T]) -> (&[T], usize) {
    let kept = tracks.len().min(SUBTITLE_TRACK_CAP);
    (&tracks[..kept], tracks.len() - kept)
}

/// A body read under a cap: the whole of it, or the finding that it
/// is larger than the cap allows.
#[derive(Debug)]
pub enum CappedBody {
    /// The body, no larger than the cap.
    Whole(Bytes),
    /// The body proved larger than the cap — by its declared length
    /// ahead of any read, or by the bytes as they arrived — and was
    /// not read past it.
    Oversized,
}

/// Read a response's body up to `cap` bytes, streaming. A declared
/// `Content-Length` over the cap is refused before a byte is read;
/// otherwise the chunks are accumulated and the read stops the
/// moment they would exceed the cap, so no more than the cap is ever
/// held. The relay and the download's sidecar writer share this so
/// neither can drift into holding an unbounded body.
///
/// # Errors
/// The transport's own, when the body cannot be read.
pub async fn read_body_capped(
    mut resp: reqwest::Response,
    cap: usize,
) -> reqwest::Result<CappedBody> {
    if resp
        .content_length()
        .is_some_and(|declared| declared > cap as u64)
    {
        return Ok(CappedBody::Oversized);
    }
    let mut body = bytes::BytesMut::new();
    while let Some(chunk) = resp.chunk().await? {
        if body.len() + chunk.len() > cap {
            return Ok(CappedBody::Oversized);
        }
        body.extend_from_slice(&chunk);
    }
    Ok(CappedBody::Whole(body.freeze()))
}

/// Fetch a subtitle track with `referer`, reading its body only up
/// to [`SUBTITLE_BODY_CAP`]: the whole body, or the finding that it
/// is larger than a subtitle file can be.
///
/// # Errors
/// - [`AniError::Network`] for connection, DNS or body-read failures
/// - [`AniError::Upstream`] when the response status is not 2xx
pub async fn fetch_subtitle(
    client: &reqwest::Client,
    url: &Url,
    referer: &str,
) -> Result<CappedBody> {
    let mut headers = HeaderMap::new();
    if let Ok(v) = HeaderValue::from_str(referer) {
        headers.insert(REFERER, v);
    }
    headers.insert(USER_AGENT, HeaderValue::from_static(UA));
    let resp = client
        .get(url.as_str())
        .headers(headers)
        .send()
        .await
        .map_err(|_| AniError::Network)?;
    let status = resp.status();
    if !status.is_success() {
        return Err(AniError::Upstream {
            status: status.as_u16(),
        });
    }
    read_body_capped(resp, SUBTITLE_BODY_CAP)
        .await
        .map_err(|_| AniError::Network)
}

/// Returns the raw bytes plus the response's `Content-Type` so the proxy
/// can echo it back to the player.
///
/// # Errors
/// - [`AniError::Network`] for connection or DNS failures
/// - [`AniError::Upstream`] when the response status is not 2xx
pub async fn fetch_text(
    client: &reqwest::Client,
    url: &Url,
    referer: &str,
) -> Result<(Bytes, Option<String>)> {
    let mut headers = HeaderMap::new();
    if let Ok(v) = HeaderValue::from_str(referer) {
        headers.insert(REFERER, v);
    }
    headers.insert(USER_AGENT, HeaderValue::from_static(UA));

    let resp = client
        .get(url.as_str())
        .headers(headers)
        .send()
        .await
        .map_err(|_| AniError::Network)?;
    let status = resp.status();
    if !status.is_success() {
        return Err(AniError::Upstream {
            status: status.as_u16(),
        });
    }
    let content_type = resp
        .headers()
        .get(HeaderName::from_static("content-type"))
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let bytes = resp.bytes().await.map_err(|_| AniError::Network)?;
    Ok((bytes, content_type))
}

/// HEAD an upstream URL and decide whether the body it would return is
/// an HLS manifest or an MP4 byte stream. Used as a fallback when the
/// URL path has no recognisable extension (some embed hosts hand back
/// opaque paths like `…/videos/<id>/sub/1`).
///
/// Rule: any `content-type` mentioning "mpegurl" or "m3u8" is HLS.
/// Everything else (incl. `application/octet-stream`, `video/mp4`,
/// missing content-type) is MP4. HLS manifests always advertise
/// themselves; MP4 streams sometimes don't, so the fall-through goes
/// to MP4 to avoid trying to parse a multi-hundred-MB binary as text.
///
/// # Errors
/// - [`AniError::Network`] for connection or DNS failures
/// - [`AniError::Upstream`] when the HEAD response is non-2xx
pub async fn classify_via_head(
    client: &reqwest::Client,
    url: &Url,
    referer: &str,
) -> Result<crate::proxy::token::MediaKind> {
    use crate::proxy::token::MediaKind;

    let mut headers = HeaderMap::new();
    if let Ok(v) = HeaderValue::from_str(referer) {
        headers.insert(REFERER, v);
    }
    headers.insert(USER_AGENT, HeaderValue::from_static(UA));

    let resp = client
        .head(url.as_str())
        .headers(headers)
        .send()
        .await
        .map_err(|_| AniError::Network)?;
    let status = resp.status();
    if !status.is_success() {
        return Err(AniError::Upstream {
            status: status.as_u16(),
        });
    }
    let ct = resp
        .headers()
        .get(HeaderName::from_static("content-type"))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    let is_hls = ct.contains("mpegurl") || ct.contains("m3u8");
    Ok(if is_hls {
        MediaKind::Hls
    } else {
        MediaKind::Mp4
    })
}

/// Issue a GET to upstream and return the [`reqwest::Response`] without
/// buffering its body. The proxy's MP4 pass-through hands the response
/// stream straight to axum, so a 600 MB MP4 doesn't get materialised in
/// memory the way [`fetch_text`] would.
///
/// Logs the (status, content-type, content-length, range) at info on
/// success and at warn when the upstream rejects us — playback errors
/// in the renderer are otherwise opaque, and intermittent CDN refusals
/// (cf-cache-miss / Referer enforcement / per-IP rate-limit) only
/// reveal themselves through this log.
///
/// Forwards the inbound `Range` header (when present) so byte-range
/// requests reach upstream verbatim — the renderer's `<video>` element
/// uses that to seek without downloading the whole file.
///
/// 2xx **and** 206 are returned to the caller; only non-success
/// responses outside of those ranges are treated as errors. (3xx
/// redirects are followed transparently by `reqwest`.)
///
/// # Errors
/// - [`AniError::Network`] for connection or DNS failures
/// - [`AniError::Upstream`] when the response status is not 2xx
pub async fn fetch_streaming(
    client: &reqwest::Client,
    url: &Url,
    referer: &str,
    range: Option<&str>,
) -> Result<reqwest::Response> {
    let mut headers = HeaderMap::new();
    if let Ok(v) = HeaderValue::from_str(referer) {
        headers.insert(REFERER, v);
    }
    headers.insert(USER_AGENT, HeaderValue::from_static(UA));
    if let Some(r) = range {
        if let Ok(v) = HeaderValue::from_str(r) {
            headers.insert(RANGE, v);
        }
    }

    let resp = client
        .get(url.as_str())
        .headers(headers)
        .send()
        .await
        .map_err(|e| {
            tracing::warn!(
                upstream = url.as_str(),
                referer = referer,
                range = range.unwrap_or(""),
                error = %e,
                "fetch_streaming: network failure",
            );
            AniError::Network
        })?;
    let status = resp.status();
    if !status.is_success() {
        let server = resp
            .headers()
            .get("server")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        let cf_ray = resp
            .headers()
            .get("cf-ray")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        tracing::warn!(
            upstream = url.as_str(),
            referer = referer,
            range = range.unwrap_or(""),
            status = status.as_u16(),
            server = server,
            cf_ray = cf_ray,
            "fetch_streaming: upstream rejected request",
        );
        return Err(AniError::Upstream {
            status: status.as_u16(),
        });
    }
    tracing::debug!(
        upstream = url.as_str(),
        status = status.as_u16(),
        range = range.unwrap_or(""),
        "fetch_streaming: upstream ok",
    );
    Ok(resp)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A server that answers one request with a chunked body and no
    /// Content-Length, so the cap has to be enforced on the bytes as
    /// they arrive rather than on a header.
    async fn chunked_server(chunks: Vec<Vec<u8>>) -> String {
        use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(async move {
            let (mut sock, _) = listener.accept().await.expect("accept");
            let mut sink = [0u8; 4096];
            let _ = sock.read(&mut sink).await;
            let mut out =
                b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nContent-Type: text/vtt\r\n\r\n"
                    .to_vec();
            for chunk in chunks {
                out.extend_from_slice(format!("{:x}\r\n", chunk.len()).as_bytes());
                out.extend_from_slice(&chunk);
                out.extend_from_slice(b"\r\n");
            }
            out.extend_from_slice(b"0\r\n\r\n");
            let _ = sock.write_all(&out).await;
            let _ = sock.shutdown().await;
        });
        format!("http://{addr}/track.vtt")
    }

    /// The cap is enforced on the bytes as they stream: a body that
    /// proves larger than the cap is refused once it does, and one
    /// that fits arrives whole.
    #[tokio::test]
    async fn a_streamed_body_is_cut_at_the_cap_and_a_fitting_one_arrives_whole() {
        let client = reqwest::Client::new();
        let chunks = || vec![b"WEBVTT\n\n".to_vec(), vec![b'a'; 20], vec![b'b'; 20]];
        let url = chunked_server(chunks()).await;
        let resp = client.get(&url).send().await.expect("response");
        assert!(
            resp.content_length().is_none(),
            "the server sends no length"
        );
        assert!(matches!(
            read_body_capped(resp, 16).await.expect("read"),
            CappedBody::Oversized
        ));
        let url = chunked_server(chunks()).await;
        let resp = client.get(&url).send().await.expect("response");
        match read_body_capped(resp, 64).await.expect("read") {
            CappedBody::Whole(body) => assert_eq!(body.len(), 48),
            CappedBody::Oversized => panic!("a body under the cap arrives whole"),
        }
    }

    /// A Content-Length over the cap refuses the body before a byte
    /// of it is read.
    #[tokio::test]
    async fn a_declared_length_over_the_cap_is_refused_unread() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/big.vtt"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_bytes(vec![b'x'; 100]))
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        let resp = client
            .get(format!("{}/big.vtt", server.uri()))
            .send()
            .await
            .expect("response");
        assert_eq!(resp.content_length(), Some(100));
        assert!(matches!(
            read_body_capped(resp, 99).await.expect("read"),
            CappedBody::Oversized
        ));
    }
    use crate::proxy::token::MediaKind;

    #[test]
    fn build_client_succeeds() {
        let _c = build_client().expect("client builds");
    }

    #[test]
    fn build_meta_client_succeeds() {
        // Covers the tight-timeout builder; the fallback arm is the
        // never-observed builder failure.
        let _c = build_meta_client();
    }

    /// classify_via_head() resolves the media kind for upstreams whose
    /// URL path doesn't carry a `.m3u8` / `.mp4` extension (some
    /// providers — fast4speed.rsvp for one — hand back opaque paths
    /// like `…/videos/<id>/sub/1`). We HEAD the URL with the right
    /// Referer; the response's `content-type` is enough to route the
    /// session through the manifest-rewrite or byte-stream path.
    #[tokio::test]
    async fn head_classify_returns_hls_for_mpegurl_content_type() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("HEAD"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .insert_header("content-type", "application/vnd.apple.mpegurl"),
            )
            .mount(&server)
            .await;
        let client = build_client().unwrap();
        let url = Url::parse(&format!("{}/abc/sub/1", server.uri())).unwrap();
        let kind = classify_via_head(&client, &url, "https://allmanga.to")
            .await
            .unwrap();
        assert_eq!(kind, MediaKind::Hls);
    }

    #[tokio::test]
    async fn head_classify_returns_mp4_for_video_mp4_content_type() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("HEAD"))
            .respond_with(
                wiremock::ResponseTemplate::new(200).insert_header("content-type", "video/mp4"),
            )
            .mount(&server)
            .await;
        let client = build_client().unwrap();
        let url = Url::parse(&format!("{}/x", server.uri())).unwrap();
        let kind = classify_via_head(&client, &url, "https://allmanga.to")
            .await
            .unwrap();
        assert_eq!(kind, MediaKind::Mp4);
    }

    /// Real-world case: tools.fast4speed.rsvp returns
    /// `content-type: application/octet-stream` with a multi-hundred-MB
    /// content-length for episodes encoded as raw MP4. Treating the
    /// fall-through as MP4 is the right default in this codebase —
    /// HLS manifests are tiny + always advertised, so no MP4 false
    /// positive should also be served as HLS.
    #[tokio::test]
    async fn head_classify_falls_back_to_mp4_for_octet_stream() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("HEAD"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .insert_header("content-type", "application/octet-stream")
                    .insert_header("content-length", "290000000"),
            )
            .mount(&server)
            .await;
        let client = build_client().unwrap();
        let url = Url::parse(&format!("{}/videos/x/sub/1", server.uri())).unwrap();
        let kind = classify_via_head(&client, &url, "https://allmanga.to")
            .await
            .unwrap();
        assert_eq!(kind, MediaKind::Mp4);
    }

    /// `application/x-mpegURL` is the older Apple HLS content type;
    /// it still appears in the wild on hianime/megacloud manifests.
    #[tokio::test]
    async fn head_classify_returns_hls_for_x_mpegurl_content_type() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("HEAD"))
            .respond_with(
                wiremock::ResponseTemplate::new(200)
                    .insert_header("content-type", "application/x-mpegURL"),
            )
            .mount(&server)
            .await;
        let client = build_client().unwrap();
        let url = Url::parse(&format!("{}/playlist", server.uri())).unwrap();
        let kind = classify_via_head(&client, &url, "https://allmanga.to")
            .await
            .unwrap();
        assert_eq!(kind, MediaKind::Hls);
    }

    #[tokio::test]
    async fn fetch_text_passes_referer_to_upstream() {
        // The Mock matcher requires the inbound Referer to match before
        // it will respond — so a successful 200 response *is* the proof
        // that the right Referer was sent. wiremock's body setter
        // overrides our explicit content-type, so we don't assert on it.
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/master.m3u8"))
            .and(wiremock::matchers::header("referer", "https://allmanga.to"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_string("#EXTM3U\n"))
            .mount(&server)
            .await;

        let client = build_client().unwrap();
        let url = Url::parse(&format!("{}/master.m3u8", server.uri())).unwrap();
        let (body, _ct) = fetch_text(&client, &url, "https://allmanga.to")
            .await
            .unwrap();
        assert_eq!(&body[..], b"#EXTM3U\n");
    }

    #[tokio::test]
    async fn fetch_text_with_wrong_referer_yields_upstream_error() {
        // If the test sends a different Referer, wiremock's matcher fails
        // and the default response is 404 — proving that the Referer is
        // actually checked against the matcher.
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::header("referer", "https://allmanga.to"))
            .respond_with(wiremock::ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let client = build_client().unwrap();
        let url = Url::parse(&format!("{}/anything", server.uri())).unwrap();
        let err = fetch_text(&client, &url, "https://wrong.example")
            .await
            .unwrap_err();
        assert!(matches!(err, AniError::Upstream { status: 404 }));
    }

    #[tokio::test]
    async fn fetch_text_returns_upstream_status_on_4xx() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(wiremock::ResponseTemplate::new(403))
            .mount(&server)
            .await;

        let client = build_client().unwrap();
        let url = Url::parse(&format!("{}/x", server.uri())).unwrap();
        let err = fetch_text(&client, &url, "https://allmanga.to")
            .await
            .unwrap_err();
        match err {
            AniError::Upstream { status } => assert_eq!(status, 403),
            other => panic!("expected Upstream {{status:403}}, got {other:?}"),
        }
    }
}
