//! Pure extraction over the provider's response bodies — split from
//! the client so each file stays inside the complexity ratchet's
//! per-file bar.

use super::{BrowseHit, EpisodeRef, LanguageEmbed};
use crate::error::{AniError, Result};

/// Whether a response body is cloudflare's challenge interstitial
/// rather than provider content.
pub fn is_cloudflare_interstitial(body: &str) -> bool {
    body.contains("Just a moment")
}

/// Space→`+` and nothing else, mirroring the script's `sed 's| |+|g'`.
pub fn encode_query(query: &str) -> String {
    query.replace(' ', "+")
}

/// The digits after a slug's last hyphen, when there are any.
pub(super) fn slug_numeric_id(slug: &str) -> Option<u64> {
    slug.rsplit('-').next()?.parse().ok()
}

/// Decode the three entities the provider's titles carry, `&amp;`
/// last so it cannot re-form another entity.
fn decode_entities(s: &str) -> String {
    s.replace("&#039;", "'")
        .replace("&quot;", "\"")
        .replace("&amp;", "&")
}

/// Extract browse hits from the search page HTML. Titles are
/// entity-decoded (`&#039;`, `&quot;`, `&amp;`). A page without
/// matching anchors yields an empty list — "no results" is the
/// caller's verdict, not a parse failure.
pub fn parse_browse(html: &str) -> Vec<BrowseHit> {
    let mut hits = Vec::new();
    // Anchor-scoped scan, like the script's `<a href` split: the slug
    // must come from the href and the title from the same anchor's
    // image alt, or a page-level scan pairs values across cards.
    for chunk in html.split("<a href").skip(1) {
        let Some(slug) = chunk
            .split_once("anime/")
            .map(|(_, rest)| rest)
            .and_then(|rest| rest.split('"').next())
        else {
            continue;
        };
        if slug_numeric_id(slug).is_none()
            || !slug
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            continue;
        }
        let Some(title) = chunk
            .split_once("alt=\"")
            .map(|(_, rest)| rest)
            .and_then(|rest| rest.split('"').next())
        else {
            continue;
        };
        hits.push(BrowseHit {
            slug: slug.to_string(),
            title: decode_entities(title),
        });
    }
    hits
}

/// Parse the episodes endpoint's JSON array into id/number pairs,
/// preserving order.
///
/// # Errors
/// [`AniError::ParseFailed`] when the body isn't the expected array.
pub fn parse_episodes(json: &str) -> Result<Vec<EpisodeRef>> {
    #[derive(serde::Deserialize)]
    struct Row {
        id: u64,
        number: u32,
    }
    let rows: Vec<Row> = serde_json::from_str(json).map_err(|e| AniError::ParseFailed {
        detail: format!("anidb episodes: {e}"),
    })?;
    Ok(rows
        .into_iter()
        .map(|r| EpisodeRef {
            id: r.id,
            number: r.number,
        })
        .collect())
}

/// Parse the languages endpoint's JSON array into per-language embeds.
///
/// # Errors
/// [`AniError::ParseFailed`] when the body isn't the expected array.
pub fn parse_languages(json: &str) -> Result<Vec<LanguageEmbed>> {
    #[derive(serde::Deserialize)]
    struct Row {
        language: String,
        embed_url: String,
    }
    let rows: Vec<Row> = serde_json::from_str(json).map_err(|e| AniError::ParseFailed {
        detail: format!("anidb languages: {e}"),
    })?;
    Ok(rows
        .into_iter()
        .map(|r| LanguageEmbed {
            language: r.language,
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
/// setup (`file: '…'`, first occurrence).
pub fn extract_master_url(embed_html: &str) -> Option<String> {
    let (_, rest) = embed_html.split_once("file: '")?;
    let url = rest.split('\'').next()?;
    if url.is_empty() {
        return None;
    }
    Some(url.to_string())
}
