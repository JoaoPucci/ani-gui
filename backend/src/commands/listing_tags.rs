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
//! was heard — at the pick, before the appended-extras rule's reads —
//! and a listing heard earlier never replaces it ([`put`]).

use crate::app::AppState;
use crate::cache::ttl::PLAY_RESOLUTION_TTL;

fn key(show_key: &str) -> String {
    format!("listing-tags:v3:{show_key}")
}

/// When a listing was heard. A walk cannot say exactly when the
/// listing it picked was fetched, only that it was after `start`, when
/// the walk asked for the show's listings, and by `at`, when it made the
/// pick. `run` names the process the stamps belong to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Hearing {
    /// The process whose clock `start` and `at` read.
    pub run: u64,
    /// No earlier than this the listing was fetched.
    pub start: u64,
    /// No later than this.
    pub at: u64,
}

/// A hearing begun: the moment a walk asks for a show's listings.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Begun(u64);

impl Begun {
    /// A hearing beginning now.
    #[must_use]
    pub(crate) fn now() -> Self {
        Self(heard_now())
    }

    /// The hearing, its pick made now.
    #[must_use]
    pub(crate) fn picked(self) -> Hearing {
        Hearing {
            run: 0,
            start: self.0,
            at: heard_now(),
        }
    }
}

/// A listing's record: its extra tags, and when the listing was heard.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Record {
    pub(crate) heard: Hearing,
    pub(crate) tags: Vec<String>,
}

/// The current moment, for a hearing's stamps: Unix milliseconds.
pub(crate) fn heard_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// The record to write for `new`, given what the store answered for the
/// show (`stored`: `Err` when the read failed), or `None` to write
/// nothing. A record heard no later than `new` is replaced; one heard
/// later is kept.
#[must_use]
pub(crate) fn next_record(stored: Result<Option<Record>, ()>, new: Record) -> Option<Record> {
    match stored {
        Ok(Some(old)) if old.heard.at > new.heard.at => None,
        _ => Some(new),
    }
}

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
