//! Pure extraction over hianime's entry page — the premiere year the
//! resolver uses as a hint. Split from the search page's reading so
//! each file stays inside the complexity ratchet's per-file bar.

/// The rest of the Aired row after its label: up to the row's
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
