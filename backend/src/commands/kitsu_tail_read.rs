//! The evidence reader's reads and the mapping of what they return
//! ([`super`]), split from it so each file stays inside the complexity
//! bar.

use super::{KitsuTail, KITSU_PAGE};
use crate::commands::play_native_appended::{EntryTail, Related, TailEpisode, ALIGNED};
use crate::meta::anilist_airing::AiringStatus;
use crate::meta::kitsu::{KitsuEpisode, KitsuRelation};

/// The 1-based Kitsu page episode `n` is on.
fn page_of(n: u32) -> u32 {
    n.saturating_sub(1) / KITSU_PAGE + 1
}

/// The Kitsu pages that hold the count's last [`ALIGNED`] episodes,
/// and the one an episode past the count would be on: the page after
/// when the count ends a page.
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
/// and the rows of the pages [`tail_pages`] names give. AniList's
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
        anilist_count: airing
            .aired
            .filter(|n| *n > 0 && airing.next_episode.is_none()),
        episodes: episodes
            .into_iter()
            .filter_map(|e| {
                Some(TailEpisode {
                    number: e.number?,
                    title: e.canonical_title,
                    airdate: e.airdate,
                })
            })
            .collect(),
        read_past_count: true,
        related: Vec::new(),
        related_complete: false,
    }
}

/// Whether Kitsu relates `r` as a TV sequel: the entry whose first
/// episodes the rows past the count are checked against.
fn is_tv_sequel(r: &KitsuRelation) -> bool {
    r.kind == "anime" && r.role == "sequel" && r.subtype.as_deref() == Some("TV")
}

/// A relation as the evidence reads it.
fn related(r: KitsuRelation, episode_titles: Option<Vec<String>>) -> Related {
    Related {
        anime: r.kind == "anime",
        role: r.role,
        subtype: r.subtype,
        status: r.status,
        episode_count: r.episode_count,
        episode_titles,
    }
}

impl KitsuTail<'_> {
    /// Kitsu's rows on the pages [`tail_pages`] names, and on the page
    /// after the last of them when that one is full to its end with
    /// nothing past the count on it: Kitsu's pages are not always
    /// dense, so a full page says nothing about what follows it.
    pub(super) async fn read_pages(&self, count: u32) -> Option<Vec<KitsuEpisode>> {
        let mut episodes = Vec::new();
        let mut last = (0, 0);
        for page in tail_pages(count) {
            let rows = self.page(page).await?;
            last = (page, rows.len());
            episodes.extend(rows);
        }
        let full = last.1 >= usize::try_from(KITSU_PAGE).unwrap_or(usize::MAX);
        let nothing_past = episodes.iter().all(|e| e.number.is_none_or(|n| n <= count));
        if full && nothing_past {
            episodes.extend(self.page(last.0 + 1).await?);
        }
        Some(episodes)
    }

    async fn page(&self, page: u32) -> Option<Vec<KitsuEpisode>> {
        crate::commands::kitsu::kitsu_episode_page(self.state, self.kitsu_id, page)
            .await
            .ok()
    }

    /// The entries Kitsu relates the show to, each TV sequel with its
    /// first page of titles, and whether they are all of them.
    pub(super) async fn read_related(&self) -> Option<(Vec<Related>, bool)> {
        let relations = crate::commands::kitsu::kitsu_relationships(self.state, self.kitsu_id)
            .await
            .ok()?;
        let mut out = Vec::new();
        for r in relations.entries {
            let episode_titles = if is_tv_sequel(&r) {
                let page = crate::commands::kitsu::kitsu_episode_page(self.state, &r.id, 1)
                    .await
                    .ok()?;
                Some(page.into_iter().filter_map(|e| e.canonical_title).collect())
            } else {
                None
            };
            out.push(related(r, episode_titles));
        }
        Some((out, relations.complete))
    }
}
