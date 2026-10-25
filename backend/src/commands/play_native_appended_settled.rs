//! When what is in hand already says the listing cannot line up with
//! Kitsu's last episodes ([`aligned`](fn@super::aligned)), so the evidence reader
//! need read no further; split from the alignment itself so each file
//! stays inside the complexity bar.

use super::aligned::{first_aligned, named};
use super::titles::{is_generic, same_title};
use super::EntryTail;
use crate::scraper::provider::EpisodeRef;

/// Whether what is in hand already says [`aligned`](fn@super::aligned) cannot hold,
/// whatever rows are still to read:
///
/// - the listing names one of the count's last episodes by nothing a
///   title could match: no name, or one with no letter or digit;
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

/// Whether a row past the count (`past`) carries the listing's name
/// for one of the count's last episodes. For the listing to line up,
/// Kitsu titles that episode the same, and the row then carries one of
/// the show's own names: the rule refuses either way, and the listing
/// alone says so.
#[must_use]
pub fn past_named_as_a_last_episode(
    listing: &[EpisodeRef],
    count: u32,
    past: &[EpisodeRef],
) -> bool {
    (first_aligned(count)..=count)
        .filter_map(|n| usize::try_from(n - 1).ok().and_then(|i| listing.get(i)))
        .filter_map(|row| row.title.as_deref())
        .any(|name| {
            past.iter()
                .filter_map(|row| row.title.as_deref())
                .any(|title| same_title(name, title))
        })
}
