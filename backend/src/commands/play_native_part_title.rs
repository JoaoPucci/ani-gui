//! Which part of a show a provider title names — "2nd Stage",
//! "Part 3", "Season 2" after a shared stem. Split from
//! `play_native_split` for the per-file complexity bar. The marker
//! grammar it reads is the one the title rules read
//! (`play_native_title_grammar`).

use super::play_native_title_grammar::{normalized, sole_ordinal};

/// Which part of `stem` a title names: 1 for the stem itself, `k` for
/// the stem followed by a short `k`-th part marker ("2nd Stage",
/// "Part 3", "Season 2"), `None` for anything else.
#[must_use]
pub(crate) fn part_ordinal(stem: &str, title: &str) -> Option<u32> {
    let (stem, title) = (normalized(stem), normalized(title));
    if title == stem {
        return Some(1);
    }
    let rest = title.strip_prefix(&stem)?;
    // "The Showdown" is not a part of "The Show" — but Japanese
    // writes its 第 straight after the title.
    if rest
        .chars()
        .next()
        .is_some_and(|c| c.is_alphanumeric() && c != '第')
    {
        return None;
    }
    let rest = rest.trim_start_matches(|c: char| !c.is_alphanumeric());
    sole_ordinal(rest).filter(|k| *k >= 2)
}

/// Whether a provider title is a part that comes before the requested
/// entry: some title the entry goes by (`entry_titles`, the canonical
/// one and every alias) names a later part of it. Kitsu keeps "X
/// Season 2" as an entry of its own; asked for it, the provider's "X"
/// is the season before — never the entry, and never the first half
/// of it.
#[must_use]
pub(crate) fn precedes_entry(title: &str, entry_titles: &[&str]) -> bool {
    entry_titles
        .iter()
        .any(|t| part_ordinal(title, t).is_some_and(|k| k >= 2))
}

#[cfg(test)]
#[path = "play_native_part_title_test.rs"]
mod tests;
