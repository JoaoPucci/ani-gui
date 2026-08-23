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

/// One variant row of a master playlist: the stream's vertical
/// resolution and its URI, as the script's `anidb_m3u8` carves them
/// out of `#EXT-X-STREAM-INF` stanzas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MasterVariant {
    /// Vertical resolution from the stanza's `RESOLUTION=WxH`.
    pub height: u32,
    /// The variant's URI — the stanza's following line, as written.
    pub url: String,
}

/// The master playlist's variant rows, highest resolution first —
/// the order the script's `sort -g -r` produces. Stanzas without a
/// parseable `RESOLUTION` height, and I-frame stanzas (which carry
/// their URI inline and no playable stream), contribute nothing.
pub fn parse_master_variants(m3u8: &str) -> Vec<MasterVariant> {
    let mut out = Vec::new();
    let mut lines = m3u8.lines();
    while let Some(line) = lines.next() {
        if !line.starts_with("#EXT-X-STREAM-INF") || line.contains("I-FRAME") {
            continue;
        }
        let Some(height) = line
            .split("RESOLUTION=")
            .nth(1)
            .and_then(|rest| rest.split(',').next())
            .and_then(|res| res.split('x').nth(1))
            .and_then(|h| h.trim().parse().ok())
        else {
            continue;
        };
        let Some(url) = lines
            .by_ref()
            .find(|l| !l.starts_with('#') && !l.trim().is_empty())
        else {
            break;
        };
        out.push(MasterVariant {
            height,
            url: url.trim().to_string(),
        });
    }
    out.sort_by(|a, b| b.height.cmp(&a.height));
    out
}

/// The variant a quality setting selects, mirroring the script's
/// `select_quality` arms: `best` takes the highest, `worst` the
/// lowest, anything else the variant whose height matches the
/// setting exactly. A miss is `None` — the caller keeps the adaptive
/// master rather than guessing.
pub fn select_variant<'a>(
    variants: &'a [MasterVariant],
    quality: &str,
) -> Option<&'a MasterVariant> {
    match quality {
        "best" => variants.first(),
        "worst" => variants.last(),
        q => variants.iter().find(|v| v.height.to_string() == q),
    }
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
