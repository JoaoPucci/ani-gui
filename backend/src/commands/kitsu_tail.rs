//! The evidence the appended-extras rule reads
//! ([`super::play_native_appended`]), from what a detail page visit
//! already caches: Kitsu's entry, the Kitsu episode pages holding the
//! count's last episodes, and AniList's airing answer — and, past
//! those, the entries Kitsu relates the show to, with each aired TV
//! entry's first episode page. Each is read
//! from the cache first and fetched only when it is missing.

use super::play_native_appended::{EntryTail, Remembered, TailSource};
use crate::app::AppState;

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

/// Where the reading the evidence last proved for an entry is kept.
///
/// v2: the rule reads every aired TV entry Kitsu relates the show to,
/// whatever the role, and refuses an entry of any role airing now. A
/// v1 reading was proved against a TV sequel alone and may be one the
/// rule now refuses, so it is not read.
fn kept_key(kitsu_id: &str) -> String {
    format!("appended:v2:{kitsu_id}")
}

#[path = "kitsu_tail_read.rs"]
mod read;
pub use read::{entry_tail, tail_pages};

/// How long a proved reading is kept: as long as the title-match
/// rows a play stores.
const KEPT_FOR: std::time::Duration = crate::cache::ttl::TITLE_MATCH_TTL;

#[async_trait::async_trait]
impl TailSource for KitsuTail<'_> {
    fn remembered(&self) -> Option<Remembered> {
        let body = crate::cache::meta_cache_get(&self.state.cache_pool, &kept_key(self.kitsu_id))
            .ok()??;
        serde_json::from_str(&body).ok()
    }

    fn remember(&self, reading: Option<&Remembered>) {
        let key = kept_key(self.kitsu_id);
        match reading.and_then(|r| serde_json::to_string(r).ok()) {
            Some(body) => {
                let _ = crate::cache::meta_cache_put(
                    &self.state.cache_pool,
                    &key,
                    &body,
                    KEPT_FOR.as_secs(),
                );
            }
            None => {
                let _ = crate::cache::meta_cache_delete(&self.state.cache_pool, &key);
            }
        }
    }

    /// Each read is made only while the ones before it leave the
    /// reading open. A refusal they already settle (Kitsu not ending
    /// the show at the count, AniList not ending it there, a row Kitsu
    /// lists past it, relations that do not explain the rows) is
    /// returned at once: a later read failing must not hand the
    /// listing back to a reading kept from before.
    async fn tail(&self, count: u32, past: usize) -> Option<EntryTail> {
        use crate::commands::play_native_appended::{
            anilist_ends_at, explained, kitsu_ends_at, rows_end_at,
        };
        let detail = super::kitsu::kitsu_anime_detail(self.state, self.kitsu_id)
            .await
            .ok()?;
        let mut tail = entry_tail(
            detail.status.as_deref(),
            detail.episode_count,
            &crate::meta::anilist_airing::AiringStatus::default(),
            Vec::new(),
        );
        if !kitsu_ends_at(&tail, count) {
            return Some(tail);
        }
        let airing = super::airing::airing_get(self.state, self.kitsu_id)
            .await
            .ok()?;
        tail.anilist_count = read::anilist_count(&airing);
        if !anilist_ends_at(&tail, count) {
            return Some(tail);
        }
        tail.episodes = read::tail_episodes(self.read_pages(count).await?);
        if !rows_end_at(&tail, count) {
            return Some(tail);
        }
        let relations = self.read_relations().await?;
        tail.related_complete = relations.complete;
        tail.related = read::unpaged(&relations.entries);
        if !explained(past, &tail) {
            return Some(tail);
        }
        tail.related = self.with_first_pages(relations.entries).await?;
        Some(tail)
    }
}

#[cfg(test)]
#[path = "kitsu_tail_test.rs"]
mod tests;
