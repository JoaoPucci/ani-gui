//! Pure extraction over hianime's HTML pages — the search page and
//! the entry page. Split from the client so each file stays inside
//! the complexity ratchet's per-file bar.

use crate::error::{AniError, Result};
use crate::scraper::provider::BrowseHit;

/// The search page's result cards. The list lives inside
/// `film_list-wrap`; the top-10 widget and the sidebar carry the same
/// `film-detail` markup and are not the answer. A page that says "No
/// animes found." is the provider answering absence; a page without
/// either shape is a parse failure, never absence.
///
/// # Errors
/// [`AniError::ParseFailed`] when the page shows neither the result
/// list nor the no-results notice.
pub fn parse_search(html: &str) -> Result<Vec<BrowseHit>> {
    let Some(start) = html.find("film_list-wrap") else {
        if html.contains("No animes found") {
            return Ok(Vec::new());
        }
        return Err(AniError::ParseFailed {
            detail: "hianime search page without its result list".into(),
        });
    };
    let end = html[start..]
        .find("main-sidebar")
        .map_or(html.len(), |i| start + i);
    Ok(html[start..end]
        .split("film-detail")
        .skip(1)
        .filter_map(parse_card)
        .collect())
}

/// One result card: the title anchor's slug and title, and the first
/// plain `fdi-item` badge (the duration badge carries a second class
/// and is skipped by the exact match).
fn parse_card(card: &str) -> Option<BrowseHit> {
    let href = attr(card, "href=\"")?;
    let slug = href
        .rsplit('/')
        .next()?
        .split('?')
        .next()?
        .trim()
        .to_string();
    if slug.is_empty() {
        return None;
    }
    let title = decode_entities(attr(card, "title=\"")?);
    let kind = card
        .split("class=\"fdi-item\">")
        .nth(1)
        .and_then(|rest| rest.split('<').next())
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .map(str::to_string);
    Some(BrowseHit { slug, title, kind })
}

/// The value of the first `name="…"` attribute in `s`.
fn attr<'a>(s: &'a str, name: &str) -> Option<&'a str> {
    let (_, rest) = s.split_once(name)?;
    rest.split('"').next()
}

/// The entities the site's titles carry, `&amp;` last so it cannot
/// re-form another entity.
fn decode_entities(s: &str) -> String {
    s.replace("&#039;", "'")
        .replace("&apos;", "'")
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// The digits after a slug's last hyphen — the entry id the AJAX
/// listing is keyed on.
#[must_use]
pub fn slug_id(slug: &str) -> Option<u64> {
    slug.rsplit('-').next()?.parse().ok()
}

/// The year the entry page's `Aired:` line starts with
/// (`Apr 3, 1998 to Apr 24, 1999` → 1998). `None` when the page
/// carries no such line or the date is unannounced.
#[must_use]
pub fn parse_detail_year(html: &str) -> Option<u32> {
    let (_, after) = html.split_once("Aired:</span>")?;
    let (_, value) = after.split_once("class=\"name\">")?;
    let text = value.split('<').next()?;
    let mut digits = String::new();
    for c in text.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
            if digits.len() == 4 {
                return digits.parse().ok();
            }
        } else {
            digits.clear();
        }
    }
    None
}
