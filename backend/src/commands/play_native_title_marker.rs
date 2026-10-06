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
    named_ordinals, part_ordinals, tail_after, trailing_markers,
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

    /// Whether `sibling`, a title starting with `stem`, names a later
    /// part than this entry's in what it adds to the stem — "Part 2",
    /// "2nd Season", "Second Half War" after an entry that ends on no
    /// part, or on part 1. What completes a listing spanning this entry
    /// and the next has to be that next entry.
    pub(crate) fn names_later_part(&self, sibling: &str, stem: &[String]) -> bool {
        let own = self.all().flat_map(part_ordinals).max().unwrap_or(1);
        named_ordinals(&tail_after(sibling, stem))
            .iter()
            .any(|n| *n > own)
    }
}

#[cfg(test)]
#[path = "play_native_title_marker_test.rs"]
mod tests;
