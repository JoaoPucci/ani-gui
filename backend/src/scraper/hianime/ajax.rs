//! The JSON envelopes hianime's AJAX endpoints answer — `{"status":
//! true, "html": "…"}` — and the HTML each one wraps: the episode
//! list and the per-episode server list.

use base64::Engine as _;
use serde::Deserialize;

use crate::error::{AniError, Result};
use crate::scraper::provider::EpisodeRef;

#[derive(Deserialize)]
struct Envelope {
    status: bool,
    #[serde(default)]
    html: Option<String>,
}

/// The HTML an envelope wraps. `status: false` is the provider
/// answering not-found for the id asked about — the same shape a
/// dead slug's 404 carries elsewhere — so it surfaces as one.
///
/// # Errors
/// [`AniError::Upstream`] with 404 on `status: false`,
/// [`AniError::ParseFailed`] when the body is not the envelope.
fn unwrap_envelope(json: &str) -> Result<String> {
    let env: Envelope = serde_json::from_str(json).map_err(|e| AniError::ParseFailed {
        detail: format!("hianime envelope: {e}"),
    })?;
    if !env.status {
        return Err(AniError::Upstream { status: 404 });
    }
    env.html.ok_or_else(|| AniError::ParseFailed {
        detail: "hianime envelope without html".into(),
    })
}

/// An empty listing is the provider answering "none"; nonempty HTML
/// with no row the parser knows is the site having changed shape, and
/// reading that as "none" would let the mode probe persist an absence
/// that hides a playable show. Rows the parser recognizes but cannot
/// use are the caller's to skip.
///
/// # Errors
/// [`AniError::ParseFailed`] for nonempty HTML without `marker`.
fn recognizable(html: &str, marker: &str, what: &str) -> Result<()> {
    if html.trim().is_empty() || html.contains(marker) {
        return Ok(());
    }
    Err(AniError::ParseFailed {
        detail: format!("hianime {what} without recognizable rows"),
    })
}

/// The value of the first `name="…"` attribute in `s`.
fn attr<'a>(s: &'a str, name: &str) -> Option<&'a str> {
    let (_, rest) = s.split_once(name)?;
    rest.split('"').next()
}

/// An entry's episodes: each `ep-item` anchor's `data-number` and
/// `data-id`. Rows whose number or id does not parse are skipped —
/// the listing numbers integers per entry.
///
/// # Errors
/// As [`unwrap_envelope`].
pub fn parse_episode_list(json: &str) -> Result<Vec<EpisodeRef>> {
    let html = unwrap_envelope(json)?;
    recognizable(&html, "ep-item", "episode list")?;
    Ok(html
        .split("ep-item")
        .skip(1)
        .filter_map(|item| {
            let number = attr(item, "data-number=\"")?.trim().parse().ok()?;
            let id = attr(item, "data-id=\"")?.trim().parse().ok()?;
            Some(EpisodeRef {
                id,
                number,
                number2: None,
            })
        })
        .collect())
}

/// One playable server for an episode: its audio mode, the site's
/// name for it, and the embed URL its hash decodes to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerEmbed {
    /// `sub` or `dub`, as the site types the server.
    pub mode: String,
    /// The site's server name (`HD-1`, `HD-2`).
    pub name: String,
    /// The player page, on the embed host.
    pub embed_url: String,
}

/// An episode's servers. A server whose hash does not decode to an
/// absolute http(s) URL is skipped: nothing downstream can use it.
///
/// # Errors
/// As [`unwrap_envelope`].
pub fn parse_servers(json: &str) -> Result<Vec<ServerEmbed>> {
    let html = unwrap_envelope(json)?;
    recognizable(&html, "server-item", "server list")?;
    Ok(html
        .split("server-item")
        .skip(1)
        .filter_map(|item| {
            let mode = attr(item, "data-type=\"")?.trim().to_string();
            let name = attr(item, "data-server-name=\"")?.trim().to_string();
            let hash = attr(item, "data-hash=\"")?.trim();
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(hash)
                .ok()?;
            let embed_url = String::from_utf8(bytes).ok()?;
            let parsed = url::Url::parse(&embed_url).ok()?;
            if !matches!(parsed.scheme(), "http" | "https") {
                return None;
            }
            Some(ServerEmbed {
                mode,
                name,
                embed_url,
            })
        })
        .collect())
}

/// The server to play `mode` from: `HD-1` when the site lists it,
/// else the first of that mode — the reference scraper's preference.
#[must_use]
pub fn preferred_server<'a>(servers: &'a [ServerEmbed], mode: &str) -> Option<&'a ServerEmbed> {
    let of_mode = || servers.iter().filter(|s| s.mode == mode);
    of_mode()
        .find(|s| s.name == "HD-1")
        .or_else(|| of_mode().next())
}
