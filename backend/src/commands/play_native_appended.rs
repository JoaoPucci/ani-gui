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
/// appended extras give it.
#[must_use]
pub fn appended_count(_listing: &[EpisodeRef], _expected: Option<u32>) -> Option<u32> {
    None
}

/// `listing` with its rows past `count` tagged as extras, when `tail`
/// proves they are not episodes of the show; `None` otherwise.
#[must_use]
pub fn appended_extras(
    _listing: &[EpisodeRef],
    _count: u32,
    _tail: &EntryTail,
) -> Option<Vec<EpisodeRef>> {
    None
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
