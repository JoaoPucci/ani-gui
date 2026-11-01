//! Whether a provider's title names a Kitsu entry outright — one of
//! the entry's own titles, up to case, punctuation and how the cour
//! suffix is spelled.
//!
//! The cross-cour guard reads a Kitsu entry's cour off its slug, and a
//! slug without a suffix reads as cour 1. That is the signal that
//! caught the Stone Ocean Part 2 → Part 1 pairing, and it is wrong for
//! every entry whose slug marks a later season some other way:
//! Gintama' is `gintama-2`, Gintama° `gintama-2015-tv`, Attack on
//! Titan Season 2 `shingeki-no-kyojin-2`. Kitsu titles those entries
//! "Gintama Season 2", "Gintama Season 4", "Attack on Titan Season 2"
//! — the very names the provider lists them under. A title the entry
//! itself carries is no evidence of another cour, so the guard does
//! not refuse it.
//!
//! The comparison is the whole name, not the cour number alone. Read
//! the number by itself and "Attack on Titan Season 3 Part 2" agrees
//! with the Season 2 entry (both "2"), and "Stone Ocean Part 3" with a
//! Stardust Crusaders entry Kitsu also calls "JoJo's Bizarre Adventure
//! Part 3".

/// Whether `provider_title` is one of `entry_titles`: the same words
/// once case and punctuation are folded, or the same name before a
/// trailing cour suffix with the same cour number however the suffix
/// is spelled ("Season 2", "2nd Season", "Second Season", "Part 2",
/// "Cour 2"). A title with no words names nothing.
#[must_use]
pub fn names_the_entry<'a>(
    provider_title: &str,
    entry_titles: impl IntoIterator<Item = &'a str>,
) -> bool {
    // Red stub: the comparison lands with its green commit.
    let _ = (provider_title, entry_titles.into_iter().count());
    false
}

#[cfg(test)]
#[path = "cour_same_name_test.rs"]
mod tests;
