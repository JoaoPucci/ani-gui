//! The entry's own titles, as the picker reads them — split from
//! `play_native` for the per-file complexity bar.
//!
//! A pick is made for one anime-database entry, under one search
//! term at a time; the term is only one of the entry's names. The
//! picker carries all of them, canonical first, so a rule about what
//! the entry is called can read every name the entry goes by rather
//! than the alias that happened to be searched.

use std::collections::BTreeSet;

#[path = "play_native_title_grammar.rs"]
mod grammar;
pub(crate) use grammar::stem;
use grammar::{named_ordinals, part_ordinals, trailing_markers};
#[cfg(test)]
use grammar::{Kind, Marker};

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
    ///   nor does a span that includes 1 ("(Part 1+2)").
    ///
    /// Only markers that end a title are read as the title's own, so
    /// "JoJo's Bizarre Adventure Part 4: Diamond is Unbreakable" names
    /// a story part, not a cour.
    pub(crate) fn admits(&self, candidate: &str) -> bool {
        let named: BTreeSet<u32> = self.all().flat_map(named_ordinals).collect();
        trailing_markers(candidate)
            .iter()
            .all(|m| m.ordinals.iter().any(|n| *n < 2 || named.contains(n)))
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
        } else {
            !parts.is_disjoint(&entry_parts)
        }
    }
}

#[cfg(test)]
#[path = "play_native_title_marker_test.rs"]
mod tests;
