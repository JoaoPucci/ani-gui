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
    /// The entries Kitsu relates the entry to.
    pub related: Vec<Related>,
    /// Whether `related` is every entry Kitsu relates it to.
    pub related_complete: bool,
}

/// An entry Kitsu relates the entry to, as the evidence reads it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Related {
    /// How Kitsu relates it: `sequel`, `side_story`, `other`, ….
    pub role: String,
    /// Whether it is an anime entry (and not a manga's).
    pub anime: bool,
    /// Its subtype: `TV`, `OVA`, `ONA`, `special`, ….
    pub subtype: Option<String>,
    /// Its status: `finished`, `current`, ….
    pub status: Option<String>,
    /// Its episode count.
    pub episode_count: Option<u32>,
    /// Its first page of episodes as Kitsu lists them, where it was
    /// read: each by its number, with the title Kitsu gives it.
    pub first_episodes: Option<Vec<SequelEpisode>>,
}

/// One of a related entry's episodes, as the evidence reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SequelEpisode {
    /// Kitsu's episode number.
    pub number: u32,
    /// Kitsu's canonical title for it, when it has one.
    pub title: Option<String>,
}

/// A reading the evidence once proved, kept so a request whose
/// evidence cannot be read reads the listing the same way: the count,
/// and the ids of the rows past it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Remembered {
    /// The provider's slug for the show the rows were listed under.
    pub slug: String,
    /// Kitsu's count the rows were read past.
    pub count: u32,
    /// The provider's ids of the rows read as extras, in order.
    pub ids: Vec<u64>,
}

/// Where the evidence comes from. Asked only for a listing that runs
/// past the count ([`appended_count`]).
#[async_trait::async_trait]
pub trait TailSource: Send + Sync + std::fmt::Debug {
    /// The entry's tail, as far as it is known for `count`; `None`
    /// when nothing could be read.
    async fn tail(&self, count: u32) -> Option<EntryTail>;

    /// The reading the evidence last proved for the entry, if any.
    fn remembered(&self) -> Option<Remembered> {
        None
    }

    /// Keep `reading` as the one the evidence proved, or forget the
    /// kept one when it is `None`.
    fn remember(&self, _reading: Option<&Remembered>) {}
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

/// `picked` with the extras its provider appended read as extras,
/// when the evidence proves them, or when the evidence cannot be read
/// and the source keeps that reading for these very rows; `picked`
/// itself otherwise — no source, a listing of another shape, evidence
/// that reads and falls short of proof (which forgets a kept reading),
/// or no evidence and nothing kept.
pub async fn fit_appended(
    picked: PickedShow,
    expected: Option<u32>,
    source: Option<&dyn TailSource>,
) -> PickedShow {
    let Some(source) = source else {
        return picked;
    };
    let Some(count) = appended_count(&picked.episodes, expected) else {
        // The show's listing no longer runs past a count — Kitsu took
        // the rows in, or the listing changed shape — so a reading
        // kept for it no longer stands.
        let kept = source.remembered();
        if expected.is_some() && kept.is_some_and(|k| k.slug == picked.hit.slug) {
            source.remember(None);
        }
        return picked;
    };
    let reading = Remembered {
        slug: picked.hit.slug.clone(),
        count,
        ids: past_ids(&picked.episodes, count),
    };
    let fitted = match tokio::time::timeout(TAIL_BUDGET, source.tail(count)).await {
        Ok(Some(tail)) => {
            let fitted = appended_extras(&picked.episodes, count, &tail);
            source.remember(fitted.as_ref().map(|_| &reading));
            fitted
        }
        _ => (source.remembered() == Some(reading)).then(|| tag_past(&picked.episodes, count)),
    };
    match fitted {
        // Every version is the same episodes under its own ids: the
        // reading holds for each of them.
        Some(episodes) => PickedShow {
            versions: super::play_native_versions::fitted_versions(&picked.versions, &episodes),
            episodes,
            ..picked
        },
        None => picked,
    }
}

#[path = "play_native_appended_rule.rs"]
mod rule;
pub use rule::{appended_extras, ALIGNED};
#[path = "play_native_appended_names.rs"]
mod names;
pub use names::is_announced;
#[path = "play_native_appended_explained.rs"]
mod explained;
#[path = "play_native_appended_titles.rs"]
mod titles;
pub use titles::same_title;
#[path = "play_native_appended_slots.rs"]
mod slots;
use slots::past_ids;
pub use slots::{extra_for_slot, tag_past};

#[cfg(test)]
#[path = "play_native_appended_test.rs"]
mod tests;

#[cfg(test)]
#[path = "play_native_appended_prop_test.rs"]
mod prop_tests;
