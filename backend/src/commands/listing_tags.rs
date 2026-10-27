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
//! record that outlives the row. Each record carries when its listing
//! was heard — the span from the pick's start to the pick, before the
//! appended-extras rule's reads — and a listing heard earlier never
//! replaces it; two whose spans overlap keep both listings' tags
//! ([`next_record`]).

use crate::app::AppState;
use crate::cache::ttl::PLAY_RESOLUTION_TTL;

fn key(show_key: &str) -> String {
    format!("listing-tags:v3:{show_key}")
}

#[path = "listing_tags_record.rs"]
mod record;
#[cfg(test)]
pub(crate) use record::tick_for_tests;
pub use record::Hearing;
pub(crate) use record::{next_record, Begun, Record};

/// Every read-then-write of a record is made under this one lock, so a
/// hearing compared against the stored one is compared against what is
/// stored when it writes.
static WRITE: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Record `tags`, the extra tags of `show_key`'s listing heard as
/// `heard` says, in the entry's numbering, as [`next_record`] decides
/// against what is stored: two walks of a show can overlap, and the one
/// that heard the listing first can write last. Best-effort: a write
/// that fails leaves the listing unknown to the next replay, which then
/// names no former number.
pub(crate) fn put(state: &AppState, show_key: &str, tags: &[String], heard: Hearing) {
    let _held = WRITE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let stored = crate::cache::meta_cache_get(&state.cache_pool, &key(show_key))
        .map(|b| b.and_then(|b| serde_json::from_str::<Record>(&b).ok()))
        .map_err(|_| ());
    let new = Record {
        heard,
        tags: tags.to_vec(),
    };
    let Some(record) = next_record(stored, new) else {
        return;
    };
    let Ok(body) = serde_json::to_string(&record) else {
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

/// Record the listing `episodes` of `show_key`, heard as `heard` says:
/// its extra tags in the entry's numbering.
pub(crate) fn record_listing(
    state: &AppState,
    show_key: &str,
    episodes: &[crate::scraper::provider::EpisodeRef],
    heard: Hearing,
) {
    put(
        state,
        show_key,
        &crate::commands::play_native_numbering::extra_episode_tags(episodes),
        heard,
    );
}

/// Hand `sink` every version's listing of `picked`, a pick of
/// `provider`'s, heard as `heard` says, each under its own show key.
pub(crate) fn hand_copies(
    sink: Option<&dyn ListingSink>,
    provider: crate::scraper::provider::ProviderId,
    picked: &crate::commands::play_native::PickedShow,
    heard: Hearing,
) {
    let Some(sink) = sink else {
        return;
    };
    for (hit, episodes) in picked.copies() {
        sink.heard(
            &crate::scraper::provider::ShowKey::new(provider, hit.slug.clone()).to_string(),
            episodes,
            heard,
        );
    }
}

/// Where a walk hands each listing it picks, as it picks it.
pub trait ListingSink: Send + Sync + std::fmt::Debug {
    /// `episodes`, the listing of `show_key` heard as `heard` says, is
    /// at hand.
    fn heard(
        &self,
        show_key: &str,
        episodes: &[crate::scraper::provider::EpisodeRef],
        heard: Hearing,
    );
}

/// The sink that records each listing a walk picks ([`record_listing`]).
pub(crate) struct Recorder<'s>(pub(crate) &'s AppState);

impl std::fmt::Debug for Recorder<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("listing_tags::Recorder")
    }
}

impl ListingSink for Recorder<'_> {
    fn heard(
        &self,
        show_key: &str,
        episodes: &[crate::scraper::provider::EpisodeRef],
        heard: Hearing,
    ) {
        record_listing(self.0, show_key, episodes, heard);
    }
}

/// The extra tags `show_key`'s listing last carried, or `None` when the
/// app has not had that listing at hand within the record's lifetime.
pub(crate) fn get(state: &AppState, show_key: &str) -> Option<Vec<String>> {
    let body = crate::cache::meta_cache_get(&state.cache_pool, &key(show_key)).ok()??;
    serde_json::from_str::<Record>(&body).ok().map(|r| r.tags)
}
