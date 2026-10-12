//! Whether a sibling beside a broad listing names a later part of the
//! entry or a season before the listing's — split from
//! `play_native_title_marker` for the per-file complexity bar.

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
