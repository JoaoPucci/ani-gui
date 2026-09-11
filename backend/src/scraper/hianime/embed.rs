//! The embed page's payload: `window.__P` carries the player's JSON,
//! XOR'd under a fixed key and base64'd. The key spells its own
//! version, so a rotation shows up here first.

use base64::Engine as _;
use serde::Deserialize;

use crate::error::{AniError, Result};
use crate::scraper::provider::SubtitleTrack;

/// The XOR key the embed pages use. Versioned in the value itself.
const EMBED_KEY: &[u8] = b"otaku-embed-v1";

/// The wire shape of a listed track — `src` where the neutral type
/// says `url`.
#[derive(Deserialize)]
struct WireTrack {
    lang: String,
    label: String,
    #[serde(default)]
    default: bool,
    src: String,
}

/// The wire shape of the payload.
#[derive(Deserialize)]
struct WirePayload {
    src: String,
    #[serde(default)]
    subtitles: Vec<WireTrack>,
}

/// What the embed page carries that playback needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbedPayload {
    /// The master-playlist URL.
    pub src: String,
    /// Sidecar subtitle tracks, outside the playlist.
    pub subtitles: Vec<SubtitleTrack>,
}

/// Decode the payload out of an embed page. A page without the
/// marker carries no playlist — the answered "nothing here". A page
/// whose marker is present but whose blob the key does not open, or
/// whose plaintext is not the payload, or whose source is not an
/// absolute http(s) URL the transport can fetch, is the site having
/// changed — the versioned key rotated, the shape moved, the source
/// written some other way — and that is a parse failure, never an
/// episode without a stream.
///
/// # Errors
/// [`AniError::NoResults`] without the marker,
/// [`AniError::ParseFailed`] when the blob does not decode to a
/// payload with a fetchable source.
pub fn decode_embed(html: &str) -> Result<EmbedPayload> {
    if !html.contains("window.__P") {
        return Err(AniError::NoResults);
    }
    let undecodable = |what: &str| AniError::ParseFailed {
        detail: format!("hianime embed payload: {what}"),
    };
    // The page carries the payload; only the one assignment form the
    // parser knows is extractable from it.
    let (_, rest) = html
        .split_once("window.__P=\"")
        .ok_or_else(|| undecodable("assignment in an unsupported form"))?;
    let blob = rest
        .split('"')
        .next()
        .ok_or_else(|| undecodable("unterminated blob"))?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(blob)
        .map_err(|_| undecodable("blob is not base64"))?;
    let plain: Vec<u8> = bytes
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ EMBED_KEY[i % EMBED_KEY.len()])
        .collect();
    let wire: WirePayload =
        serde_json::from_slice(&plain).map_err(|_| undecodable("plaintext is not the payload"))?;
    if !is_fetchable(&wire.src) {
        return Err(undecodable("source is not an absolute http(s) URL"));
    }
    Ok(EmbedPayload {
        src: wire.src,
        subtitles: wire
            .subtitles
            .into_iter()
            .map(|t| SubtitleTrack {
                lang: t.lang,
                label: t.label,
                default: t.default,
                url: t.src,
            })
            .collect(),
    })
}

/// Whether the transport can fetch `src`: an absolute URL under
/// http or https, nothing else.
fn is_fetchable(src: &str) -> bool {
    url::Url::parse(src).is_ok_and(|u| matches!(u.scheme(), "http" | "https"))
}

/// The origin the CDN wants as `Referer` on every playlist fetch:
/// the embed host's, with a trailing slash as the browser sends it.
#[must_use]
pub fn embed_origin(embed_url: &str) -> Option<String> {
    let parsed = url::Url::parse(embed_url).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    Some(format!("{}/", parsed.origin().ascii_serialization()))
}
