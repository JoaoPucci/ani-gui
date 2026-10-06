//! The entry's own titles, as the picker reads them — split from
//! `play_native` for the per-file complexity bar.
//!
//! A pick is made for one anime-database entry, under one search
//! term at a time; the term is only one of the entry's names. The
//! picker carries all of them, canonical first, so a rule about what
//! the entry is called can read every name the entry goes by rather
//! than the alias that happened to be searched.

use std::collections::BTreeSet;

pub(crate) use super::play_native_title_grammar::stem;
use super::play_native_title_grammar::{
    later_divisions, named_ordinals, names_past_stem, part_ordinals, reading, trailing_markers,
};
#[cfg(test)]
use super::play_native_title_grammar::{Kind, Marker};

/// Every title the entry goes by: its canonical title, then the
/// fallbacks the walk searches in order.
#[derive(Debug, Clone, Copy)]
pub(crate) struct EntryTitles<'a> {
    /// The canonical title, searched first.
    canonical: &'a str,
    /// The fallback titles.
    alts: &'a [&'a str],
}

impl<'a> EntryTitles<'a> {
    /// The entry under `titles`, canonical first — the slice the walk
    /// hands the pick. An empty slice is an entry with no names.
    #[must_use]
    pub(crate) fn new(titles: &'a [&'a str]) -> Self {
        match titles.split_first() {
            Some((canonical, alts)) => Self { canonical, alts },
            None => Self::bare(""),
        }
    }

    /// An entry known by one title alone.
    #[must_use]
    pub(crate) fn bare(title: &'a str) -> Self {
        Self {
            canonical: title,
            alts: &[],
        }
    }
}

impl EntryTitles<'_> {
    /// Every title, canonical first.
    fn all(&self) -> impl Iterator<Item = &str> + '_ {
        std::iter::once(self.canonical).chain(self.alts.iter().copied())
    }

    /// Whether a candidate titled `candidate` may be this entry, by
    /// the season and part markers the two carry.
    ///
    /// - A season or part the candidate's title ends on must be one
    ///   some title of the entry names — "My Star: Season 2" is not
    ///   `[Oshi no Ko]`, whose titles name no second anything. Which
    ///   kind names it does not matter: the catalogues disagree on
    ///   it, and "86 Part 2" is listed as "Eighty Six: 2nd Season".
    ///   An ordinal of 1 never disqualifies ("Final Season, Part 1"),
    ///   nor does a span that includes 1 ("(Part 1+2)"); any other span
    ///   must have every ordinal named ("Part 2+3"). Zero is an
    ///   ordinal like any other: "Season 0" is admitted only where the
    ///   entry names 0 ("Jujutsu Kaisen 0").
    ///
    /// Only markers that end a title are read as the title's own, so
    /// "JoJo's Bizarre Adventure Part 4: Diamond is Unbreakable" names
    /// a story part, not a cour.
    pub(crate) fn admits(&self, candidate: &str) -> bool {
        let named: BTreeSet<u32> = self.all().flat_map(named_ordinals).collect();
        trailing_markers(candidate)
            .iter()
            .all(|m| m.named_by(&named))
    }

    /// Whether the part a candidate's title ends on agrees with the
    /// part the entry's titles end on — no part marker reading as the
    /// first part. Two same-year, same-length cours ("2nd Season" and
    /// "2nd Season Part 2") are told apart by nothing else.
    pub(crate) fn part_agrees(&self, candidate: &str) -> bool {
        let mut entry_parts: BTreeSet<u32> = self.all().flat_map(part_ordinals).collect();
        if entry_parts.is_empty() {
            entry_parts.insert(1);
        }
        let parts = part_ordinals(candidate);
        if parts.is_empty() {
            entry_parts.contains(&1)
        } else if !parts.contains(&1) {
            // A span without the first part agrees only where the
            // entry ends on every part in it.
            parts.is_subset(&entry_parts)
        } else {
            !parts.is_disjoint(&entry_parts)
        }
    }

    /// Whether `sibling` is a later part of this entry beside the
    /// listing `wide` that would span both: right after `wide`'s stem
    /// it names a division of its own ([`later_divisions`]) beyond the
    /// part the entry's titles end on — part 1 when they end on none.
    ///
    /// [`later_divisions`]: super::play_native_title_grammar::later_divisions
    pub(crate) fn names_later_part(&self, sibling: &str, wide: &str) -> bool {
        let own = self.own_part();
        later_divisions(sibling, wide).iter().any(|n| *n > own)
    }

    /// The part the entry's titles end on — the highest, part 1 when
    /// they end on none.
    fn own_part(&self) -> u32 {
        self.all().flat_map(part_ordinals).max().unwrap_or(1)
    }

    /// Whether `title` reads as one of the entry's titles does
    /// ([`reading`]) — the one reader the spanning cut compares titles
    /// with. A spanning listing that reads as the entry has the entry
    /// for its head.
    ///
    /// [`reading`]: super::play_native_title_grammar::reading
    pub(crate) fn reads_as_entry(&self, title: &str) -> bool {
        let theirs = reading(title);
        self.all().any(|t| reading(t) == theirs)
    }

    /// Whether `listing` is this entry's own beside the listing `wide`
    /// that would span it: it reads as the entry
    /// ([`Self::reads_as_entry`]) and carries `wide`'s stem alone or
    /// names a division past it ([`names_past_stem`]). So "Show 2",
    /// "Show 2nd Season" and "Show 2nd Season Part 1" are "Show 2"'s
    /// own beside "Show" while "Show 2nd Season Part 2" is not; "Lucky
    /// 2 2nd Season" is not "Lucky 2"'s; and "Show Side Story", which
    /// names no division past the stem, is another show of the
    /// franchise.
    ///
    /// [`names_past_stem`]: super::play_native_title_grammar::names_past_stem
    pub(crate) fn names_own_part(&self, listing: &str, wide: &str) -> bool {
        self.reads_as_entry(listing)
            && (stem(listing) == stem(wide) || names_past_stem(listing, wide))
    }
}

#[cfg(test)]
#[path = "play_native_title_marker_test.rs"]
mod tests;
