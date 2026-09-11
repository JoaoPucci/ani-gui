//! The megaplay embed host: its page carries no payload, and its
//! player asks the site for the sources by the media id the page's
//! player element carries. The answer names the master playlist and
//! the captions tracks in the clear; the CDN checks the embed host's
//! origin as `Referer` on every fetch of them, like zokoanime's.

use serde::Deserialize;

use super::embed::EmbedPayload;
use crate::error::{AniError, Result};
use crate::scraper::provider::SubtitleTrack;

/// The element whose `data-id` is the media id.
const PLAYER_ELEMENT: &str = "id=\"megaplay-player\"";

/// The endpoint the player asks for the sources, under the page's
/// own origin. The site's older `getSources` answers the sources
/// encrypted; this one answers them in the clear.
const SOURCES_PATH: &str = "/stream/getSourcesNew?id=";

/// The media id off an embed page: the `data-id` of the page's
/// player element, whatever the attribute order. `None` when the
/// page carries no such element or the element no id — a page the
/// reader does not read.
#[must_use]
pub fn media_id(html: &str) -> Option<u64> {
    let at = html.find(PLAYER_ELEMENT)?;
    let start = html[..at].rfind('<')?;
    let end = at + html[at..].find('>')?;
    let tag = &html[start..end];
    let (_, rest) = tag.split_once("data-id=\"")?;
    rest.split('"').next()?.trim().parse().ok()
}

/// The sources endpoint for `embed_url`'s origin — the site serves
/// its player from mirrors, and each answers for its own pages.
/// `None` for an embed URL that is not an absolute http(s) URL.
#[must_use]
pub fn sources_url(embed_url: &str, id: u64) -> Option<String> {
    let parsed = url::Url::parse(embed_url).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    Some(format!(
        "{}{SOURCES_PATH}{id}",
        parsed.origin().ascii_serialization()
    ))
}

/// A listed file, in the shapes the site's client reads: one object
/// with a `file`, or a list whose first entry has one.
#[derive(Deserialize)]
#[serde(untagged)]
enum WireSources {
    One(WireFile),
    Many(Vec<WireFile>),
}

#[derive(Deserialize)]
struct WireFile {
    #[serde(default)]
    file: String,
}

/// A listed track. `kind` tells captions from the player's other
/// tracks (thumbnails); the language is not listed, only the label.
#[derive(Deserialize)]
struct WireTrack {
    #[serde(default)]
    file: String,
    #[serde(default)]
    label: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    default: bool,
}

/// The wire shape of the sources response.
#[derive(Deserialize)]
struct WireResponse {
    #[serde(default)]
    sources: Option<WireSources>,
    #[serde(default)]
    tracks: Vec<WireTrack>,
}

/// The payload out of a sources response. A body that is not the
/// response, or one whose sources name no absolute http(s) URL — the
/// older endpoint's encrypted answer carries `null` in the clear —
/// is the site having changed what it hands the client, never an
/// episode without a stream. Tracks that are not captions, or whose
/// file the transport cannot fetch, are left out.
///
/// # Errors
/// [`AniError::ParseFailed`] for a body without a fetchable source.
pub fn parse_sources(json: &str) -> Result<EmbedPayload> {
    let undecodable = |what: &str| AniError::ParseFailed {
        detail: format!("megaplay sources: {what}"),
    };
    let wire: WireResponse =
        serde_json::from_str(json).map_err(|_| undecodable("body is not the response"))?;
    let src = match wire.sources {
        Some(WireSources::One(f)) => f.file,
        Some(WireSources::Many(files)) => {
            files.into_iter().next().map(|f| f.file).unwrap_or_default()
        }
        None => String::new(),
    };
    if !is_fetchable(&src) {
        return Err(undecodable("no source the transport can fetch"));
    }
    let subtitles = wire
        .tracks
        .into_iter()
        .filter(|t| is_captions(&t.kind) && is_fetchable(&t.file))
        .map(|t| SubtitleTrack {
            lang: lang_of_track(&t.file, &t.label),
            label: t.label,
            default: t.default,
            url: t.file,
        })
        .collect();
    Ok(EmbedPayload { src, subtitles })
}

/// The language a track carries: the three-letter code its file name
/// ends in (`track_0_eng.vtt`, `track_2_Latin_American_spa.vtt`) —
/// the site lists no code, only a label — or else the label itself,
/// or `und` when there is neither.
#[must_use]
pub fn lang_of_track(file: &str, label: &str) -> String {
    let name = file
        .rsplit('/')
        .next()
        .unwrap_or(file)
        .split(['?', '#'])
        .next()
        .unwrap_or_default();
    let stem = name
        .strip_suffix(".vtt")
        .or_else(|| name.strip_suffix(".VTT"))
        .unwrap_or(name);
    let code = stem.rsplit('_').next().unwrap_or_default();
    if code.len() == 3 && code.bytes().all(|b| b.is_ascii_lowercase()) {
        return code.to_string();
    }
    let label = label.trim();
    if label.is_empty() {
        "und".to_string()
    } else {
        label.to_string()
    }
}

/// Whether a listed track is a subtitle track, by the kind the
/// player types it with.
fn is_captions(kind: &str) -> bool {
    matches!(
        kind.trim().to_ascii_lowercase().as_str(),
        "captions" | "subtitles"
    )
}

/// Whether the transport can fetch `src`: an absolute URL under
/// http or https, nothing else.
fn is_fetchable(src: &str) -> bool {
    url::Url::parse(src).is_ok_and(|u| matches!(u.scheme(), "http" | "https"))
}
