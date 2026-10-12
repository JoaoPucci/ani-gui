//! The entry's own titles, as the picker reads them — split from
//! `play_native` for the per-file complexity bar.
//!
//! A pick is made for one anime-database entry, under one search
//! term at a time; the term is only one of the entry's names. The
//! picker carries all of them, canonical first, so a rule about what
//! the entry is called can read every name the entry goes by rather
//! than the alias that happened to be searched.

use std::collections::BTreeSet;

use super::play_native_title_grammar::normalized;
pub(crate) use super::play_native_title_grammar::stem;
use super::play_native_title_grammar::Kind;
use super::play_native_title_grammar::Marker;
use super::play_native_title_grammar::{
    named_ordinals, part_ordinals, stem_number, trailing_markers,
};
use super::play_native_title_number::{carries_number_of, continues_number_of};
use super::play_native_title_reading::{
    divisions_beside, is_name_and_first_divisions, name, names_past_stem,
    opens_on_a_first_division, reading, starts_with_name_of,
};

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
    /// - A bare number of one or two digits the candidate's title ends
    ///   on, before any markers, is held to the same rule: "Overlord:
    ///   Ple Ple Pleiades 2" is not Overlord, "Show 2 Part 1" is not
    ///   "Show", while "Kaiju No. 8" is admitted for an entry naming 8.
    ///   A 1 never disqualifies, and three digits ("Mob Psycho 100")
    ///   number no sequel. An entry names a number written before a
    ///   Japanese counter too: "怪獣８号" admits "Kaiju No. 8".
    ///
    /// - A title matching one of the entry's up to its first number and
    ///   only continuing that number is another show
    ///   ([`Self::continues_a_number`]): "Show20" for "Show 2", "Mob
    ///   Psycho 1000 Part 2" for "Mob Psycho 100 II", "861" for "86 Part
    ///   2".
    ///
    /// Only markers that end a title are read as the title's own, so
    /// "JoJo's Bizarre Adventure Part 4: Diamond is Unbreakable" names
    /// a story part, not a cour.
    pub(crate) fn admits(&self, candidate: &str) -> bool {
        let named: BTreeSet<u32> = self.all().flat_map(named_ordinals).collect();
        trailing_markers(candidate)
            .iter()
            .all(|m| m.named_by(&named))
            && stem_number(candidate).is_none_or(|n| n == 1 || named.contains(&n))
            && !self.continues_a_number(candidate)
    }

    /// Whether `candidate` matches one of the entry's titles only by
    /// continuing its first number ([`continues_number_of`]) — "Show20"
    /// for "Show 2", "861" for "86 Part 2" — and no title of the entry
    /// carries the number it does: another show.
    ///
    /// [`continues_number_of`]: super::play_native_title_number::continues_number_of
    fn continues_a_number(&self, candidate: &str) -> bool {
        self.all().any(|t| continues_number_of(candidate, t))
            && !self.all().any(|t| carries_number_of(candidate, t))
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
}

#[path = "play_native_title_sibling.rs"]
mod title_sibling;

#[path = "play_native_title_own.rs"]
mod title_own;

#[cfg(test)]
#[path = "play_native_title_marker_test.rs"]
mod tests;
