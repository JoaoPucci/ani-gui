//! Whether a sibling beside a broad listing names a later part of the
//! entry or a season before the listing's — split from
//! `play_native_title_marker` for the per-file complexity bar.

use super::super::play_native_title_reading::without_stem_number;
use super::*;

impl EntryTitles<'_> {
    /// Whether `sibling` is a later part of this entry beside the
    /// listing `wide` that would span both: right after `wide`'s stem
    /// it names a division of its own ([`divisions_beside`]) later than
    /// `wide` — a season above the one `wide` carries, or a part above
    /// the one the entry's titles end on (part 1 when they end on
    /// none). A sibling naming an earlier season anywhere among those
    /// divisions is never one, whatever part it names besides
    /// ([`names_earlier_season`](Self::names_earlier_season)): "Show
    /// Season 2 Part 2" beside "Show Season 3" is another season.
    ///
    /// [`divisions_beside`]: super::play_native_title_reading::divisions_beside
    pub(crate) fn names_later_part(&self, sibling: &str, wide: &str) -> bool {
        let own = self.own_part();
        divisions_beside(sibling, wide).is_some_and(|(named, season)| {
            !named.iter().any(|m| season_before(m, season))
                && named.iter().any(|m| {
                    let bar = if m.kind == Kind::Season { season } else { own };
                    m.ordinals.iter().any(|n| *n > bar)
                })
        })
    }

    /// Whether `sibling` names, right after `wide`'s stem, a season
    /// before the one `wide` carries ([`divisions_beside`]): "Show
    /// Season 2" beside "Show Season 3", "Show Season 1" beside "Show
    /// 2" — the season before `wide`, never this entry's listing.
    ///
    /// [`divisions_beside`]: super::play_native_title_reading::divisions_beside
    pub(crate) fn names_earlier_season(&self, sibling: &str, wide: &str) -> bool {
        divisions_beside(sibling, wide)
            .is_some_and(|(named, season)| named.iter().any(|m| season_before(m, season)))
    }

    /// Whether `candidate` is the season before this entry: one of its
    /// titles carries a season `candidate` names one before, read past
    /// that title's stem as a sibling is read past a broad listing's
    /// ([`names_earlier_season`](Self::names_earlier_season)) — "Show
    /// Season 2 Part 2" for "Show Season 3 Part 2" — or `candidate`'s
    /// stem ends on a lower number after the same words ("Show 2 Part
    /// 2" for "Show 3 Part 2"); and it is not one of the entry's titles.
    pub(crate) fn names_season_before_entry(&self, candidate: &str) -> bool {
        let exact = normalized(candidate);
        !self.all().any(|t| normalized(t) == exact)
            && self
                .all()
                .any(|t| self.names_earlier_season(candidate, t) || lower_stem_number(candidate, t))
    }

    /// Whether `candidate` is another entry of the same show, never
    /// picked: the entry before this one
    /// ([`before_entry`](Self::before_entry)) or the season before it
    /// ([`names_season_before_entry`](Self::names_season_before_entry)).
    pub(crate) fn another_entry(&self, candidate: &str) -> bool {
        self.before_entry(candidate) || self.names_season_before_entry(candidate)
    }

    /// The part the entry's titles end on — the highest, part 1 when
    /// they end on none.
    fn own_part(&self) -> u32 {
        self.all().flat_map(part_ordinals).max().unwrap_or(1)
    }
}

/// Whether `m` is a season before `season`.
fn season_before(m: &Marker, season: u32) -> bool {
    m.kind == Kind::Season && m.ordinals.iter().any(|n| *n < season)
}

/// Whether `candidate`'s stem ends on a number lower than the one
/// `title`'s stem ends on, after the same words.
fn lower_stem_number(candidate: &str, title: &str) -> bool {
    match (
        without_stem_number(&stem(candidate)),
        without_stem_number(&stem(title)),
    ) {
        (Some((words, m)), Some((theirs, n))) => words == theirs && m < n,
        _ => false,
    }
}
