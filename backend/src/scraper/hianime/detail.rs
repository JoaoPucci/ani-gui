//! Pure extraction over hianime's entry page — the premiere year the
//! resolver uses as a hint, the other names the page gives the show,
//! its genres and its age rating. Split from the search page's reading so
//! each file stays inside the complexity ratchet's per-file bar.

use crate::scraper::provider::EntryPage;

use super::markup::decode_entities;

/// The rest of a row after its label: up to the row's
/// closing tag or the next row's label, whichever comes first, so
/// the value read stays inside the row that carries the label.
fn aired_row(after: &str) -> &str {
    let end = ["</div>", "class=\"item-head\""]
        .iter()
        .filter_map(|marker| after.find(marker))
        .min()
        .unwrap_or(after.len());
    &after[..end]
}

/// The year the entry page's `Aired:` line starts with
/// (`Apr 3, 1998 to Apr 24, 1999` → 1998). `None` when the page
/// carries no such line, the date is unannounced, or the row's
/// value cannot be read: the value is looked for inside the Aired
/// row alone, never in a named element further down the page.
#[must_use]
pub fn parse_detail_year(html: &str) -> Option<u32> {
    let (_, after) = html.split_once("Aired:</span>")?;
    let (_, value) = aired_row(after).split_once("class=\"name\">")?;
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

/// The labels of the rows whose value is another name for the show:
/// its title in Japanese script, and the synonyms the site lists.
const NAME_ROWS: [&str; 2] = ["Japanese:</span>", "Synonyms:</span>"];

/// The other names the entry page gives the show — the value of its
/// Japanese row and of its Synonyms row, each entity-decoded and
/// trimmed, in that order, a row that is missing or empty giving
/// none. A synonyms row is one name as written: the site's synonyms
/// carry commas of their own ("Shingeki no Kyojin: Final Season, Part
/// 1"), so the row is never split. Each value is read inside its own
/// row, as the year is.
#[must_use]
pub fn parse_detail_names(html: &str) -> Vec<String> {
    NAME_ROWS
        .iter()
        .filter_map(|label| {
            let (_, after) = html.split_once(label)?;
            let (_, value) = aired_row(after).split_once("class=\"name\">")?;
            let text = decode_entities(value.split('<').next()?);
            let text = text.trim();
            (!text.is_empty()).then(|| text.to_string())
        })
        .collect()
}

/// The genres the entry page files the show under: the text of every
/// genre link in its Genres row, entity-decoded and trimmed, in page
/// order — read inside that row alone, up to its closing tag or the
/// next row's label, so a genre link elsewhere on the page (the
/// sidebar's genre menu, another show's card) is never the show's.
/// A page without the row lists none.
#[must_use]
pub fn parse_detail_genres(html: &str) -> Vec<String> {
    let Some((_, after)) = html.split_once("Genres:</span>") else {
        return Vec::new();
    };
    aired_row(after)
        .split("<a ")
        .skip(1)
        .filter(|link| {
            link.split('>')
                .next()
                .is_some_and(|tag| tag.contains("/genres/"))
        })
        .filter_map(|link| {
            let text = link.split_once('>')?.1.split('<').next()?;
            let text = decode_entities(text);
            let text = text.trim();
            (!text.is_empty()).then(|| text.to_string())
        })
        .collect()
}

/// The age rating the entry page gives the show: the text of the
/// rating mark (`tick-pg`) in the stats under the show's own heading,
/// entity-decoded and trimmed — "G", "PG-13", "R+", "Rx". Read inside
/// that stats block alone, which ends where the page's buttons begin,
/// so a mark on another show's card further down the page is never the
/// show's. `None` when the heading carries no such mark.
#[must_use]
pub fn parse_detail_rating(html: &str) -> Option<String> {
    let (_, heading) = html.split_once("<h2 class=\"film-name")?;
    let (_, stats) = heading.split_once("class=\"film-stats\"")?;
    let stats = stats.split("class=\"film-buttons\"").next()?;
    let (_, mark) = stats.split_once("tick-item tick-pg\"")?;
    let text = mark.split_once('>')?.1.split('<').next()?;
    let text = decode_entities(text);
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// The entry page as the picker reads it: [`parse_detail_year`],
/// [`parse_detail_names`], [`parse_detail_genres`] and
/// [`parse_detail_rating`] over the one page.
#[must_use]
pub fn parse_entry_page(html: &str) -> EntryPage {
    EntryPage {
        year: parse_detail_year(html),
        names: parse_detail_names(html),
        genres: parse_detail_genres(html),
        rating: parse_detail_rating(html),
    }
}
