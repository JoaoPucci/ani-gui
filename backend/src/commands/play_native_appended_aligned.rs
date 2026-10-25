//! Whether a listing names Kitsu's last episodes as Kitsu does
//! ([`super`]), and when the rows read already say it does not; split
//! from the rule so each file stays inside the complexity bar.

use super::titles::{is_generic, same_title};
use super::{EntryTail, ALIGNED};
use crate::scraper::provider::EpisodeRef;

/// The first of the count's last [`ALIGNED`] episodes.
fn first_aligned(count: u32) -> u32 {
    count.saturating_sub(ALIGNED - 1).max(1)
}

/// Episode `n`'s title, when Kitsu's first row for it is dated and
/// titled, and the listing's row `n` carries the same title.
fn named<'a>(listing: &[EpisodeRef], tail: &'a EntryTail, n: u32) -> Option<&'a str> {
    let kitsu = tail.episodes.iter().find(|e| e.number == n)?;
    kitsu.airdate.as_ref()?;
    let row = listing.get(usize::try_from(n - 1).ok()?)?;
    let title = kitsu.title.as_deref()?;
    same_title(title, row.title.as_deref()?).then_some(title)
}

/// Whether the listing names Kitsu's last [`ALIGNED`] episodes as
/// Kitsu does, and Kitsu dates each of them; titles that are only
/// numbers line up whatever a row holds, so one at least must be more.
#[must_use]
pub fn aligned(listing: &[EpisodeRef], count: u32, tail: &EntryTail) -> bool {
    let titles: Option<Vec<&str>> = (first_aligned(count)..=count)
        .map(|n| named(listing, tail, n))
        .collect();
    titles.is_some_and(|t| t.iter().any(|t| !is_generic(t)))
}

/// Whether what is in hand already says [`aligned`] cannot hold,
/// whatever rows are still to read:
///
/// - the listing names one of the count's last episodes by nothing a
///   title could match: no name, or punctuation alone;
/// - Kitsu's first row read for one of them is undated, untitled or
///   titled otherwise (rows read later come after it, so cannot become
///   the first);
/// - the listing names every one of them only by its number: a Kitsu
///   title matching such a name is a number too, and the rule asks for
///   one that is more.
#[must_use]
pub fn misaligned_so_far(listing: &[EpisodeRef], count: u32, tail: &EntryTail) -> bool {
    let name = |n: u32| {
        usize::try_from(n - 1)
            .ok()
            .and_then(|i| listing.get(i))
            .and_then(|row| row.title.as_deref())
    };
    let last = first_aligned(count)..=count;
    let unmatched = last.clone().any(|n| {
        let matchable = name(n).is_some_and(|t| same_title(t, t));
        let read = tail.episodes.iter().any(|e| e.number == n);
        !matchable || (read && named(listing, tail, n).is_none())
    });
    let numbers_only = last.clone().all(|n| name(n).is_none_or(is_generic));
    unmatched || (!last.is_empty() && numbers_only)
}
