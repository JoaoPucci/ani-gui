//! The rule that reads a listing's rows past Kitsu's count as extras
//! ([`super`]), split from it so each file stays inside the
//! complexity bar.

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
///   the same count;
/// - Kitsu's rows for its last [`ALIGNED`] episodes are there, dated
///   and named, the pages read reach past the count, and none of them
///   lists an episode past it;
/// - the listing names each of those episodes as Kitsu does
///   ([`same_title`]), so its numbering is Kitsu's up to the count;
/// - every row past the count has a name of its own: not missing, not
///   just its number, not one of Kitsu's episodes.
///
/// `None` otherwise, and for a listing [`appended_count`] turns away.
#[must_use]
pub fn appended_extras(
    listing: &[EpisodeRef],
    count: u32,
    tail: &EntryTail,
) -> Option<Vec<EpisodeRef>> {
    if appended_count(listing, Some(count)) != Some(count) || !ends_at(tail, count) {
        return None;
    }
    let first = count.saturating_sub(ALIGNED - 1).max(1);
    for n in first..=count {
        let kitsu = tail.episodes.iter().find(|e| e.number == n)?;
        kitsu.airdate.as_ref()?;
        let row = listing.get(usize::try_from(n - 1).ok()?)?;
        if !same_title(kitsu.title.as_deref()?, row.title.as_deref()?) {
            return None;
        }
    }
    let at = usize::try_from(count).ok()?;
    let past = listing.get(at..)?;
    let own = past.iter().all(|row| {
        row.title.as_deref().is_some_and(|t| {
            !is_generic(t)
                && !tail
                    .episodes
                    .iter()
                    .filter_map(|e| e.title.as_deref())
                    .any(|k| same_title(k, t))
        })
    });
    if !own {
        return None;
    }
    Some(tag_past(listing, count))
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

/// A title reduced to what tells two titles apart: its letters and
/// digits, lowercased. Case, spacing and punctuation are how two
/// catalogues write one title differently.
fn reduced(title: &str) -> String {
    title
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Whether two titles are one title as the catalogues write it.
#[must_use]
pub fn same_title(a: &str, b: &str) -> bool {
    let a = reduced(a);
    !a.is_empty() && a == reduced(b)
}

/// Whether a title is only a number: "Episode 13", "EP 13", "13".
fn is_generic(title: &str) -> bool {
    let r = reduced(title);
    let digits = r
        .strip_prefix("episode")
        .or_else(|| r.strip_prefix("ep"))
        .unwrap_or(&r);
    digits.chars().all(|c| c.is_ascii_digit())
}
