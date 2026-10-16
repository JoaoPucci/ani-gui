//! The evidence the appended-extras rule reads
//! ([`super::play_native_appended`]), from what a detail page visit
//! already caches: Kitsu's entry, the Kitsu episode pages holding the
//! count's last episodes, and AniList's airing answer. Each is read
//! from the cache first and fetched only when it is missing.

use super::play_native_appended::{EntryTail, TailEpisode, TailSource, ALIGNED};
use crate::app::AppState;
use crate::meta::anilist_airing::AiringStatus;
use crate::meta::kitsu::KitsuEpisode;

/// How many episodes Kitsu lists to a page, as the backend asks for
/// them.
pub const KITSU_PAGE: u32 = 20;

/// The evidence for one Kitsu entry.
pub struct KitsuTail<'a> {
    state: &'a AppState,
    kitsu_id: &'a str,
}

impl<'a> KitsuTail<'a> {
    /// The source for the entry `kitsu_id` names; none for a caller
    /// with no entry behind it.
    #[must_use]
    pub fn for_entry(state: &'a AppState, kitsu_id: Option<&'a str>) -> Option<Self> {
        kitsu_id
            .filter(|id| !id.is_empty())
            .map(|kitsu_id| Self { state, kitsu_id })
    }
}

impl std::fmt::Debug for KitsuTail<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KitsuTail")
            .field("kitsu_id", &self.kitsu_id)
            .finish_non_exhaustive()
    }
}

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
    }
}

#[async_trait::async_trait]
impl TailSource for KitsuTail<'_> {
    async fn tail(&self, count: u32) -> Option<EntryTail> {
        let detail = super::kitsu::kitsu_anime_detail(self.state, self.kitsu_id)
            .await
            .ok()?;
        let airing = super::airing::airing_get(self.state, self.kitsu_id)
            .await
            .ok()?;
        let mut episodes = Vec::new();
        for page in tail_pages(count) {
            episodes.extend(
                super::kitsu::kitsu_episode_page(self.state, self.kitsu_id, page)
                    .await
                    .ok()?,
            );
        }
        Some(entry_tail(
            detail.status.as_deref(),
            detail.episode_count,
            &airing,
            episodes,
        ))
    }
}

#[cfg(test)]
#[path = "kitsu_tail_test.rs"]
mod tests;
