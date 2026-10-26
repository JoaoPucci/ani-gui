//! The evidence reader's reads and the mapping of what they return
//! ([`super`]), split from it so each file stays inside the complexity
//! bar.

use super::{KitsuTail, KITSU_PAGE};
use crate::commands::play_native_appended::{
    EntryTail, Related, SequelEpisode, TailEpisode, ALIGNED,
};
use crate::meta::anilist_airing::AiringStatus;
use crate::meta::kitsu::{KitsuEpisode, KitsuRelation};

/// The 1-based Kitsu page episode `n` is on.
pub(super) fn page_of(n: u32) -> u32 {
    n.saturating_sub(1) / KITSU_PAGE + 1
}

/// The Kitsu pages that hold the count's last [`ALIGNED`] episodes,
/// and, when the count ends a page, the page after it: the pages the
/// recorded corpus holds for a counted show. The reader reads these
/// and, for a count that does not end a page, the page after the last
/// of them too (see `kitsu_rows`).
#[must_use]
pub fn tail_pages(count: u32) -> Vec<u32> {
    let first = count.saturating_sub(ALIGNED - 1).max(1);
    let mut pages: Vec<u32> = (page_of(first)..=page_of(count)).collect();
    if count % KITSU_PAGE == 0 {
        pages.push(page_of(count) + 1);
    }
    pages
}

/// The tail Kitsu's `status` and `count`, AniList's `airing` answer
/// and Kitsu's `episodes` rows give. AniList's
/// count is the one it gives a show it calls finished: an aired count
/// with nothing scheduled after it.
#[must_use]
pub fn entry_tail(
    status: Option<&str>,
    count: Option<u32>,
    airing: &AiringStatus,
    episodes: Vec<KitsuEpisode>,
) -> EntryTail {
    EntryTail {
        finished: status == Some("finished"),
        count,
        anilist_count: anilist_count(airing),
        episodes: tail_episodes(episodes),
        read_past_count: true,
        related: Vec::new(),
        related_complete: false,
    }
}

/// The count AniList gives a show it calls finished: an aired count
/// with nothing scheduled after it.
pub(super) fn anilist_count(airing: &AiringStatus) -> Option<u32> {
    airing
        .aired
        .filter(|n| *n > 0 && airing.next_episode.is_none())
}

/// Kitsu's rows as the evidence reads them: each with a number.
pub(super) fn tail_episodes(episodes: Vec<KitsuEpisode>) -> Vec<TailEpisode> {
    episodes
        .into_iter()
        .filter_map(|e| {
            Some(TailEpisode {
                number: e.number?,
                title: e.canonical_title,
                airdate: e.airdate,
            })
        })
        .collect()
}

/// The relations as the evidence reads them before any page is asked
/// for: enough to say whether they explain the rows past the count.
pub(super) fn unpaged(entries: &[KitsuRelation]) -> Vec<Related> {
    entries.iter().map(|r| related(r.clone(), None)).collect()
}

/// Whether `r` is a TV entry, in whatever role Kitsu relates it: an
/// entry whose first episodes the rows past the count are checked
/// against.
pub(super) fn is_tv(r: &KitsuRelation) -> bool {
    r.kind == "anime" && r.subtype.as_deref() == Some("TV")
}

/// A related entry's page of episodes as the evidence reads it: each
/// row with a number, by that number, with the title Kitsu gives it.
pub(super) fn sequel_episodes(page: Vec<KitsuEpisode>) -> Vec<SequelEpisode> {
    page.into_iter()
        .filter_map(|e| {
            Some(SequelEpisode {
                number: e.number?,
                title: e.canonical_title,
            })
        })
        .collect()
}

/// A relation as the evidence reads it.
pub(super) fn related(r: KitsuRelation, first_episodes: Option<Vec<SequelEpisode>>) -> Related {
    Related {
        anime: r.kind == "anime",
        role: r.role,
        subtype: r.subtype,
        status: r.status,
        episode_count: r.episode_count,
        first_episodes,
    }
}

impl KitsuTail<'_> {
    /// Kitsu's rows on the entry's `page`.
    pub(super) async fn page(&self, page: u32) -> Option<Vec<KitsuEpisode>> {
        crate::commands::kitsu::kitsu_episode_page(self.state, self.kitsu_id, page)
            .await
            .ok()
    }

    /// The entries Kitsu relates the show to, and whether they are
    /// all of them.
    pub(super) async fn read_relations(&self) -> Option<crate::meta::kitsu::KitsuRelations> {
        crate::commands::kitsu::kitsu_relationships(self.state, self.kitsu_id)
            .await
            .ok()
    }
}
