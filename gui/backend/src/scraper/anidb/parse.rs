//! Pure extraction over the provider's response bodies — split from
//! the client so each file stays inside the complexity ratchet's
//! per-file bar.

use super::BrowseHit;
use crate::error::{AniError, Result};

/// Whether a response body is cloudflare's challenge interstitial
/// rather than provider content. Case-insensitive, like the script's
/// `grep -qi`: challenge pages have varied the title's spelling.
pub fn is_cloudflare_interstitial(body: &str) -> bool {
    body.to_ascii_lowercase().contains("just a moment")
}

/// Form-urlencode a search query: space→`+`, reserved and non-ASCII
/// bytes percent-encoded. The script's naive space swap sent `;` and
/// friends raw and the provider answers those with a 400.
pub fn encode_query(query: &str) -> String {
    url::form_urlencoded::byte_serialize(query.as_bytes()).collect()
}

/// The digits after a slug's last hyphen, when there are any.
pub(super) fn slug_numeric_id(slug: &str) -> Option<u64> {
    slug.rsplit('-').next()?.parse().ok()
}

/// The Kitsu-searchable text an anidb slug carries: its hyphenated
/// words with the trailing numeric id removed (`one-piece-69` →
/// `one piece`). `None` when `slug` isn't slug-shaped — legacy
/// allanime ids are mixed-case and hyphenless, so they fall through
/// to their own resolve path.
pub fn slug_search_term(slug: &str) -> Option<String> {
    slug_numeric_id(slug)?;
    let (words, _id) = slug.rsplit_once('-')?;
    if words.is_empty() {
        return None;
    }
    Some(words.replace('-', " "))
}

/// Decode the three entities the provider's titles carry, `&amp;`
/// last so it cannot re-form another entity.
fn decode_entities(s: &str) -> String {
    s.replace("&#039;", "'")
        .replace("&quot;", "\"")
        .replace("&amp;", "&")
}

/// The year the detail page's premiere-season link names
/// (`/browse?season=fall&year=1999` → 1999). `None` when the page
/// carries no season link.
pub fn parse_detail_year(html: &str) -> Option<u32> {
    let (_, after) = html.split_once("browse?season=")?;
    let (_, after_year) = after.split_once("year=")?;
    let digits: String = after_year
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

/// Whether zero-hit browse HTML actually shows the browse page —
/// its results grid or its no-results copy — rather than an
/// unrecognized maintenance/WAF body. The fixtures this pins are
/// synthesized from the ani-cli 5.0 pipeline's shapes (the live
/// page refuses uninstrumented capture), so the accepted set is
/// deliberately narrow and the unrecognized direction fails loud:
/// a marker drift breaks no-result searches visibly and
/// transiently, while a page wrongly read as absence is cached for
/// the negative TTL.
fn shows_browse_shape(html: &str) -> bool {
    let lower = html.to_ascii_lowercase();
    lower.contains("no results") || lower.contains("class=\"grid")
}

/// Extract browse hits from the search page HTML. Titles are
/// entity-decoded (`&#039;`, `&quot;`, `&amp;`). A page without
/// matching anchors yields an empty list — "no results" is the
/// caller's verdict — but only when the page shows the browse
/// shape ([`shows_browse_shape`]).
///
/// # Errors
/// [`AniError::ParseFailed`] on zero-hit HTML that shows neither
/// the results grid nor the no-results copy: a maintenance or WAF
/// body must never read as absence, because the walk persists a
/// clean miss as a negative availability row.
pub fn parse_browse(html: &str) -> Result<Vec<BrowseHit>> {
    let mut hits = Vec::new();
    // Anchor-scoped scan, like the script's `<a href` split: the slug
    // must come from the href and the title from the same anchor's
    // image alt, or a page-level scan pairs values across cards.
    for chunk in html.split("<a href").skip(1) {
        // Bounded at the card's closing anchor: markup between </a>
        // and the next anchor belongs to no card, and reading it
        // would title an altless card with a stray image or hand it a
        // stray badge's kind. A chunk without </a> is scanned whole,
        // as before.
        let chunk = chunk.split("</a>").next().unwrap_or(chunk);
        // The chunk begins right after `<a href`, so the first quoted
        // value IS the href. The slug comes from it alone: an anime/
        // path in nested markup — an image src, say — is not where
        // the anchor points.
        let Some(href) = chunk
            .split_once('"')
            .map(|(_, rest)| rest)
            .and_then(|rest| rest.split('"').next())
        else {
            continue;
        };
        let Some(slug) = href.split_once("anime/").map(|(_, rest)| rest) else {
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
        let kind = chunk
            .split("class=\"badge badge-")
            .skip(1)
            .filter_map(|b| b.split_once('>').map(|(_, rest)| rest))
            .filter_map(|rest| rest.split('<').next())
            .map(str::trim)
            .find(|t| !t.is_empty())
            .map(str::to_string);
        hits.push(BrowseHit {
            kind,
            slug: slug.to_string(),
            title: decode_entities(title),
        });
    }
    if hits.is_empty() && !shows_browse_shape(html) {
        return Err(AniError::ParseFailed {
            detail: "anidb browse: zero hits in an unrecognized page shape".into(),
        });
    }
    Ok(hits)
}

#[cfg(test)]
#[path = "parse_prop_test.rs"]
mod parse_prop_tests;
