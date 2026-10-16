//! Extras a provider appends after a show's last episode.
//!
//! hianime lists some shows' OADs and web-only episodes as further
//! whole-numbered rows after the last episode Kitsu counts — Noragami
//! lists its two OADs as episodes 13 and 14 of a twelve-episode show.
//! Read as whole episodes they become tiles 13 and 14, raise the cap
//! past the show's end, and send tracker progress past the count the
//! trackers hold. When Kitsu and AniList agree conclusively that the
//! show ended at its count, and the listing's own titles line up with
//! Kitsu's up to that count, the rows past it are read as extras of
//! their own instead: tagged `12.1`, `12.2` beside episode 12, as a
//! recap is tagged `7.5`. Anything short of that evidence leaves the
//! listing as it was (see docs/title-resolution.md, "Episode
//! numbers").

use super::play_native::PickedShow;
use crate::scraper::provider::EpisodeRef;

/// The most rows past the count that can be read as appended extras.
/// A listing running further past Kitsu's count than this is a
/// different disagreement — a long runner Kitsu stopped counting, a
/// listing spanning entries — and is left alone. It also keeps every
/// extra's tag a single decimal digit (`12.1` to `12.9`), which reads
/// back as itself wherever a tag is parsed as a number.
pub const MAX_APPENDED: u32 = 9;

/// How long the evidence may take before the listing is left as it
/// was. Every source behind it is cached; this bounds a cold one.
pub const TAIL_BUDGET: std::time::Duration = std::time::Duration::from_secs(5);

/// One of Kitsu's episode rows, as the evidence reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TailEpisode {
    /// Kitsu's episode number.
    pub number: u32,
    /// Kitsu's canonical title for it, when it has one.
    pub title: Option<String>,
    /// Its air date, when Kitsu has one.
    pub airdate: Option<String>,
}

/// What Kitsu and AniList say about the end of an entry: everything
/// the rule reads to decide that a listing's rows past the count are
/// not episodes of the show.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EntryTail {
    /// Kitsu's status is `finished`.
    pub finished: bool,
    /// Kitsu's episode count.
    pub count: Option<u32>,
    /// The count AniList gives a show it calls finished; `None` while
    /// AniList calls it anything else, or does not know it.
    pub anilist_count: Option<u32>,
    /// Kitsu's episode rows on the pages read: the ones holding the
    /// count's last episodes.
    pub episodes: Vec<TailEpisode>,
    /// Whether the pages read are the ones that would hold an episode
    /// past the count, had Kitsu one.
    pub read_past_count: bool,
}

/// Where the evidence comes from. Asked only for a listing that runs
/// past the count ([`appended_count`]).
#[async_trait::async_trait]
pub trait TailSource: Send + Sync + std::fmt::Debug {
    /// The entry's tail, as far as it is known for `count`; `None`
    /// when nothing could be read.
    async fn tail(&self, count: u32) -> Option<EntryTail>;
}

/// The count to ask the evidence about, when `listing` has the shape
/// appended extras give it: Kitsu counts the entry, every row is
/// numbered by its position and carries no tag of its own — a recap's
/// fraction, a continuation's offset, a stitched or cut listing's
/// re-display each mean another reading is already in force — and the
/// listing runs past the count by at most [`MAX_APPENDED`] rows.
#[must_use]
pub fn appended_count(listing: &[EpisodeRef], expected: Option<u32>) -> Option<u32> {
    let count = expected.filter(|c| *c > 0)?;
    let plain = listing
        .iter()
        .zip(1u32..)
        .all(|(e, slot)| e.number == slot && e.number2.is_none());
    let past = u32::try_from(listing.len()).ok()?.checked_sub(count)?;
    (plain && (1..=MAX_APPENDED).contains(&past)).then_some(count)
}

/// How many of the count's last episodes must carry the listing's own
/// titles: three, or all of them for a shorter show.
const ALIGNED: u32 = 3;

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
    let mut out = listing.to_vec();
    for (row, i) in out.iter_mut().skip(at).zip(1u32..) {
        row.number2 = Some(format!("{count}.{i}"));
    }
    Some(out)
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

/// `picked` with the extras its provider appended read as extras,
/// when the evidence proves them; `picked` itself otherwise — no
/// source, a listing of another shape, evidence that is missing,
/// late or short of proof.
pub async fn fit_appended(
    picked: PickedShow,
    expected: Option<u32>,
    source: Option<&dyn TailSource>,
) -> PickedShow {
    let (Some(count), Some(source)) = (appended_count(&picked.episodes, expected), source) else {
        return picked;
    };
    let Ok(Some(tail)) = tokio::time::timeout(TAIL_BUDGET, source.tail(count)).await else {
        return picked;
    };
    match appended_extras(&picked.episodes, count, &tail) {
        Some(episodes) => PickedShow { episodes, ..picked },
        None => picked,
    }
}

#[cfg(test)]
#[path = "play_native_appended_test.rs"]
mod tests;

#[cfg(test)]
#[path = "play_native_appended_prop_test.rs"]
mod prop_tests;
