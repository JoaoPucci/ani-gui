//! Reading one search-result card — its heading anchor, slug and title,
//! with the site's entities decoded; split from [`super`] so each file
//! stays inside the CRAP gate's per-file bar.

use super::*;

/// One result card: the heading anchor's slug and title, and the
/// first plain `fdi-item` badge (the duration badge carries a second
/// class and is skipped by the exact match). Both fields come from
/// the one anchor ([`heading_anchor`]): read from anywhere in the
/// block, a heading missing one of them would be completed from a
/// later link and come back as a hit carrying another entry's slug
/// under this card's title. A slug without the decimal tail the
/// episode listing is keyed on ([`slug_id`]) is not a card the
/// client can resolve, and is a card the reader cannot read like
/// any other.
pub(super) fn parse_card(card: &str) -> Option<BrowseHit> {
    let anchor = heading_anchor(card)?;
    let href = attr(anchor, "href=\"")?;
    let slug = href
        .rsplit('/')
        .next()?
        .split('?')
        .next()?
        .trim()
        .to_string();
    if slug.is_empty() || slug_id(&slug).is_none() {
        return None;
    }
    let title = decode_entities(attr(anchor, "title=\"")?);
    // A card without a title names nothing: it would put an empty
    // title on the resolve and match no title the user typed.
    if title.trim().is_empty() {
        return None;
    }
    let kind = card
        .split("class=\"fdi-item\">")
        .nth(1)
        .and_then(|rest| rest.split('<').next())
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .map(str::to_string);
    Some(BrowseHit { slug, title, kind })
}

/// The card's heading anchor, whole — the first `<a …>` open tag in
/// the detail block, from its `<` to its `>` — so the card's href
/// and title are read from the same tag whatever order the site
/// writes them in. The detail block leads with its heading; the
/// poster link that precedes the block is not in it.
fn heading_anchor(detail: &str) -> Option<&str> {
    let mut from = 0;
    while let Some(found) = detail[from..].find("<a") {
        let at = from + found;
        let after = &detail[at + 2..];
        if after.starts_with(|c: char| c.is_ascii_whitespace() || c == '>') {
            let close = after.find('>')?;
            return Some(&detail[at..=at + 2 + close]);
        }
        from = at + 2;
    }
    None
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
