//! The embed page's payload: `window.__P` carries the player's JSON,
//! XOR'd under a fixed key and base64'd. The key spells its own
//! version, so a rotation shows up here first.

use base64::Engine as _;
use serde::Deserialize;

/// The XOR key the embed pages use. Versioned in the value itself.
const EMBED_KEY: &[u8] = b"otaku-embed-v1";

/// A sidecar subtitle track the embed lists beside the stream.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SubtitleTrack {
    /// Language code (`en`).
    pub lang: String,
    /// Display label (`English`).
    pub label: String,
    /// Whether the player selects it by default.
    #[serde(default)]
    pub default: bool,
    /// The `.vtt` URL, on the same CDN as the stream.
    pub src: String,
}

/// What the embed page carries that playback needs.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct EmbedPayload {
    /// The master-playlist URL.
    pub src: String,
    /// Sidecar subtitle tracks, outside the playlist.
    #[serde(default)]
    pub subtitles: Vec<SubtitleTrack>,
}

/// Decode the payload out of an embed page. `None` for a page without
/// the marker, a blob that is not base64, or plaintext that is not the
/// payload — each is a miss, never a panic.
#[must_use]
pub fn decode_embed(html: &str) -> Option<EmbedPayload> {
    let (_, rest) = html.split_once("window.__P=\"")?;
    let blob = rest.split('"').next()?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(blob)
        .ok()?;
    let plain: Vec<u8> = bytes
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ EMBED_KEY[i % EMBED_KEY.len()])
        .collect();
    serde_json::from_slice(&plain).ok()
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
