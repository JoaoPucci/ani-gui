//! The rule that reads a listing's rows past Kitsu's count as extras
//! ([`super`]), split from it so each file stays inside the
//! complexity bar.

use super::aligned::aligned;
use super::explained::explained;
use super::names::own_names;
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
/// - every row past the count has a name of its own, which none of
///   the first episodes of any aired TV entry Kitsu relates the show
///   to carries, whatever the relation's role ([`own_names`]);
/// - Kitsu relates the entry to a finished OVA, ONA or special exactly
///   as long as the rows past the count, and to no entry airing now,
///   whatever its role or format, a music video aside ([`explained`]).
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
    kitsu_ends_at(tail, count) && anilist_ends_at(tail, count) && rows_end_at(tail, count)
}

/// Whether Kitsu calls the show finished at `count`. Read from Kitsu's
/// entry alone, so the evidence reader asks for nothing more when it
/// does not.
#[must_use]
pub fn kitsu_ends_at(tail: &EntryTail, count: u32) -> bool {
    tail.finished && tail.count == Some(count)
}

/// Whether AniList ends the show at `count`, with nothing scheduled.
#[must_use]
pub fn anilist_ends_at(tail: &EntryTail, count: u32) -> bool {
    tail.anilist_count == Some(count)
}

/// Whether the Kitsu rows read are the ones that would show an episode
/// past `count`, and show none.
#[must_use]
pub fn rows_end_at(tail: &EntryTail, count: u32) -> bool {
    tail.read_past_count && tail.episodes.iter().all(|e| e.number <= count)
}
