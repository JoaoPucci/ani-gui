//! Parsers over the provider's API-endpoint and embed responses —
//! split from `parse` (which keeps the HTML page parsing) so each
//! file stays inside the complexity ratchet's per-file bar.

use super::{EpisodeRef, LanguageEmbed};
use crate::error::{AniError, Result};

/// Parse the episodes endpoint's response into id/number pairs,
/// preserving order. The provider wraps the list in an `episodes`
/// envelope and carries `number2`/`filler` fields alongside; only
/// id and number matter here, and unknown fields pass through
/// serde untouched.
///
/// # Errors
/// [`AniError::ParseFailed`] when the body isn't the expected shape.
pub fn parse_episodes(json: &str) -> Result<Vec<EpisodeRef>> {
    #[derive(serde::Deserialize)]
    struct Row {
        id: u64,
        number: u32,
        #[serde(default)]
        number2: Option<serde_json::Value>,
    }
    fn tag(v: serde_json::Value) -> Option<String> {
        match v {
            serde_json::Value::Null => None,
            serde_json::Value::String(s) => Some(s),
            other => Some(other.to_string()),
        }
    }
    #[derive(serde::Deserialize)]
    struct Envelope {
        episodes: Vec<Row>,
    }
    let env: Envelope = serde_json::from_str(json).map_err(|e| AniError::ParseFailed {
        detail: format!("anidb episodes: {e}"),
    })?;
    Ok(env
        .episodes
        .into_iter()
        .map(|r| EpisodeRef {
            id: r.id,
            number: r.number,
            number2: r.number2.and_then(tag),
        })
        .collect())
}

/// Parse the languages endpoint's response into per-language embeds.
/// The provider wraps the list in a `languages` envelope and names
/// the language field `code` ("jpn"/"eng"), with a display `name`
/// alongside that nothing here needs.
///
/// # Errors
/// [`AniError::ParseFailed`] when the body isn't the expected shape.
pub fn parse_languages(json: &str) -> Result<Vec<LanguageEmbed>> {
    #[derive(serde::Deserialize)]
    struct Row {
        code: String,
        embed_url: String,
    }
    #[derive(serde::Deserialize)]
    struct Envelope {
        languages: Vec<Row>,
    }
    let env: Envelope = serde_json::from_str(json).map_err(|e| AniError::ParseFailed {
        detail: format!("anidb languages: {e}"),
    })?;
    Ok(env
        .languages
        .into_iter()
        .map(|r| LanguageEmbed {
            language: r.code,
            embed_url: r.embed_url,
        })
        .collect())
}

/// The embed the given mode plays: `jpn` for sub, `eng` for dub —
/// first match wins, as in the script.
pub fn preferred_embed<'a>(embeds: &'a [LanguageEmbed], mode: &str) -> Option<&'a LanguageEmbed> {
    let lang = if mode == "dub" { "eng" } else { "jpn" };
    embeds.iter().find(|e| e.language == lang)
}

/// Pull the master-playlist URL out of an embed page's jwplayer
/// setup (`file: '…'`, first occurrence). Only an absolute http(s)
/// URL counts: a malformed value that left here as "resolved" let
/// the orchestrator record success, stamp availability, and write
/// history before its own URL parse failed — a playback error on a
/// show marked available and watched. Anything unusable is the same
/// miss as an empty value.
pub fn extract_master_url(embed_html: &str) -> Option<String> {
    let (_, rest) = embed_html.split_once("file: '")?;
    let url = rest.split('\'').next()?;
    let parsed = url::Url::parse(url).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    Some(url.to_string())
}
