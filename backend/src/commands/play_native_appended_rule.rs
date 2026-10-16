//! The rule that reads a listing's rows past Kitsu's count as extras
//! ([`super`]), split from it so each file stays inside the
//! complexity bar.

use super::names::{explained, own_names};
use super::titles::{is_generic, same_title};
use super::{appended_count, tag_past, EntryTail};
use crate::scraper::provider::EpisodeRef;

/// How many of the count's last episodes must carry the listing's own
/// titles: three, or all of them for a shorter show.
pub const ALIGNED: u32 = 3;

/// `listing` with its rows past `count` tagged as extras — `count.1`,
/// `count.2`, … in listing order, each row otherwise the row it was —
/// when `tail` proves they are not episodes of the show:
///
/// - Kitsu calls the show finished at `count`, and AniList ends it at
///   the same count ([`ends_at`]);
/// - the listing names Kitsu's last [`ALIGNED`] episodes as Kitsu
///   does, at least one of them by more than its number, and Kitsu
///   dates them ([`aligned`]), so its numbering is Kitsu's up to the
///   count;
/// - every row past the count has a name of its own, which no TV
///   sequel's first episodes carry ([`own_names`]);
/// - Kitsu relates the entry to a finished OVA, ONA or special exactly
///   as long as the rows past the count, and to no sequel still to
///   finish ([`explained`]).
///
/// `None` otherwise, and for a listing [`appended_count`] turns away.
#[must_use]
pub fn appended_extras(
    listing: &[EpisodeRef],
    count: u32,
    tail: &EntryTail,
) -> Option<Vec<EpisodeRef>> {
    let at = usize::try_from(count).ok()?;
    let past = listing.get(at..)?;
    let proven = appended_count(listing, Some(count)) == Some(count)
        && ends_at(tail, count)
        && aligned(listing, count, tail)
        && own_names(past, tail)
        && explained(past.len(), tail);
    proven.then(|| tag_past(listing, count))
}

/// Whether Kitsu and AniList both end the show at `count`, and Kitsu's
/// rows read show nothing past it.
fn ends_at(tail: &EntryTail, count: u32) -> bool {
    tail.finished
        && tail.count == Some(count)
        && tail.anilist_count == Some(count)
        && tail.read_past_count
        && tail.episodes.iter().all(|e| e.number <= count)
}

/// Whether the listing names Kitsu's last [`ALIGNED`] episodes as
/// Kitsu does, and Kitsu dates each of them; titles that are only
/// numbers line up whatever a row holds, so one at least must be more.
fn aligned(listing: &[EpisodeRef], count: u32, tail: &EntryTail) -> bool {
    let first = count.saturating_sub(ALIGNED - 1).max(1);
    let named = |n: u32| -> Option<&str> {
        let kitsu = tail.episodes.iter().find(|e| e.number == n)?;
        kitsu.airdate.as_ref()?;
        let row = listing.get(usize::try_from(n - 1).ok()?)?;
        let title = kitsu.title.as_deref()?;
        same_title(title, row.title.as_deref()?).then_some(title)
    };
    let titles: Option<Vec<&str>> = (first..=count).map(named).collect();
    titles.is_some_and(|t| t.iter().any(|t| !is_generic(t)))
}
