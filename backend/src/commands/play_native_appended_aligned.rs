//! Whether a listing names Kitsu's last episodes as Kitsu does
//! ([`super`]); split from the rule so each file stays inside the
//! complexity bar.

use super::titles::{is_generic, same_title};
use super::{EntryTail, ALIGNED};
use crate::scraper::provider::EpisodeRef;

/// The first of the count's last [`ALIGNED`] episodes.
pub(super) fn first_aligned(count: u32) -> u32 {
    count.saturating_sub(ALIGNED - 1).max(1)
}

/// Episode `n`'s title, when Kitsu's first row for it is dated and
/// titled, and the listing's row `n` carries the same title.
pub(super) fn named<'a>(listing: &[EpisodeRef], tail: &'a EntryTail, n: u32) -> Option<&'a str> {
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
