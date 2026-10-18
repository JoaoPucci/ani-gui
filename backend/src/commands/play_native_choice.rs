//! Choosing among a pick's survivors — split from the probe loop
//! for the per-file complexity bar: the winner selection over probed
//! candidates, plus the identity rank both the winner guard and the
//! probe loop's transport-death tracking share. The countless pick
//! lives in `play_native_countless`.

use crate::scraper::provider::{BrowseHit, EpisodeRef};

use super::play_native_title_grammar::normalized;
use super::play_native_title_marker::EntryTitles;

/// Identity a candidate carries: 0 = exact title, 1 = year-confirmed,
/// 2 = neither. Lower outranks higher.
pub(super) fn identity_rank(title_matches: bool, confirmed: bool) -> u8 {
    if title_matches {
        0
    } else if confirmed {
        1
    } else {
        2
    }
}

/// The rank a candidate is weighed by against the entry, in the
/// order winner selection prefers: 0 for an exact title; then, for a
/// candidate whose part agrees with the entry's
/// ([`EntryTitles::part_agrees`]), 1 with a matched year and 2
/// without; then 3 and 4 the same way for one naming another part.
/// An exact title or a matched year is identity — ranks 0 and odd.
pub(super) fn entry_rank(entry: EntryTitles<'_>, title: &str, needle: &str, confirmed: bool) -> u8 {
    let rank = identity_rank(normalized(title) == needle, confirmed);
    if rank == 0 || entry.part_agrees(title) {
        rank
    } else {
        rank + 2
    }
}

/// Whether a probed candidate could have been picked, as far as the
/// entry's titles go: admitted, not another entry of the same show —
/// each read past a bracketed version tag, as every reader of a probed
/// candidate's divisions reads ([`EntryTitles::admits_as_read`]) — and
/// not ruled out by the caller's rules otherwise (`could_win`).
pub(super) fn could_have_won(entry: EntryTitles<'_>, title: &str, could_win: bool) -> bool {
    could_win && entry.admits_as_read(title) && !entry.another_entry_as_read(title)
}

/// The strongest transport-dead candidate so far, by [`entry_rank`]
/// and then provider position, with the candidate that just died at
/// `pos` weighed in. A candidate the entry's titles refuse, or another
/// entry of the same show ([`EntryTitles::another_entry_as_read`]), could
/// never have won, so its death blocks no winner, and neither does one
/// the name rule that applies would have refused (`could_win` false:
/// `play_native_dated_name::may_win`); it still leaves the pool
/// unheard, which the caller records apart.
pub(super) fn strongest_dead(
    best: Option<(u8, usize)>,
    entry: EntryTitles<'_>,
    title: &str,
    needle: &str,
    confirmed: bool,
    pos: usize,
    could_win: bool,
) -> Option<(u8, usize)> {
    if !could_have_won(entry, title, could_win) {
        return best;
    }
    let failed = (entry_rank(entry, title, needle, confirmed), pos);
    Some(best.map_or(failed, |best| best.min(failed)))
}

/// The winner among best-distance candidates, plus its rank
/// ([`entry_rank`]). An exact title match is the user's own words and stays
/// dominant; below it, a candidate whose part marker agrees with the
/// entry's ([`EntryTitles::part_agrees`]) outranks one that names
/// another part; below that, a detail year that matched Kitsu's
/// exactly outranks a merely tolerated neighbor; only a full tie
/// falls to provider order (min_by_key keeps the first of equals).
pub(super) fn select_winner(
    probed_ok: &[(&BrowseHit, Vec<EpisodeRef>, u32, bool)],
    best_dist: u32,
    needle: &str,
    entry: EntryTitles<'_>,
) -> (usize, u8) {
    let winner_idx = probed_ok
        .iter()
        .enumerate()
        .filter(|(_, (_, _, d, _))| *d == best_dist)
        .min_by_key(|(_, (h, _, _, confirmed))| {
            (
                normalized(&h.title) != needle,
                !entry.part_agrees(&h.title),
                !*confirmed,
            )
        })
        .map(|(i, _)| i)
        .expect("best_dist came from this list");
    let (h, _, _, confirmed) = &probed_ok[winner_idx];
    (winner_idx, entry_rank(entry, &h.title, needle, *confirmed))
}
