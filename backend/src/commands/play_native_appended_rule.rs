//! The rule that reads a listing's rows past Kitsu's count as extras
//! ([`super`]), split from it so each file stays inside the
//! complexity bar.

use super::{appended_count, tag_past, EntryTail, Related};
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

/// Whether every row past the count has a name of its own: one that is
/// not missing, not only its number, and not the name of an episode
/// Kitsu lists for the entry or on a TV sequel's first page.
fn own_names(past: &[EpisodeRef], tail: &EntryTail) -> bool {
    let sequels = tail.related.iter().filter(|r| is_tv_sequel(r));
    let Some(sequel_titles) = sequels
        .map(|r| r.episode_titles.as_deref())
        .collect::<Option<Vec<&[String]>>>()
    else {
        return false;
    };
    let taken: Vec<&str> = tail
        .episodes
        .iter()
        .filter_map(|e| e.title.as_deref())
        .chain(sequel_titles.into_iter().flatten().map(String::as_str))
        .collect();
    past.iter().all(|row| {
        row.title
            .as_deref()
            .is_some_and(|t| !is_generic(t) && !taken.iter().any(|k| same_title(k, t)))
    })
}

fn is_tv_sequel(r: &Related) -> bool {
    r.anime && r.role == "sequel" && r.subtype.as_deref() == Some("TV")
}

/// Whether Kitsu's relations, read whole, name what the `past` rows
/// are — a finished OVA, ONA or special exactly that long — and no
/// sequel still to finish, which the rows could be the start of.
fn explained(past: usize, tail: &EntryTail) -> bool {
    let finished = |r: &Related| r.status.as_deref() == Some("finished");
    let length = u32::try_from(past).ok();
    let extra = |r: &Related| {
        r.anime
            && finished(r)
            && matches!(r.subtype.as_deref(), Some("OVA" | "ONA" | "special"))
            && r.episode_count == length
    };
    let airing_sequel = |r: &Related| r.anime && r.role == "sequel" && !finished(r);
    tail.related_complete
        && tail.related.iter().any(extra)
        && !tail.related.iter().any(airing_sequel)
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
