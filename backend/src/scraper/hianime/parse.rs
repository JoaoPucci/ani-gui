//! Pure extraction over hianime's search page — the result cards and
//! the slug's id. The entry page's reading lives beside this in
//! [`super::detail`]; both are split from the client so each file
//! stays inside the complexity ratchet's per-file bar.

use crate::error::{AniError, Result};
use crate::scraper::provider::BrowseHit;

/// The search page's result cards. The list lives inside
/// `film_list-wrap`; the top-10 widget and the sidebar carry the same
/// `film-detail` markup and are not the answer. Cards are split at
/// their own boundary (`flw-item`) and only a card's detail block is
/// read: the poster link that leads a card carries an href and title
/// of its own, and a card is never read from its neighbour.
///
/// The listing is all of its cards or none of it: a card the reader
/// cannot read refuses the page, naming that card's place in the
/// list. Dropped instead, it would leave a shorter list that still
/// reads as an answer while the entry the user searched for is
/// missing from it, and the pick — with no episode count or year to
/// weigh — would take the first card that survived, resolving and
/// playing an unrelated entry rather than surfacing the drift. Which
/// fragments of the split are held to that is [`card_shaped`].
///
/// A page that says "No animes found." is the provider answering
/// absence — that notice, with no list, is how the site renders an
/// empty search — and a page without either shape is a parse
/// failure, never absence. So is a list with no card boundary inside
/// and no notice: that is what the list looks like once the boundary
/// is renamed, whether or not the cards are still there.
///
/// # Errors
/// [`AniError::ParseFailed`] when the page shows neither the result
/// list nor the no-results notice, when a list has no card boundary
/// and no notice, when a card-shaped fragment cannot be read, and
/// when the list holds no card-shaped fragment at all.
pub fn parse_search(html: &str) -> Result<Vec<BrowseHit>> {
    let Some(start) = html.find("film_list-wrap") else {
        return none_or_refused(html, "hianime search page without its result list");
    };
    let end = html[start..]
        .find("main-sidebar")
        .map_or(html.len(), |i| start + i);
    let fragments: Vec<&str> = html[start..end].split("flw-item").skip(1).collect();
    if fragments.is_empty() {
        return none_or_refused(html, "hianime search result list without card boundaries");
    }
    let hits: Vec<BrowseHit> = fragments
        .into_iter()
        .filter(|fragment| card_shaped(fragment))
        .enumerate()
        .map(|(index, card)| {
            read_card(card).ok_or_else(|| AniError::ParseFailed {
                detail: format!("hianime search: card {} cannot be read", index + 1),
            })
        })
        .collect::<Result<Vec<BrowseHit>>>()?;
    if hits.is_empty() {
        // The boundary is there and nothing behind it carries a
        // card's marks: the site has changed shape. Read as "no
        // results" that would be persisted as absence for every
        // title searched.
        return Err(AniError::ParseFailed {
            detail: "hianime search page without readable cards".into(),
        });
    }
    Ok(hits)
}

/// The marks a result card carries of its own: the poster block that
/// leads it, the detail block beside it, and the film name inside
/// that block.
const CARD_MARKS: [&str; 3] = ["film-poster", "film-detail", "film-name"];

/// How many of [`CARD_MARKS`] make a fragment one of the list's
/// cards.
const CARD_AT_LEAST: usize = 2;

/// Whether a fragment of the split is one of the list's cards, and
/// so a fragment whose unreadability refuses the page. The boundary
/// is a class name rather than a tag, so the split catches more than
/// cards: a card whose class list names the boundary twice leaves a
/// fragment between the two mentions that is part of one tag, and
/// whatever the site writes between the last card and the sidebar
/// rides along on the last card's fragment. A fragment carrying at
/// least two of a card's own marks is a card; the rest is the list's
/// furniture and is passed over.
///
/// The threshold sits between the two things that can go wrong. The
/// change of shape that costs a card its heading anchor can rename
/// one of the marks in the same stroke, so a card is not held to
/// carrying all three — held to that, the drift this rule exists to
/// catch would slip past as furniture. And the furniture carries
/// none of the three, so two is as low as the rule can go without
/// refusing pages over the list's own chrome.
///
/// Its limits are worth stating. Chrome that does carry two of the
/// marks — a script between the list and the sidebar naming the
/// site's own class names — would be held to being a card and refuse
/// the page; and a card whose own markup mentions the boundary again
/// is split in two, of which the first half may carry two marks and
/// no anchor. Neither shape appears on the pages the reader was
/// written against, and both are visible as a page that refuses
/// rather than a page that answers wrongly. What nothing here can
/// see is a card the site did not render at all: a list one card
/// short still reads as complete, exactly as the episode listing's
/// rows do.
fn card_shaped(fragment: &str) -> bool {
    CARD_MARKS
        .iter()
        .filter(|mark| fragment.contains(*mark))
        .count()
        >= CARD_AT_LEAST
}

/// One card of the list, read from its detail block
/// ([`parse_card`]) — or nothing when the fragment carries no detail
/// block, which is a card the reader cannot read like any other.
fn read_card(fragment: &str) -> Option<BrowseHit> {
    let (_, detail) = fragment.split_once("film-detail")?;
    parse_card(detail)
}

/// The empty answer when the page carries the no-results notice,
/// else the parse failure `detail` names: without the notice, a page
/// that shows no card is not the provider answering none.
fn none_or_refused(html: &str, detail: &str) -> Result<Vec<BrowseHit>> {
    if html.contains("No animes found") {
        return Ok(Vec::new());
    }
    Err(AniError::ParseFailed {
        detail: detail.into(),
    })
}

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
fn parse_card(card: &str) -> Option<BrowseHit> {
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
