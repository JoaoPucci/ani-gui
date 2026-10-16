//! Pure extraction over hianime's entry page — the premiere year the
//! resolver uses as a hint, and the other names the page gives the
//! show. Split from the search page's reading so
//! each file stays inside the complexity ratchet's per-file bar.

use crate::scraper::provider::EntryPage;

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

/// The other names the entry page gives the show. Not read yet: the
/// page is reduced to its year alone.
#[must_use]
pub fn parse_detail_names(_html: &str) -> Vec<String> {
    Vec::new()
}

/// The genres the entry page files the show under. Not read yet: the
/// page is reduced to its year alone.
#[must_use]
pub fn parse_detail_genres(_html: &str) -> Vec<String> {
    Vec::new()
}

/// The entry page as the picker reads it: [`parse_detail_year`],
/// [`parse_detail_names`] and [`parse_detail_genres`] over the one
/// page.
#[must_use]
pub fn parse_entry_page(html: &str) -> EntryPage {
    EntryPage {
        year: parse_detail_year(html),
        names: parse_detail_names(html),
        genres: parse_detail_genres(html),
    }
}
