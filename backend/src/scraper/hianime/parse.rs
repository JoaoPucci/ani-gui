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
    // The split's first piece is what precedes the first boundary:
    // the list's opening chrome, normally, and passed over as the
    // mark-less sliver it is — but a first card that lost its
    // boundary class sits there with its marks intact, and judged
    // like any fragment it is read as the card it is, or refuses the
    // listing when it cannot be read, rather than being passed over
    // for the cards after it. The boundary itself must still be
    // there: a list with none is a renamed shape, refused above.
    let mut fragments = html[start..end].split("flw-item");
    let prefix = fragments.next().unwrap_or_default();
    let fragments: Vec<&str> = fragments.collect();
    if fragments.is_empty() {
        return none_or_refused(html, "hianime search result list without card boundaries");
    }
    let hits: Vec<BrowseHit> = std::iter::once(prefix)
        .chain(fragments)
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

/// Whether a fragment of the split is one of the list's cards, and
/// so a fragment whose unreadability refuses the page. The boundary
/// is a class name rather than a tag, so the split catches more than
/// cards: a card whose class list names the boundary twice —
/// `class="flw-item flw-item-big"` — leaves the `" "` between the
/// two mentions as a fragment of its own, part of one tag and no
/// card at all, and whatever the site writes between the last card
/// and the sidebar rides along on the last card's fragment. A
/// fragment carrying any one of a card's own marks is a card; the
/// mark-less slivers are the list's furniture and are passed over.
///
/// One mark is where the line sits because a card the site rendered
/// broken still carries one. The change of shape that costs a card
/// its heading anchor can take a mark with it in the same stroke,
/// and the search shape already renders cards with no `film-poster`
/// block, so a card left with nothing but its `film-name` heading is
/// a shape the site can reach. Held to two marks that card is
/// furniture: dropped without a word, leaving a listing one card
/// short that still reads as an answer, and a pick with no episode
/// count or year to weigh takes the card after it and plays an
/// unrelated entry. A refusal costs the search; a drop costs the
/// right answer.
///
/// What the line newly refuses is chrome inside the list's own
/// bounds that names one of the three classes — a script between the
/// last card and the sidebar mentioning `film-name`, a "load more"
/// block carrying a `film-poster`. Those bounds are narrow: the
/// parse reads between `film_list-wrap` and `main-sidebar`, and the
/// two places this markup appears away from the results — the top-10
/// widget and the sidebar — are both outside them, leaving the
/// mark-less sliver as the only furniture the captured shapes put
/// inside. Such chrome would show as a page that refuses rather than
/// a page that answers wrongly. What nothing here can see is a card
/// the site did not render at all: a list one card short still reads
/// as complete, exactly as the episode listing's rows do.
fn card_shaped(fragment: &str) -> bool {
    CARD_MARKS.iter().any(|mark| fragment.contains(*mark))
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

/// The value of the first `name="…"` attribute in `s`, `name` given
/// with its `="` — read by the whole attribute name: a match is taken
/// only where the character before it cannot be part of a name, so
/// `data-title="…"` ahead of `title="…"` is stepped over rather than
/// read as the title.
fn attr<'a>(s: &'a str, name: &str) -> Option<&'a str> {
    let mut from = 0;
    while let Some(found) = s[from..].find(name) {
        let at = from + found;
        let bounded = s[..at]
            .chars()
            .next_back()
            .is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ':'));
        if bounded {
            return s[at + name.len()..].split('"').next();
        }
        from = at + 1;
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

/// The digits after a slug's last hyphen — the entry id the AJAX
/// listing is keyed on.
#[must_use]
pub fn slug_id(slug: &str) -> Option<u64> {
    slug.rsplit('-').next()?.parse().ok()
}
