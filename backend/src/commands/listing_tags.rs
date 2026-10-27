//! The extra tags a show's listing carried when the app last had that
//! listing at hand, kept per show key. A resolve — a play's, a
//! hand-off's, a download's, a warm's — records every version's listing
//! as it picks the show, whether or not the episode then serves
//! ([`Recorder`]); a probe and a range download record the listings
//! they pick, whatever they answer.
//!
//! A replay reads them to check the former number its cached row kept:
//! the row's resolve found the number free in the listing it had, and a
//! later listing of the same show may have come to tag another episode
//! with it ([`crate::commands::play_native_merge::former_on_replay`]).
//! A record lives as long as a cached row may ([`PLAY_RESOLUTION_TTL`])
//! from the last listing that wrote it, so a row's own resolve leaves a
//! record that outlives the row.

use crate::app::AppState;
use crate::cache::ttl::PLAY_RESOLUTION_TTL;

fn key(show_key: &str) -> String {
    format!("listing-tags:v1:{show_key}")
}

/// Record `tags`, the extra tags of `show_key`'s listing at hand, in the
/// entry's numbering. Best-effort: a write that fails leaves the listing
/// unknown to the next replay, which then names no former number.
pub(crate) fn put(state: &AppState, show_key: &str, tags: &[String]) {
    let Ok(body) = serde_json::to_string(tags) else {
        return;
    };
    if let Err(e) = crate::cache::meta_cache_put(
        &state.cache_pool,
        &key(show_key),
        &body,
        PLAY_RESOLUTION_TTL.as_secs(),
    ) {
        tracing::warn!(show_key, error = ?e, "listing tags write failed");
    }
}

/// Record the listing `episodes` of `show_key`, at hand: its extra tags
/// in the entry's numbering.
pub(crate) fn record_listing(
    state: &AppState,
    show_key: &str,
    episodes: &[crate::scraper::provider::EpisodeRef],
) {
    put(
        state,
        show_key,
        &crate::commands::play_native_numbering::extra_episode_tags(episodes),
    );
}

/// Where a walk hands each listing it picks, as it picks it.
pub trait ListingSink: Send + Sync + std::fmt::Debug {
    /// `episodes`, the listing of `show_key`, is at hand.
    fn heard(&self, show_key: &str, episodes: &[crate::scraper::provider::EpisodeRef]);
}

/// The sink that records each listing a walk picks ([`record_listing`]).
pub(crate) struct Recorder<'s>(pub(crate) &'s AppState);

impl std::fmt::Debug for Recorder<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("listing_tags::Recorder")
    }
}

impl ListingSink for Recorder<'_> {
    fn heard(&self, show_key: &str, episodes: &[crate::scraper::provider::EpisodeRef]) {
        record_listing(self.0, show_key, episodes);
    }
}

/// The extra tags `show_key`'s listing last carried, or `None` when the
/// app has not had that listing at hand within the record's lifetime.
pub(crate) fn get(state: &AppState, show_key: &str) -> Option<Vec<String>> {
    let body = crate::cache::meta_cache_get(&state.cache_pool, &key(show_key)).ok()??;
    serde_json::from_str(&body).ok()
}
